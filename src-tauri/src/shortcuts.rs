//! Zen's customised keyboard shortcuts.
//!
//! Zen writes every shortcut it knows — its own (workspaces, compact mode,
//! split view) and Firefox's — to `zen-keyboard-shortcuts.json` in the profile
//! root, as `{"shortcuts": [...]}`. Nothing in it is specific to a device.
//!
//! The schema version is *not* in the file: it lives in the pref
//! `zen.keyboard.shortcuts.version`. On startup Zen migrates the file forward
//! when the pref is behind the build's own version, and rebuilds the shortcuts
//! from defaults when the pref is ahead of it. So the file and the pref have to
//! travel together, and the pref must never claim a version this Zen predates.

use std::path::{Path, PathBuf};

use crate::prefs;

pub const FILE: &str = "zen-keyboard-shortcuts.json";
pub const VERSION_PREF: &str = "zen.keyboard.shortcuts.version";

pub fn path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(FILE)
}

/// The stored shortcuts, if this profile has a readable shortcuts file.
pub fn read(profile_dir: &Path) -> Option<serde_json::Value> {
    let path = path(profile_dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            if path.exists() {
                crate::zslog!("[shortcuts] could not read {}: {e}", path.display());
            }
            return None;
        }
    };
    match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
        Ok(value) => Some(value),
        Err(e) => {
            crate::zslog!("[shortcuts] {} is not valid JSON: {e}", path.display());
            None
        }
    }
}

/// How many shortcuts a stored document holds.
pub fn count(value: &serde_json::Value) -> usize {
    value
        .get("shortcuts")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

/// The schema version recorded for this profile, if the pref is set.
pub fn version(prefs_content: &str) -> Option<i64> {
    prefs::get_raw(prefs_content, VERSION_PREF)?.trim().parse().ok()
}

/// The version a restore should record. Claiming a version newer than the local
/// Zen understands makes it throw the restored file away and rebuild from
/// defaults, so the lower of the two wins; an older version is safe, because
/// Zen migrates the file forward from there.
pub fn version_to_write(snapshot: Option<i64>, local: Option<i64>) -> Option<i64> {
    match (snapshot, local) {
        (Some(snapshot), Some(local)) => Some(snapshot.min(local)),
        (snapshot, _) => snapshot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reads_shortcuts_and_counts_them() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            r#"{"shortcuts":[{"id":"zen-workspace-switch-1","key":"1","modifiers":{"alt":true}},
                             {"id":"zen-compact-mode-toggle","key":"S","modifiers":{"accel":true}}]}"#,
        )
        .unwrap();
        let value = read(dir.path()).unwrap();
        assert_eq!(count(&value), 2);
        assert_eq!(value["shortcuts"][0]["id"], "zen-workspace-switch-1");
    }

    #[test]
    fn missing_or_broken_file_reads_as_none() {
        let dir = tempdir().unwrap();
        assert!(read(dir.path()).is_none());
        std::fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert!(read(dir.path()).is_none());
    }

    #[test]
    fn version_comes_from_the_pref() {
        assert_eq!(version("user_pref(\"zen.keyboard.shortcuts.version\", 20);"), Some(20));
        assert_eq!(version("user_pref(\"browser.tabs.warnOnClose\", false);"), None);
    }

    #[test]
    fn restore_never_records_a_version_newer_than_this_zen() {
        // Snapshot from a newer Zen: stay at what this build migrates to.
        assert_eq!(version_to_write(Some(20), Some(18)), Some(18));
        // Snapshot from an older Zen: let this build migrate the file forward.
        assert_eq!(version_to_write(Some(18), Some(20)), Some(18));
        // Nothing recorded here yet, or nothing in the snapshot.
        assert_eq!(version_to_write(Some(20), None), Some(20));
        assert_eq!(version_to_write(None, Some(20)), None);
    }
}
