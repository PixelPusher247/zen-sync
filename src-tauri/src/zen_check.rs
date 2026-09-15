use sysinfo::{ProcessRefreshKind, System, UpdateKind};

/// Returns true if the Zen browser process is running under the current user.
/// zen-sync hard-blocks all backup/restore operations while Zen is open to
/// prevent SQLite corruption and browser state weirdness.
pub fn is_running() -> bool {
    // Only names and owners: the UI polls this every few seconds.
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessRefreshKind::new().with_user(UpdateKind::OnlyIfNotSet));

    let my_pid = sysinfo::Pid::from(std::process::id() as usize);
    let my_uid = sys.process(my_pid).and_then(|p| p.user_id()).cloned();

    sys.processes().values().any(|p| {
        if p.pid() == my_pid {
            return false;
        }
        let name = p.name().to_lowercase();
        let name = name.trim_end_matches(".exe");
        let is_zen = name == "zen" || name == "zen browser" || name.starts_with("zen-");
        let is_subprocess = name.contains("helper") || name.contains("crashreporter");
        if !is_zen || is_subprocess {
            return false;
        }
        match (&my_uid, p.user_id()) {
            (Some(mine), Some(theirs)) => mine == theirs,
            (None, None) => true,
            _ => false,
        }
    })
}

/// Async so the process scan stays off the main thread.
#[tauri::command]
pub async fn is_zen_running() -> bool {
    tauri::async_runtime::spawn_blocking(is_running)
        .await
        .unwrap_or(false)
}
