use std::path::{Path, PathBuf};

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
    let content = std::fs::read_to_string(zen_dir.join("profiles.ini")).ok()?;
    let candidates = default_profiles(&content)
        .into_iter()
        .map(|rel| zen_dir.join(rel))
        .filter(|p| p.is_dir());
    most_recently_used(candidates)
}

/// Profiles profiles.ini marks as a default: each `[Install…]` section's
/// `Default=` (one per Zen install, e.g. release and Twilight side by side),
/// or else a `[Profile…]` section with `Default=1`.
fn default_profiles(ini: &str) -> Vec<String> {
    #[derive(PartialEq)]
    enum Section {
        Install,
        Profile,
        Other,
    }
    let mut section = Section::Other;
    let mut installs = Vec::new();
    let mut profile_path: Option<String> = None;
    let mut profile_is_default = false;
    let mut profile_defaults = Vec::new();
    let mut finish_profile = |path: &mut Option<String>, is_default: &mut bool| {
        if let (Some(p), true) = (path.take(), *is_default) {
            profile_defaults.push(p);
        }
        *is_default = false;
    };

    for line in ini.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            finish_profile(&mut profile_path, &mut profile_is_default);
            let lower = line.to_lowercase();
            section = if lower.starts_with("[install") {
                Section::Install
            } else if lower.starts_with("[profile") {
                Section::Profile
            } else {
                Section::Other
            };
        } else if let Some(value) = line.strip_prefix("Default=") {
            match section {
                Section::Install => installs.push(value.to_string()),
                Section::Profile => profile_is_default = value == "1",
                Section::Other => {}
            }
        } else if let (Section::Profile, Some(value)) = (&section, line.strip_prefix("Path=")) {
            profile_path = Some(value.to_string());
        }
    }
    finish_profile(&mut profile_path, &mut profile_is_default);
    if installs.is_empty() { profile_defaults } else { installs }
}

/// Of several profiles, the one Zen wrote to last (by prefs.js), so the
/// install the user actually uses wins over a leftover one.
fn most_recently_used(candidates: impl Iterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.max_by_key(|p| {
        std::fs::metadata(p.join("prefs.js"))
            .and_then(|m| m.modified())
            .ok()
    })
}

fn first_release_profile(profiles_dir: &Path) -> Option<PathBuf> {
    let candidates = std::fs::read_dir(profiles_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| e.file_name().to_string_lossy().to_lowercase().contains("release"))
        .map(|e| e.path());
    most_recently_used(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_every_install_default() {
        let ini = "[Profile0]\nName=Default (release)\nPath=Profiles/a.Default (release)\nDefault=1\n\n[InstallAAA]\nDefault=Profiles/a.Default (release)\nLocked=1\n\n[InstallBBB]\nDefault=Profiles/b.Default (twilight)\n";
        assert_eq!(
            default_profiles(ini),
            vec!["Profiles/a.Default (release)", "Profiles/b.Default (twilight)"]
        );
    }

    #[test]
    fn falls_back_to_profile_marked_default() {
        let ini = "[Profile0]\nPath=Profiles/old\n\n[Profile1]\nPath=Profiles/main\nDefault=1\n";
        assert_eq!(default_profiles(ini), vec!["Profiles/main"]);
    }
}
