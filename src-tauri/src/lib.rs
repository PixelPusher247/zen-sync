mod bundle;
mod crypto;
mod ext_storage;
mod extensions;
mod fsutil;
mod github;
mod keyfile;
mod local_state;
pub mod logger;
mod prefs;
mod profile;
mod shortcuts;
mod sine;
mod sync;
mod zen_check;

use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

// ── App state ─────────────────────────────────────────────────────────────────

struct AppState {
    github_client: Option<Arc<github::GitHubClient>>,
    local_state: local_state::LocalState,
    config_dir: std::path::PathBuf,
    /// A saved sign-in is being restored in the background.
    reconnecting: bool,
    encryption: Encryption,
}

/// Where this device stands with the snapshot key.
#[derive(Clone)]
enum Encryption {
    /// Not connected, or not checked yet.
    Unknown,
    /// The repository has no key file: the first device sets a passphrase.
    /// `old_backups`: there are backups from 0.5 or earlier, which setup
    /// deletes because they used the old, unprotected key.
    NeedsSetup { old_backups: bool },
    /// The repository has a key file this device has no matching key for:
    /// the passphrase or recovery code unlocks it.
    Locked,
    Ready(crypto::DataKey),
}

impl Encryption {
    fn label(&self) -> &'static str {
        match self {
            Encryption::Unknown => "unknown",
            Encryption::NeedsSetup { .. } => "needs-setup",
            Encryption::Locked => "locked",
            Encryption::Ready(_) => "ready",
        }
    }
}

/// Check the repository's key file against this device's copy of the key.
async fn check_encryption(client: &github::GitHubClient) -> Result<Encryption, String> {
    let Some((file, _)) = client.read_key_file().await? else {
        return Ok(Encryption::NeedsSetup { old_backups: client.has_backups_without_key_file().await? });
    };
    Ok(match keyfile::load_local_key() {
        Some(key) if key.id_hex() == file.key_id => Encryption::Ready(key),
        _ => Encryption::Locked,
    })
}

/// Run a CPU-heavy key derivation off the async runtime's worker threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("Key derivation task failed: {e}"))?
}

/// Held for the whole of a backup or restore, so two can never run at once,
/// whatever the window does in the meantime.
struct OperationLock(tokio::sync::Mutex<()>);

/// How often, and how far apart, startup retries a saved sign-in that
/// failed for a reason other than GitHub rejecting it (e.g. no network yet).
const RECONNECT_ATTEMPTS: u32 = 20;
const RECONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);

impl AppState {
    fn selection(&self) -> bundle::Selection {
        bundle::Selection {
            options: self.local_state.sync_options.clone(),
            extension_overrides: self.local_state.extension_overrides.clone(),
            pref_overrides: self.local_state.pref_overrides.clone(),
        }
    }

    fn machine_id(&self) -> String {
        self.local_state.machine_id()
    }

    /// Change the saved settings: applied to a copy, saved, and only then
    /// adopted, so a failed save leaves memory and disk in agreement.
    fn update_local(&mut self, change: impl FnOnce(&mut local_state::LocalState)) -> Result<(), String> {
        let mut next = self.local_state.clone();
        change(&mut next);
        next.save(&self.config_dir)?;
        self.local_state = next;
        Ok(())
    }
}

struct UpdateStore {
    update: tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>,
    version: std::sync::Mutex<Option<String>>,
    notes: std::sync::Mutex<Option<String>>,
}

impl UpdateStore {
    fn new() -> Self {
        Self {
            update: tokio::sync::Mutex::new(None),
            version: std::sync::Mutex::new(None),
            notes: std::sync::Mutex::new(None),
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_oauth::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let log_dir = app.path().app_config_dir().unwrap_or_default();
            crate::logger::init(log_dir.join("zen-sync.log"));
            crate::zslog!("=== Zen Sync {} starting ===", app.package_info().version);

            let config_dir = app.path().app_config_dir().unwrap_or_default();
            let ls = local_state::LocalState::load(&config_dir);

            let has_token = github::has_stored_token();
            let state = Arc::new(Mutex::new(AppState {
                github_client: None,
                local_state: ls,
                config_dir,
                reconnecting: has_token,
                encryption: Encryption::Unknown,
            }));
            app.manage(state.clone());
            app.manage(OperationLock(tokio::sync::Mutex::new(())));

            let update_store = Arc::new(UpdateStore::new());
            app.manage(update_store.clone());

            // Sync OS autostart state to persisted preference (defaults to false on first run)
            {
                use tauri_plugin_autostart::ManagerExt;
                let autostart_enabled = state.lock().unwrap().local_state.autostart_enabled;
                if autostart_enabled {
                    let _ = app.autolaunch().enable();
                } else {
                    let _ = app.autolaunch().disable();
                }
            }

            let window = app.get_webview_window("main")
                .ok_or("main window not found")?;
            #[cfg(windows)]
            round_corners(&window);

            // Show window on first run (no GitHub token yet)
            if !has_token {
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Restore GitHub client from keychain in background
            if has_token {
                let state_clone = state.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    restore_saved_sign_in(&state_clone, &app_handle).await;
                    state_clone.lock().unwrap().reconnecting = false;
                    let _ = app_handle.emit("github-restored", ());
                    // The window always appears, connected or not: the app
                    // has no tray, so a hidden window would be unreachable.
                    if let Some(w) = app_handle.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                });
            }

            // Update check: 5s after launch, then every 24h
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                loop {
                    check_for_updates(&app_handle, false).await;
                    tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            zen_check::is_zen_running,
            profile::detect_profile_path,
            get_status_cmd,
            connect_github_cmd,
            disconnect_github_cmd,
            backup_now_cmd,
            get_legacy_snapshot_count_cmd,
            get_backup_summary_cmd,
            get_snapshots_cmd,
            restore_snapshot_cmd,
            setup_encryption_cmd,
            unlock_encryption_cmd,
            change_passphrase_cmd,
            new_recovery_code_cmd,
            reset_encryption_cmd,
            set_machine_name_cmd,
            set_snapshot_count_cmd,
            set_sync_options_cmd,
            set_autostart_cmd,
            get_extensions_with_selection_cmd,
            set_extension_selection_cmd,
            get_prefs_with_selection_cmd,
            set_pref_selection_cmd,
            open_log_cmd,
            get_log_cmd,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running zen-sync");
}

/// Restore the saved GitHub sign-in, retrying while it fails for reasons
/// other than GitHub rejecting it, e.g. when Windows starts the app at login
/// before the network is up. The window is shown after the first failure so
/// the user sees the app reconnecting instead of nothing.
async fn restore_saved_sign_in(state: &Arc<Mutex<AppState>>, app: &tauri::AppHandle) {
    for attempt in 1..=RECONNECT_ATTEMPTS {
        let restored = match github::GitHubClient::from_keychain().await {
            Ok(Some(client)) => {
                let checked = check_encryption(&client).await;
                checked.map(|encryption| Some((client, encryption)))
            }
            other => other.map(|_| None),
        };
        match restored {
            Ok(Some((client, encryption))) => {
                let mut s = state.lock().unwrap();
                s.github_client = Some(Arc::new(client));
                s.encryption = encryption;
                crate::zslog!("[app] GitHub client restored from keychain ({})", s.encryption.label());
                return;
            }
            Ok(None) => {
                crate::zslog!("[app] saved GitHub sign-in is no longer valid");
                return;
            }
            Err(e) => {
                crate::zslog!("[app] GitHub restore failed (attempt {attempt}): {e}");
                if attempt == 1 {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                if attempt < RECONNECT_ATTEMPTS {
                    tokio::time::sleep(RECONNECT_INTERVAL).await;
                }
            }
        }
    }
}

/// Frameless windows get square corners by default. Windows 11 rounds them on
/// request; older versions reject the attribute and stay square.
#[cfg(windows)]
fn round_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    let Ok(hwnd) = window.hwnd() else { return };
    // SAFETY: `hwnd` is our live window and the value matches the attribute's size.
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &DWMWCP_ROUND as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&DWMWCP_ROUND) as u32,
        )
    };
    if let Err(e) = result {
        crate::zslog!("[app] rounded corners unavailable: {e}");
    }
}

// ── Update handling ───────────────────────────────────────────────────────────

/// Portable builds have no installation for the updater to replace.
const PORTABLE: bool = cfg!(feature = "portable");
const RELEASES_URL: &str = "https://github.com/PixelPusher247/zen-sync/releases";

async fn check_for_updates(app: &tauri::AppHandle, manual: bool) {
    use tauri_plugin_notification::NotificationExt;

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[updater] init error: {e}");
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            if manual {
                let _ = app
                    .notification()
                    .builder()
                    .title("Zen Sync is up to date")
                    .body("You're running the latest version.")
                    .show();
            }
            return;
        }
        Err(e) => {
            eprintln!("[updater] check error: {e}");
            return;
        }
    };

    let version = update.version.clone();
    let notes = update.body.clone().unwrap_or_default();

    let store = app.state::<Arc<UpdateStore>>();
    *store.update.lock().await = Some(update);
    *store.version.lock().unwrap() = Some(version.clone());
    *store.notes.lock().unwrap() = Some(notes.clone());

    if manual {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }

    let _ = app.emit(
        "update-available",
        serde_json::json!({ "version": version, "notes": notes, "portable": PORTABLE }),
    );
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppStatusPayload {
    connected: bool,
    username: Option<String>,
    machine_name: String,
    /// The id this device's snapshots are filed under, so the UI can mark them.
    machine_id: String,
    last_backup_at: Option<String>,
    snapshot_count: u8,
    autostart_enabled: bool,
    sync_options: bundle::SyncOptions,
    /// A saved sign-in is still being restored (e.g. waiting for the network).
    reconnecting: bool,
    /// "unknown", "needs-setup", "locked" or "ready"; see `Encryption`.
    encryption: &'static str,
    /// With "needs-setup": setting up will delete backups from 0.5 or earlier.
    old_backups: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupEncryptionResult {
    /// Shown to the user once; never stored by the app.
    recovery_code: String,
    status: AppStatusPayload,
    /// Set if removing the old backups failed after setup succeeded.
    warning: Option<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfoPayload {
    pub index: u8,
    pub pushed_at: String,
    pub machine_name: String,
    pub size_mb: f32,
    pub is_current: bool,
    pub machine_id: String,
}

#[tauri::command]
fn get_status_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> AppStatusPayload {
    use tauri_plugin_autostart::ManagerExt;
    let s = state.lock().unwrap();
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    AppStatusPayload {
        connected: s.github_client.is_some(),
        username: s
            .github_client
            .as_ref()
            .map(|c| c.username.clone()),
        machine_name: s.local_state.machine_name.clone(),
        machine_id: s.machine_id(),
        last_backup_at: s.local_state.last_backup_at.clone(),
        snapshot_count: s.local_state.snapshot_count,
        autostart_enabled,
        sync_options: s.local_state.sync_options.clone(),
        reconnecting: s.reconnecting && s.github_client.is_none(),
        encryption: s.encryption.label(),
        old_backups: matches!(s.encryption, Encryption::NeedsSetup { old_backups: true }),
    }
}

#[tauri::command]
async fn connect_github_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<AppStatusPayload, String> {
    let client = github::GitHubClient::connect(&app).await?;
    let encryption = check_encryption(&client).await?;
    {
        let mut s = state.lock().unwrap();
        s.github_client = Some(Arc::new(client));
        s.encryption = encryption;
    }
    Ok(get_status_cmd(app, state))
}

#[tauri::command]
fn disconnect_github_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    github::remove_stored_token()?;
    // Signing out also forgets the key: reconnecting asks for the
    // passphrase again.
    keyfile::forget_local_key()?;
    let mut s = state.lock().unwrap();
    s.github_client = None;
    s.encryption = Encryption::Unknown;
    Ok(())
}

// ── Encryption ────────────────────────────────────────────────────────────────

fn connected_client(state: &Mutex<AppState>) -> Result<Arc<github::GitHubClient>, String> {
    state
        .lock()
        .unwrap()
        .github_client
        .clone()
        .ok_or_else(|| "Not connected to GitHub".to_string())
}

fn ready_key(state: &Mutex<AppState>) -> Result<crypto::DataKey, String> {
    match &state.lock().unwrap().encryption {
        Encryption::Ready(key) => Ok(key.clone()),
        _ => Err("Encryption isn't set up on this device yet.".into()),
    }
}

const KEY_FILE_RACE: &str =
    "Encryption was just set up from another device. Enter the passphrase you chose there.";

/// First device: create the snapshot key, protect it with the passphrase
/// and a new recovery code, and remove backups made with the old key.
#[tauri::command]
async fn setup_encryption_cmd(
    passphrase: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    lock: tauri::State<'_, OperationLock>,
) -> Result<SetupEncryptionResult, String> {
    let _operation = lock.0.try_lock().map_err(|_| OPERATION_BUSY)?;
    keyfile::check_passphrase(&passphrase)?;
    let client = connected_client(&state)?;
    if client.read_key_file().await?.is_some() {
        state.lock().unwrap().encryption = Encryption::Locked;
        return Err(KEY_FILE_RACE.into());
    }

    let key = crypto::DataKey::generate();
    let code = keyfile::RecoveryCode::generate();
    let recovery_code = code.display();
    let file = {
        let key = key.clone();
        blocking(move || keyfile::KeyFile::create(&key, &passphrase, &code)).await?
    };
    // Kept locally first: if the upload then fails, the stray local copy is
    // simply replaced next time.
    keyfile::save_local_key(&key)?;
    if !client.write_key_file(&file, None).await? {
        state.lock().unwrap().encryption = Encryption::Locked;
        return Err(KEY_FILE_RACE.into());
    }
    crate::zslog!("[encryption] set up, key {}", key.id_hex());
    state.lock().unwrap().encryption = Encryption::Ready(key);

    // Starting fresh: snapshots made with the old key could never be
    // restored, and the old key must not stay readable in the repository.
    let warning = match client.delete_all_backups(false).await {
        Ok(()) => None,
        Err(e) => {
            crate::zslog!("[encryption] removing old backups failed: {e}");
            Some(format!(
                "Encryption is set up, but removing backups made with the old key failed: {e}"
            ))
        }
    };
    Ok(SetupEncryptionResult {
        recovery_code,
        status: get_status_cmd(app, state),
        warning,
    })
}

/// Another device: unlock the snapshot key with the passphrase or the
/// recovery code.
#[tauri::command]
async fn unlock_encryption_cmd(
    secret: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<AppStatusPayload, String> {
    let client = connected_client(&state)?;
    let Some((file, _)) = client.read_key_file().await? else {
        let old_backups = client.has_backups_without_key_file().await?;
        state.lock().unwrap().encryption = Encryption::NeedsSetup { old_backups };
        return Err("Encryption isn't set up yet. Choose a passphrase to set it up.".into());
    };
    let key = blocking(move || file.unlock(&secret))
        .await?
        .ok_or("That passphrase or recovery code doesn't match.")?;
    keyfile::save_local_key(&key)?;
    state.lock().unwrap().encryption = Encryption::Ready(key);
    Ok(get_status_cmd(app, state))
}

/// Re-wrap the key under a new passphrase, after checking the current one.
#[tauri::command]
async fn change_passphrase_cmd(
    current: String,
    new_passphrase: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    keyfile::check_passphrase(&new_passphrase)?;
    let key = ready_key(&state)?;
    let client = connected_client(&state)?;
    let (file, sha) = client
        .read_key_file()
        .await?
        .ok_or("The key file is missing from the backup repository.")?;
    let updated = blocking(move || {
        match file.unlock_with_passphrase(&current)? {
            Some(k) if k.id_hex() == key.id_hex() => file.with_passphrase(&key, &new_passphrase),
            Some(_) => Err("The key file changed on another device. Restart Zen Sync and try again.".into()),
            None => Err("The current passphrase is wrong.".into()),
        }
    })
    .await?;
    if !client.write_key_file(&updated, Some(&sha)).await? {
        return Err("The key file changed on GitHub meanwhile. Try again.".into());
    }
    crate::zslog!("[encryption] passphrase changed");
    Ok(())
}

/// Replace the recovery code, after checking the passphrase. Returns the new
/// code, which is shown once and not stored.
#[tauri::command]
async fn new_recovery_code_cmd(
    passphrase: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let key = ready_key(&state)?;
    let client = connected_client(&state)?;
    let (file, sha) = client
        .read_key_file()
        .await?
        .ok_or("The key file is missing from the backup repository.")?;
    let code = keyfile::RecoveryCode::generate();
    let shown = code.display();
    let updated = blocking(move || {
        match file.unlock_with_passphrase(&passphrase)? {
            Some(k) if k.id_hex() == key.id_hex() => file.with_recovery_code(&key, &code),
            Some(_) => Err("The key file changed on another device. Restart Zen Sync and try again.".into()),
            None => Err("The passphrase is wrong.".into()),
        }
    })
    .await?;
    if !client.write_key_file(&updated, Some(&sha)).await? {
        return Err("The key file changed on GitHub meanwhile. Try again.".into());
    }
    crate::zslog!("[encryption] recovery code replaced");
    Ok(shown)
}

/// Both passphrase and recovery code forgotten: delete every backup and the
/// key file, so encryption can be set up again from scratch.
#[tauri::command]
async fn reset_encryption_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    lock: tauri::State<'_, OperationLock>,
) -> Result<AppStatusPayload, String> {
    let _operation = lock.0.try_lock().map_err(|_| OPERATION_BUSY)?;
    let client = connected_client(&state)?;
    client.delete_all_backups(true).await?;
    keyfile::forget_local_key()?;
    crate::zslog!("[encryption] reset: all backups and the key file deleted");
    state.lock().unwrap().encryption = Encryption::NeedsSetup { old_backups: false };
    Ok(get_status_cmd(app, state))
}

const OPERATION_BUSY: &str = "Another backup or restore is still running. Wait for it to finish.";

#[tauri::command]
async fn backup_now_cmd(
    delete_legacy: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    lock: tauri::State<'_, OperationLock>,
) -> Result<(), String> {
    let _operation = lock.0.try_lock().map_err(|_| OPERATION_BUSY)?;
    if zen_check::is_running() {
        return Err(
            "Zen Browser is open. Close it completely before backing up.".into(),
        );
    }

    let (client, machine_name, machine_id, max_snapshots, selection) = {
        let s = state.lock().unwrap();
        let c = s.github_client.clone().ok_or("Not connected to GitHub")?;
        (
            c,
            s.local_state.machine_name.clone(),
            s.machine_id(),
            s.local_state.snapshot_count,
            s.selection(),
        )
    };
    let key = ready_key(&state)?;

    let app_p = app.clone();
    let pushed_at = sync::backup(
        sync::Remote { client: &client, key: &key },
        &machine_name,
        &machine_id,
        max_snapshots,
        selection,
        delete_legacy,
        move |msg| {
            let _ = app_p.emit("sync-progress", msg);
        },
    )
    .await?;

    {
        let mut s = state.lock().unwrap();
        if let Err(e) = s.update_local(|ls| ls.last_backup_at = Some(pushed_at.clone())) {
            // The backup itself is stored; only the local "last backup" time is stale.
            crate::zslog!("[app] couldn't record last backup time: {e}");
            s.local_state.last_backup_at = Some(pushed_at);
        }
    }
    let _ = app.emit("sync-updated", ());
    Ok(())
}

/// Number of full-profile snapshots left over from zen-sync 0.1.x.
#[tauri::command]
async fn get_legacy_snapshot_count_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<usize, String> {
    let client = {
        let s = state.lock().unwrap();
        s.github_client.clone().ok_or("Not connected to GitHub")?
    };
    let legacy = client.find_legacy_snapshots().await?;
    // A leftover metadata file with no assets still needs cleaning up.
    Ok(if legacy.is_empty() { 0 } else { legacy.asset_ids.len().max(1) })
}

#[tauri::command]
fn get_backup_summary_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<bundle::BackupSummary, String> {
    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found. Is Zen Browser installed?")?;
    let selection = state.lock().unwrap().selection();
    bundle::summarize(&profile_dir, &selection)
}

#[tauri::command]
async fn get_snapshots_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<SnapshotInfoPayload>, String> {
    let client = {
        let s = state.lock().unwrap();
        s.github_client.clone().ok_or("Not connected to GitHub")?
    };

    let Some(metadata) = client.read_metadata().await? else {
        return Ok(Vec::new());
    };

    // Collect all snapshots from all machines, newest first across machines.
    let mut infos: Vec<SnapshotInfoPayload> = metadata
        .metadata
        .machines
        .iter()
        .flat_map(|m| {
            let current = m.current_index;
            m.snapshots.iter().map(move |s| SnapshotInfoPayload {
                index: s.index,
                pushed_at: s.pushed_at.clone(),
                machine_name: s.machine_name.clone(),
                size_mb: s.size_bytes as f32 / 1_048_576.0,
                is_current: s.index == current,
                machine_id: m.machine_id.clone(),
            })
        })
        .collect();

    // Sort newest-first by pushed_at
    infos.sort_by(|a, b| b.pushed_at.cmp(&a.pushed_at));
    Ok(infos)
}

#[tauri::command]
async fn restore_snapshot_cmd(
    index: u8,
    machine_id: String,
    pushed_at: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    lock: tauri::State<'_, OperationLock>,
) -> Result<bundle::RestoreReport, String> {
    let _operation = lock.0.try_lock().map_err(|_| OPERATION_BUSY)?;
    if zen_check::is_running() {
        return Err(
            "Zen Browser is open. Close it completely before restoring.".into(),
        );
    }

    let (client, selection, safety_root) = {
        let s = state.lock().unwrap();
        let c = s.github_client.clone().ok_or("Not connected to GitHub")?;
        (c, s.selection(), s.config_dir.join("restore-backups"))
    };
    let key = ready_key(&state)?;

    let app_p = app.clone();
    let report = sync::restore(
        sync::Remote { client: &client, key: &key },
        &machine_id,
        index,
        pushed_at.as_deref(),
        selection,
        safety_root,
        move |msg| {
            let _ = app_p.emit("sync-progress", msg);
        },
    )
    .await?;

    let _ = app.emit("sync-updated", ());
    Ok(report)
}

#[tauri::command]
fn set_machine_name_cmd(
    name: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("The device name can't be empty.".into());
    }
    let mut s = state.lock().unwrap();
    // Snapshots are filed under an id derived from the name. Keep the current
    // id through any rename, so snapshots made before it (or one uploading
    // right now, or ones from a reinstall) stay with this device.
    let id = s.machine_id();
    s.update_local(|ls| {
        if ls.machine_id.is_none() {
            ls.machine_id = Some(id);
        }
        ls.machine_name = name;
    })
}

#[tauri::command]
fn set_snapshot_count_cmd(
    count: u8,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    state
        .lock()
        .unwrap()
        .update_local(|ls| ls.snapshot_count = count.clamp(1, 10))
}

#[tauri::command]
fn set_sync_options_cmd(
    options: bundle::SyncOptions,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    state.lock().unwrap().update_local(|ls| ls.sync_options = options)
}


#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInfoWithSelection {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub icon_url: Option<String>,
    pub synced: bool,
    /// Size of the extension's storage.local folder in bytes (0 if it has none).
    pub storage_bytes: u64,
    pub password_manager: bool,
}

#[tauri::command]
fn get_extensions_with_selection_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<ExtensionInfoWithSelection>, String> {
    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found. Is Zen Browser installed?")?;
    let list = extensions::list_extensions(&profile_dir)?;
    let prefs_content = std::fs::read_to_string(profile_dir.join("prefs.js")).unwrap_or_default();
    let uuids = ext_storage::uuid_map(&prefs_content).unwrap_or_default();
    let overrides = state.lock().unwrap().local_state.extension_overrides.clone();
    let result = list
        .into_iter()
        .map(|e| ExtensionInfoWithSelection {
            synced: extensions::is_selected(&overrides, &e.id, &e.name),
            storage_bytes: uuids
                .get(&e.id)
                .map(|uuid| fsutil::dir_size(&ext_storage::storage_local_dir(&profile_dir, uuid)))
                .unwrap_or(0),
            password_manager: extensions::is_password_manager(&e.id, &e.name),
            id: e.id,
            name: e.name,
            version: e.version,
            enabled: e.enabled,
            icon_url: e.icon_url,
        })
        .collect();
    Ok(result)
}

#[tauri::command]
fn set_extension_selection_cmd(
    ids: Vec<String>,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found. Is Zen Browser installed?")?;
    let installed = extensions::list_extensions(&profile_dir)?;
    state.lock().unwrap().update_local(|ls| {
        extensions::update_overrides(&mut ls.extension_overrides, &installed, &ids);
    })
}

/// One about:config pref this device could sync, for the review screen.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PrefWithSelection {
    pub name: String,
    /// The raw prefs.js value literal, shown as-is.
    pub value: String,
    pub synced: bool,
}

/// Every syncable pref in this profile, with the user's choice for each.
#[tauri::command]
fn get_prefs_with_selection_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<PrefWithSelection>, String> {
    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found. Is Zen Browser installed?")?;
    let content = std::fs::read_to_string(profile_dir.join("prefs.js"))
        .map_err(|e| format!("Could not read prefs.js: {e}"))?;
    let overrides = state.lock().unwrap().local_state.pref_overrides.clone();
    Ok(prefs::syncable(&content, &sine::declared_prefs(&profile_dir))
        .into_iter()
        .map(|(name, value)| PrefWithSelection {
            synced: prefs::is_selected(&overrides, &name),
            name,
            value,
        })
        .collect())
}

#[tauri::command]
fn set_pref_selection_cmd(
    names: Vec<String>,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found. Is Zen Browser installed?")?;
    let content = std::fs::read_to_string(profile_dir.join("prefs.js"))
        .map_err(|e| format!("Could not read prefs.js: {e}"))?;
    let candidates: std::collections::BTreeSet<String> =
        prefs::syncable(&content, &sine::declared_prefs(&profile_dir))
            .into_keys()
            .collect();
    state.lock().unwrap().update_local(|ls| {
        prefs::update_overrides(&mut ls.pref_overrides, &candidates, &names);
    })
}

#[tauri::command]
async fn set_autostart_cmd(
    enabled: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch()
            .enable()
            .map_err(|e| format!("Failed to enable autostart: {e}"))?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|e| format!("Failed to disable autostart: {e}"))?;
    }
    // The OS setting is what counts (get_status reads it); the saved copy only
    // re-applies it at startup, so a failed save is logged, not reported.
    if let Err(e) = state.lock().unwrap().update_local(|ls| ls.autostart_enabled = enabled) {
        crate::zslog!("[autostart] couldn't save preference: {e}");
    }
    crate::zslog!("[autostart] set to {enabled}");
    Ok(())
}

#[tauri::command]
async fn open_log_cmd(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::WebviewWindowBuilder;
    if let Some(w) = app.get_webview_window("log") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, "log", tauri::WebviewUrl::App("index.html".into()))
        .title("Zen Sync — Log")
        .inner_size(700.0, 500.0)
        .min_inner_size(400.0, 300.0)
        .resizable(true)
        .center()
        .build()
        .map_err(|e| format!("Cannot open log window: {e}"))?;
    Ok(())
}

#[tauri::command]
fn get_log_cmd() -> Result<String, String> {
    let path = match logger::path() {
        Some(p) => p.to_path_buf(),
        None => return Err("Logging is not initialised".into()),
    };
    std::fs::read_to_string(&path).map_err(|e| format!("Cannot read log: {e}"))
}

#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    store: tauri::State<'_, Arc<UpdateStore>>,
) -> Result<(), String> {
    if PORTABLE {
        // Running the installer would add a second, installed copy; send the
        // user to the release page to download the new portable exe instead.
        use tauri_plugin_opener::OpenerExt;
        let version = store.version.lock().unwrap().clone().ok_or("No pending update")?;
        return app
            .opener()
            .open_url(format!("{RELEASES_URL}/tag/v{version}"), None::<&str>)
            .map_err(|e| format!("Failed to open browser: {e}"));
    }

    // Cloned rather than taken, so a failed download can be retried.
    let update = store
        .update
        .lock()
        .await
        .clone()
        .ok_or("No pending update")?;
    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}
