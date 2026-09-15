mod bundle;
mod crypto;
mod ext_storage;
mod extensions;
mod fsutil;
mod github;
mod local_state;
pub mod logger;
mod prefs;
mod profile;
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
}

impl AppState {
    fn machine_id(&self) -> String {
        // Stable identifier derived from machine name (URL-safe, lowercase)
        self.local_state
            .machine_name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
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

            let state = Arc::new(Mutex::new(AppState {
                github_client: None,
                local_state: ls,
                config_dir,
            }));
            app.manage(state.clone());

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

            // Show window on first run (no GitHub token yet)
            if !github::has_stored_token() {
                window.show().unwrap();
                let _ = window.set_focus();
            }

            // Restore GitHub client from keychain in background
            {
                let state_clone = state.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    match github::GitHubClient::from_keychain().await {
                        Ok(Some(client)) => {
                            state_clone.lock().unwrap().github_client =
                                Some(Arc::new(client));
                            crate::zslog!("[app] GitHub client restored from keychain");
                            let _ = app_handle.emit("github-restored", ());
                            // Show window after successful restore if connected
                            if let Some(w) = app_handle.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        Ok(None) => {
                            crate::zslog!("[app] no stored GitHub token");
                        }
                        Err(e) => crate::zslog!("[app] GitHub restore failed: {e}"),
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
            set_machine_name_cmd,
            set_snapshot_count_cmd,
            set_autostart_cmd,
            get_extensions_with_selection_cmd,
            set_extension_selection_cmd,
            open_log_cmd,
            get_log_cmd,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running zen-sync");
}

// ── Update handling ───────────────────────────────────────────────────────────

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
        serde_json::json!({ "version": version, "notes": notes }),
    );
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppStatusPayload {
    connected: bool,
    username: Option<String>,
    machine_name: String,
    last_backup_at: Option<String>,
    snapshot_count: u8,
    autostart_enabled: bool,
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
        last_backup_at: s.local_state.last_backup_at.clone(),
        snapshot_count: s.local_state.snapshot_count,
        autostart_enabled,
    }
}

#[tauri::command]
async fn connect_github_cmd(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<AppStatusPayload, String> {
    let client = github::GitHubClient::connect(&app).await?;
    {
        let mut s = state.lock().unwrap();
        s.github_client = Some(Arc::new(client));
    }
    Ok(get_status_cmd(app, state))
}

#[tauri::command]
fn disconnect_github_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    github::remove_stored_token()?;
    state.lock().unwrap().github_client = None;
    Ok(())
}

#[tauri::command]
async fn backup_now_cmd(
    delete_legacy: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    if zen_check::is_zen_running() {
        return Err(
            "Zen Browser is open. Close it completely before backing up.".into(),
        );
    }

    let (client, machine_name, machine_id, max_snapshots, overrides, config_dir) = {
        let s = state.lock().unwrap();
        let c = s.github_client.clone().ok_or("Not connected to GitHub")?;
        (
            c,
            s.local_state.machine_name.clone(),
            s.machine_id(),
            s.local_state.snapshot_count,
            s.local_state.extension_overrides.clone(),
            s.config_dir.clone(),
        )
    };

    let app_p = app.clone();
    let pushed_at = sync::backup(
        &client,
        &machine_name,
        &machine_id,
        max_snapshots,
        overrides,
        delete_legacy,
        move |msg| {
            let _ = app_p.emit("sync-progress", msg);
        },
    )
    .await?;

    {
        let mut s = state.lock().unwrap();
        s.local_state.last_backup_at = Some(pushed_at);
        let _ = s.local_state.save(&config_dir);
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
    let overrides = state.lock().unwrap().local_state.extension_overrides.clone();
    Ok(bundle::summarize(&profile_dir, &overrides))
}

#[tauri::command]
async fn get_snapshots_cmd(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<SnapshotInfoPayload>, String> {
    let client = {
        let s = state.lock().unwrap();
        s.github_client.clone().ok_or("Not connected to GitHub")?
    };

    let metadata = client
        .read_metadata()
        .await?
        .ok_or("No backup data found yet. Back up from any device first.")?;

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
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<bundle::RestoreReport, String> {
    if zen_check::is_zen_running() {
        return Err(
            "Zen Browser is open. Close it completely before restoring.".into(),
        );
    }

    let (client, overrides, safety_root) = {
        let s = state.lock().unwrap();
        let c = s.github_client.clone().ok_or("Not connected to GitHub")?;
        (
            c,
            s.local_state.extension_overrides.clone(),
            s.config_dir.join("restore-backups"),
        )
    };

    let app_p = app.clone();
    let report = sync::restore(&client, &machine_id, index, overrides, safety_root, move |msg| {
        let _ = app_p.emit("sync-progress", msg);
    })
    .await?;

    let _ = app.emit("sync-updated", ());
    Ok(report)
}

#[tauri::command]
fn set_machine_name_cmd(
    name: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    s.local_state.machine_name = name;
    let config_dir = s.config_dir.clone();
    s.local_state.save(&config_dir)
}

#[tauri::command]
fn set_snapshot_count_cmd(
    count: u8,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    s.local_state.snapshot_count = count.clamp(1, 10);
    let config_dir = s.config_dir.clone();
    s.local_state.save(&config_dir)
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
    let mut s = state.lock().unwrap();
    extensions::update_overrides(&mut s.local_state.extension_overrides, &installed, &ids);
    let config_dir = s.config_dir.clone();
    s.local_state.save(&config_dir)
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
    {
        let mut s = state.lock().unwrap();
        s.local_state.autostart_enabled = enabled;
        let config_dir = s.config_dir.clone();
        let _ = s.local_state.save(&config_dir);
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
    let update = store
        .update
        .lock()
        .await
        .take()
        .ok_or("No pending update")?;
    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}
