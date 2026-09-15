use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};

const KEYCHAIN_SERVICE: &str = "zen-sync";
const KEYCHAIN_ACCOUNT: &str = "github";
const KEYCHAIN_ENC_ACCOUNT: &str = "encryption-key";

// Set via environment at build time. See README for GitHub OAuth App setup.
pub const GITHUB_CLIENT_ID: &str = env!("GITHUB_CLIENT_ID");
const GITHUB_CLIENT_SECRET: &str = env!("GITHUB_CLIENT_SECRET");

pub const REPO_NAME: &str = "zen-sync-backup";
const RELEASE_TAG: &str = "storage";
const ENCRYPTION_KEY_ASSET: &str = "encryption-key.b64";
const METADATA_PATH: &str = "metadata-v2.json";
const SNAPSHOT_ASSET_PREFIX: &str = "mods-";
/// Full-profile snapshots written by zen-sync ≤ 0.1.x.
const LEGACY_METADATA_PATH: &str = "metadata.json";
const LEGACY_ASSET_PREFIX: &str = "profile-";
const API_BASE: &str = "https://api.github.com";
const UPLOAD_BASE: &str = "https://uploads.github.com";

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnapshotEntry {
    pub index: u8,
    pub pushed_at: String,
    pub machine_name: String,
    pub size_bytes: u64,
}

/// Per-machine metadata stored in the repo.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MachineMetadata {
    pub machine_id: String,
    pub snapshots: Vec<SnapshotEntry>,
    pub max_snapshots: u8,
    pub current_index: u8,
}

impl MachineMetadata {
    /// Index slot to write to next (ring buffer within this machine's slots).
    pub fn next_index(&self, max_snapshots: u8) -> u8 {
        if (self.snapshots.len() as u8) < max_snapshots {
            self.snapshots.len() as u8
        } else {
            (self.current_index + 1) % max_snapshots
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SyncMetadata {
    pub machines: Vec<MachineMetadata>,
}

pub struct MetadataWithSha {
    pub metadata: SyncMetadata,
    pub sha: String,
}

/// Leftovers from zen-sync ≤ 0.1.x, deleted on the first backup in the new format.
#[derive(Default)]
pub struct LegacySnapshots {
    pub asset_ids: Vec<u64>,
    metadata_sha: Option<String>,
}

impl LegacySnapshots {
    pub fn is_empty(&self) -> bool {
        self.asset_ids.is_empty() && self.metadata_sha.is_none()
    }
}

#[derive(Deserialize)]
struct Asset {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct ContentsResp {
    content: String,
    sha: String,
}

fn snapshot_asset_name(machine_id: &str, index: u8) -> String {
    format!("{SNAPSHOT_ASSET_PREFIX}{machine_id}-{index}.enc")
}

#[derive(Serialize, Deserialize)]
struct StoredCredentials {
    token: String,
    user_id: u64,
    username: String,
}

#[derive(Clone)]
pub struct GitHubClient {
    pub token: String,
    pub user_id: u64,
    pub username: String,
    pub release_id: u64,
    pub encryption_key: [u8; 32],
    pub http: reqwest::Client,
}

// ── Token / key management ────────────────────────────────────────────────────

pub fn has_stored_token() -> bool {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()
        .and_then(|e| e.get_password().ok())
        .is_some()
}

fn save_token(token: &str, user_id: u64, username: &str) -> Result<(), String> {
    let creds = StoredCredentials {
        token: token.to_string(),
        user_id,
        username: username.to_string(),
    };
    let json = serde_json::to_string(&creds).map_err(|e| e.to_string())?;
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|e| format!("Keychain error: {e}"))?
        .set_password(&json)
        .map_err(|e| format!("Failed to save token: {e}"))
}

pub fn remove_stored_token() -> Result<(), String> {
    for account in [KEYCHAIN_ACCOUNT, KEYCHAIN_ENC_ACCOUNT] {
        if let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, account) {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(format!("Failed to remove credentials: {e}")),
            }
        }
    }
    Ok(())
}

fn load_token_parts() -> Option<(String, u64, String)> {
    let json = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .ok()?
        .get_password()
        .ok()?;
    let creds: StoredCredentials = serde_json::from_str(&json).ok()?;
    Some((creds.token, creds.user_id, creds.username))
}

fn save_encryption_key(key: &[u8; 32]) -> Result<(), String> {
    let b64 = BASE64.encode(key);
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ENC_ACCOUNT)
        .map_err(|e| format!("Keychain error: {e}"))?
        .set_password(&b64)
        .map_err(|e| format!("Failed to save encryption key: {e}"))
}

fn load_encryption_key() -> Option<[u8; 32]> {
    let b64 = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ENC_ACCOUNT)
        .ok()?
        .get_password()
        .ok()?;
    let bytes = BASE64.decode(b64.trim()).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}

// ── OAuth ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

async fn fetch_user(http: &reqwest::Client, token: &str) -> Result<(u64, String), String> {
    let user: GitHubUser = http
        .get(format!("{API_BASE}/user"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "zen-sync-app")
        .send()
        .await
        .map_err(|e| format!("GitHub user fetch failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub user fetch error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("GitHub user parse error: {e}"))?;
    Ok((user.id, user.login))
}

pub async fn oauth_connect(app: &tauri::AppHandle) -> Result<(String, u64, String), String> {
    use tauri_plugin_opener::OpenerExt;

    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

    let port = tauri_plugin_oauth::start(move |url| {
        if let Some(tx) = tx.lock().unwrap().take() {
            let _ = tx.send(url);
        }
    })
    .map_err(|e| format!("OAuth server start failed: {e}"))?;

    let auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope=repo&redirect_uri=http://127.0.0.1:{}",
        GITHUB_CLIENT_ID, port
    );

    app.opener()
        .open_url(auth_url, None::<&str>)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    let redirect_url = rx
        .await
        .map_err(|_| "OAuth cancelled or timed out".to_string())?;

    let code = redirect_url
        .split('?')
        .nth(1)
        .and_then(|q| {
            q.split('&').find_map(|kv| {
                let mut parts = kv.splitn(2, '=');
                if parts.next()? == "code" {
                    parts.next().map(str::to_string)
                } else {
                    None
                }
            })
        })
        .ok_or("OAuth redirect did not contain a code")?;

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let resp: TokenResponse = http
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "zen-sync-app")
        .json(&serde_json::json!({
            "client_id": GITHUB_CLIENT_ID,
            "client_secret": GITHUB_CLIENT_SECRET,
            "code": code,
            "redirect_uri": format!("http://127.0.0.1:{port}"),
        }))
        .send()
        .await
        .map_err(|e| format!("Token exchange failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Token exchange parse error: {e}"))?;

    if let Some(err) = resp.error {
        return Err(format!(
            "OAuth error: {err} — {}",
            resp.error_description.unwrap_or_default()
        ));
    }

    let token = resp.access_token.ok_or("No access_token in response")?;
    let (user_id, username) = fetch_user(&http, &token).await?;
    Ok((token, user_id, username))
}

// ── GitHubClient ──────────────────────────────────────────────────────────────

impl GitHubClient {
    fn api_headers(&self) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(
            "Authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        h.insert(
            "Accept",
            "application/vnd.github+json".parse().unwrap(),
        );
        h.insert(
            "X-GitHub-Api-Version",
            "2022-11-28".parse().unwrap(),
        );
        h.insert("User-Agent", "zen-sync-app".parse().unwrap());
        h
    }

    async fn ensure_repo(&self) -> Result<bool, String> {
        let check = self
            .http
            .get(format!(
                "{API_BASE}/repos/{}/{REPO_NAME}",
                self.username
            ))
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| format!("Repo check failed: {e}"))?;

        if check.status() == reqwest::StatusCode::NOT_FOUND {
            self.http
                .post(format!("{API_BASE}/user/repos"))
                .headers(self.api_headers())
                .json(&serde_json::json!({
                    "name": REPO_NAME,
                    "private": true,
                    "description": "zen-sync profile storage — do not modify",
                    "auto_init": true
                }))
                .send()
                .await
                .map_err(|e| format!("Repo create failed: {e}"))?
                .error_for_status()
                .map_err(|e| format!("Repo create error: {e}"))?;
            return Ok(true);
        }
        Ok(false)
    }

    async fn wait_for_repo_ready(&self) -> Result<(), String> {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct Branch {
            name: String,
        }

        for attempt in 0..10u32 {
            tokio::time::sleep(std::time::Duration::from_millis(
                500 * (1u64 << attempt.min(4)),
            ))
            .await;

            let resp = self
                .http
                .get(format!(
                    "{API_BASE}/repos/{}/{REPO_NAME}/branches",
                    self.username
                ))
                .headers(self.api_headers())
                .send()
                .await
                .map_err(|e| format!("Repo-ready check failed: {e}"))?;

            if resp.status().is_success() {
                let branches: Vec<Branch> = resp
                    .json()
                    .await
                    .map_err(|e| format!("Repo-ready parse error: {e}"))?;
                if !branches.is_empty() {
                    return Ok(());
                }
            }
        }
        Err("Timed out waiting for GitHub to initialise the repository".into())
    }

    async fn ensure_release(&self) -> Result<u64, String> {
        #[derive(Deserialize)]
        struct Release {
            id: u64,
        }

        let check = self
            .http
            .get(format!(
                "{API_BASE}/repos/{}/{REPO_NAME}/releases/tags/{RELEASE_TAG}",
                self.username
            ))
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| format!("Release check failed: {e}"))?;

        if check.status() == reqwest::StatusCode::NOT_FOUND {
            let r: Release = self
                .http
                .post(format!(
                    "{API_BASE}/repos/{}/{REPO_NAME}/releases",
                    self.username
                ))
                .headers(self.api_headers())
                .json(&serde_json::json!({
                    "tag_name": RELEASE_TAG,
                    "name": "zen-sync Storage",
                    "body": "Managed by zen-sync — do not modify",
                    "draft": false,
                    "prerelease": false
                }))
                .send()
                .await
                .map_err(|e| format!("Release create failed: {e}"))?
                .error_for_status()
                .map_err(|e| format!("Release create error: {e}"))?
                .json()
                .await
                .map_err(|e| format!("Release create parse error: {e}"))?;
            Ok(r.id)
        } else {
            let r: Release = check
                .error_for_status()
                .map_err(|e| format!("Release fetch error: {e}"))?
                .json()
                .await
                .map_err(|e| format!("Release parse error: {e}"))?;
            Ok(r.id)
        }
    }

    async fn get_or_create_encryption_key(&self) -> Result<[u8; 32], String> {
        // Try local keychain first (avoids a network round-trip on reconnect)
        if let Some(key) = load_encryption_key() {
            return Ok(key);
        }

        // Try downloading from the repo
        if let Some(asset_id) = self.get_asset_id(ENCRYPTION_KEY_ASSET).await? {
            let b64 = self
                .http
                .get(format!(
                    "{API_BASE}/repos/{}/{REPO_NAME}/releases/assets/{asset_id}",
                    self.username
                ))
                .headers({
                    let mut h = self.api_headers();
                    h.insert("Accept", "application/octet-stream".parse().unwrap());
                    h
                })
                .send()
                .await
                .map_err(|e| format!("Key download failed: {e}"))?
                .text()
                .await
                .map_err(|e| format!("Key read failed: {e}"))?;
            let bytes = BASE64
                .decode(b64.trim())
                .map_err(|e| format!("Key decode failed: {e}"))?;
            if bytes.len() != 32 {
                return Err(format!("Encryption key wrong length: {}", bytes.len()));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            save_encryption_key(&key)?;
            return Ok(key);
        }

        // Generate fresh key and upload
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let b64 = BASE64.encode(key);
        self.upload_asset(ENCRYPTION_KEY_ASSET, b64.as_bytes(), "text/plain")
            .await?;
        save_encryption_key(&key)?;
        Ok(key)
    }

    pub async fn connect(app: &tauri::AppHandle) -> Result<Self, String> {
        let (token, user_id, username) = oauth_connect(app).await?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))?;
        let mut client = GitHubClient {
            token,
            user_id,
            username,
            release_id: 0,
            encryption_key: [0u8; 32],
            http,
        };
        let repo_created = client.ensure_repo().await?;
        if repo_created {
            client.wait_for_repo_ready().await?;
        }
        client.release_id = client.ensure_release().await?;
        client.encryption_key = client.get_or_create_encryption_key().await?;
        save_token(&client.token, client.user_id, &client.username)?;
        Ok(client)
    }

    pub async fn from_keychain() -> Result<Option<Self>, String> {
        let Some((token, user_id, username)) = load_token_parts() else {
            return Ok(None);
        };
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))?;
        if let Err(e) = fetch_user(&http, &token).await {
            crate::zslog!("[github] token invalid on restore: {e}");
            return Ok(None);
        }
        let mut client = GitHubClient {
            token,
            user_id,
            username,
            release_id: 0,
            encryption_key: [0u8; 32],
            http,
        };
        client.release_id = client.ensure_release().await?;
        client.encryption_key = client.get_or_create_encryption_key().await?;
        Ok(Some(client))
    }

    // ── Metadata ──────────────────────────────────────────────────────────────

    async fn get_contents(&self, path: &str) -> Result<Option<ContentsResp>, String> {
        let resp = self
            .http
            .get(format!(
                "{API_BASE}/repos/{}/{REPO_NAME}/contents/{path}",
                self.username
            ))
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| format!("Metadata fetch failed: {e}"))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let c: ContentsResp = resp
            .error_for_status()
            .map_err(|e| format!("Metadata fetch error: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Metadata parse error: {e}"))?;
        Ok(Some(c))
    }

    pub async fn read_metadata(&self) -> Result<Option<MetadataWithSha>, String> {
        let Some(c) = self.get_contents(METADATA_PATH).await? else {
            return Ok(None);
        };

        let decoded = BASE64
            .decode(c.content.replace('\n', ""))
            .map_err(|e| format!("Metadata decode error: {e}"))?;
        let metadata: SyncMetadata = serde_json::from_slice(&decoded)
            .map_err(|e| format!("Metadata JSON error: {e}"))?;

        Ok(Some(MetadataWithSha {
            metadata,
            sha: c.sha,
        }))
    }

    pub async fn write_metadata(
        &self,
        metadata: &SyncMetadata,
        expected_sha: Option<&str>,
    ) -> Result<(), String> {
        let json = serde_json::to_vec(metadata).map_err(|e| e.to_string())?;
        let content = BASE64.encode(&json);

        let mut body = serde_json::json!({
            "message": "zen-sync: update backup metadata",
            "content": content,
        });
        if let Some(sha) = expected_sha {
            body["sha"] = serde_json::json!(sha);
        }

        self.http
            .put(format!(
                "{API_BASE}/repos/{}/{REPO_NAME}/contents/{METADATA_PATH}",
                self.username
            ))
            .headers(self.api_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Metadata write failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Metadata write error: {e}"))?;
        Ok(())
    }

    // ── Legacy snapshots ──────────────────────────────────────────────────────

    pub async fn find_legacy_snapshots(&self) -> Result<LegacySnapshots, String> {
        let metadata_sha = self.get_contents(LEGACY_METADATA_PATH).await?.map(|c| c.sha);
        let asset_ids = self
            .list_assets()
            .await?
            .into_iter()
            .filter(|a| a.name.starts_with(LEGACY_ASSET_PREFIX) && a.name.ends_with(".enc"))
            .map(|a| a.id)
            .collect();
        Ok(LegacySnapshots { asset_ids, metadata_sha })
    }

    pub async fn delete_legacy_snapshots(&self, legacy: &LegacySnapshots) -> Result<(), String> {
        for &id in &legacy.asset_ids {
            self.delete_asset(id).await?;
        }
        if let Some(sha) = &legacy.metadata_sha {
            self.http
                .delete(format!(
                    "{API_BASE}/repos/{}/{REPO_NAME}/contents/{LEGACY_METADATA_PATH}",
                    self.username
                ))
                .headers(self.api_headers())
                .json(&serde_json::json!({
                    "message": "zen-sync: remove legacy full-profile snapshots",
                    "sha": sha,
                }))
                .send()
                .await
                .map_err(|e| format!("Legacy metadata delete failed: {e}"))?
                .error_for_status()
                .map_err(|e| format!("Legacy metadata delete error: {e}"))?;
        }
        crate::zslog!(
            "[github] deleted {} legacy snapshot assets",
            legacy.asset_ids.len()
        );
        Ok(())
    }

    // ── Release asset helpers ─────────────────────────────────────────────────

    async fn list_assets(&self) -> Result<Vec<Asset>, String> {
        const PER_PAGE: usize = 100;
        let mut all = Vec::new();
        for page in 1.. {
            let batch: Vec<Asset> = self
                .http
                .get(format!(
                    "{API_BASE}/repos/{}/{REPO_NAME}/releases/{}/assets?per_page={PER_PAGE}&page={page}",
                    self.username, self.release_id
                ))
                .headers(self.api_headers())
                .send()
                .await
                .map_err(|e| format!("Asset list failed: {e}"))?
                .error_for_status()
                .map_err(|e| format!("Asset list error: {e}"))?
                .json()
                .await
                .map_err(|e| format!("Asset list parse error: {e}"))?;
            let done = batch.len() < PER_PAGE;
            all.extend(batch);
            if done {
                break;
            }
        }
        Ok(all)
    }

    pub async fn get_asset_id(&self, name: &str) -> Result<Option<u64>, String> {
        Ok(self
            .list_assets()
            .await?
            .into_iter()
            .find(|a| a.name == name)
            .map(|a| a.id))
    }

    pub async fn delete_asset(&self, asset_id: u64) -> Result<(), String> {
        self.http
            .delete(format!(
                "{API_BASE}/repos/{}/{REPO_NAME}/releases/assets/{asset_id}",
                self.username
            ))
            .headers(self.api_headers())
            .send()
            .await
            .map_err(|e| format!("Asset delete failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Asset delete error: {e}"))?;
        Ok(())
    }

    async fn upload_asset(
        &self,
        name: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<(), String> {
        crate::zslog!("[github] uploading '{}' ({} bytes)", name, data.len());
        let upload_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))?;
        upload_client
            .post(format!(
                "{UPLOAD_BASE}/repos/{}/{REPO_NAME}/releases/{}/assets?name={name}",
                self.username, self.release_id
            ))
            .headers(self.api_headers())
            .header("Content-Type", content_type)
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| format!("Asset upload failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Asset upload error: {e}"))?;
        crate::zslog!("[github] upload complete: '{}'", name);
        Ok(())
    }

    /// Upload a snapshot bundle for the given machine_id + index slot.
    pub async fn upload_snapshot(
        &self,
        machine_id: &str,
        index: u8,
        data: &[u8],
    ) -> Result<(), String> {
        let name = snapshot_asset_name(machine_id, index);
        if let Some(id) = self.get_asset_id(&name).await? {
            self.delete_asset(id).await?;
        }
        self.upload_asset(&name, data, "application/octet-stream").await
    }

    pub async fn download_snapshot(
        &self,
        machine_id: &str,
        index: u8,
    ) -> Result<Vec<u8>, String> {
        let name = snapshot_asset_name(machine_id, index);
        crate::zslog!("[github] downloading '{}'", name);
        let asset_id = self
            .get_asset_id(&name)
            .await?
            .ok_or_else(|| format!("Snapshot asset '{name}' not found"))?;

        let download_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))?;

        let bytes = download_client
            .get(format!(
                "{API_BASE}/repos/{}/{REPO_NAME}/releases/assets/{asset_id}",
                self.username
            ))
            .headers({
                let mut h = self.api_headers();
                h.insert("Accept", "application/octet-stream".parse().unwrap());
                h
            })
            .send()
            .await
            .map_err(|e| format!("Snapshot download failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Snapshot download error: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("Snapshot read failed: {e}"))?
            .to_vec();
        crate::zslog!("[github] downloaded {} bytes", bytes.len());
        Ok(bytes)
    }

    /// Delete all snapshot assets for expired slots.
    pub async fn delete_snapshot_assets(
        &self,
        machine_id: &str,
        indices: &[u8],
    ) -> Result<(), String> {
        for &idx in indices {
            let name = snapshot_asset_name(machine_id, idx);
            if let Some(id) = self.get_asset_id(&name).await? {
                self.delete_asset(id).await?;
                crate::zslog!("[github] deleted expired asset '{}'", name);
            }
        }
        Ok(())
    }
}
