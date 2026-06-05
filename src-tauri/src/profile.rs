use std::path::{Path, PathBuf};

pub const SYNC_FILES: &[&str] = &[
    "places.sqlite",
    "prefs.js",
    "extensions.json",
    "zen-themes.json",
    "zen-keyboard-shortcuts.json",
    "zen-sessions.jsonlz4",
    "zen-live-folders.jsonlz4",
    "chrome/zen-themes.css",
    "containers.json",
];

#[tauri::command]
pub fn detect_profile_path() -> Result<String, String> {
    find_zen_profile()
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| "Zen profile folder not found. Is Zen Browser installed?".into())
}

pub fn find_zen_profile() -> Option<PathBuf> {
    // Windows: %APPDATA%\zen\Profiles
    let profiles_dir = dirs::data_dir()?.join("zen\\Profiles");
    crate::zslog!("[profile] profiles_dir = {}", profiles_dir.display());

    let from_ini = read_active_profile_from_ini(&profiles_dir);
    crate::zslog!("[profile] profiles.ini result = {:?}", from_ini);

    let fallback = first_release_profile(&profiles_dir);
    crate::zslog!("[profile] release-folder fallback = {:?}", fallback);

    let result = from_ini.filter(|p| p.is_dir()).or(fallback);
    crate::zslog!("[profile] resolved profile = {:?}", result);
    result
}

fn read_active_profile_from_ini(profiles_dir: &Path) -> Option<PathBuf> {
    let zen_dir = profiles_dir.parent()?;
    let ini_path = zen_dir.join("profiles.ini");
    let content = std::fs::read_to_string(ini_path).ok()?;

    let mut in_install_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_install_section = line.to_lowercase().starts_with("[install");
        } else if in_install_section {
            if let Some(rel_path) = line.strip_prefix("Default=") {
                return Some(zen_dir.join(rel_path));
            }
        }
    }
    None
}

fn first_release_profile(profiles_dir: &PathBuf) -> Option<PathBuf> {
    std::fs::read_dir(profiles_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .find(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.contains("release")
        })
        .map(|e| e.path())
}
