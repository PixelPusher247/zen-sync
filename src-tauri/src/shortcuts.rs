//! Zen's customised keyboard shortcuts.
//!
//! Since Zen 1.24 its shortcuts are Firefox's customisable keys: Zen's own
//! (workspaces, compact mode, split view) and Firefox's are `<key>` elements,
//! and rebinding one, in Zen's Keyboard Shortcuts settings or in
//! `about:keyboard`, goes through Firefox's CustomKeys. That keeps only the
//! keys you changed, in `customKeys.json` in the profile root:
//! `{"<key id>": {"modifiers": "accel,shift", "key": "Y"}}`, with `keycode` in
//! place of `key` for keys such as F1, and `{}` for a key you cleared. A
//! missing file means every key is at its default. Nothing in it is specific
//! to a device.
//!
//! Older Zen kept every shortcut in its own file, see [`legacy`]. Zen 1.24
//! moves that file's customisations into `customKeys.json` on its first start
//! and deletes it.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

pub const FILE: &str = "customKeys.json";

pub fn path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(FILE)
}

/// The keys this profile customised, by key id. Empty when the file doesn't
/// exist, because then every key is at its default; None when it exists but
/// can't be read.
pub fn read(profile_dir: &Path) -> Option<Map<String, Value>> {
    let path = path(profile_dir);
    match read_json(&path)? {
        None => Some(Map::new()),
        Some(Value::Object(keys)) => Some(keys),
        Some(_) => {
            crate::zslog!("[shortcuts] {} is not a JSON object", path.display());
            None
        }
    }
}

/// A JSON document: Some(None) when the file doesn't exist, None when it
/// can't be read or parsed (logged here).
fn read_json(path: &Path) -> Option<Option<Value>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(None),
        Err(e) => {
            crate::zslog!("[shortcuts] could not read {}: {e}", path.display());
            return None;
        }
    };
    match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
        Ok(value) => Some(Some(value)),
        Err(e) => {
            crate::zslog!("[shortcuts] {} is not valid JSON: {e}", path.display());
            None
        }
    }
}

/// Zen's own shortcuts file, used up to Zen 1.23.
///
/// Zen wrote every shortcut it knew, its own and Firefox's, to
/// `zen-keyboard-shortcuts.json` in the profile root, as `{"shortcuts": [...]}`.
///
/// The schema version is *not* in the file: it lives in the pref
/// `zen.keyboard.shortcuts.version`. On startup Zen migrates the file forward
/// when the pref is behind the build's own version, and rebuilds the shortcuts
/// from defaults when the pref is ahead of it. So the file and the pref have to
/// travel together, and the pref must never claim a version this Zen predates.
///
/// Zen 1.24 reads the file once whatever its version, copies each shortcut
/// that differs from its default into `customKeys.json` unless that key is
/// already customised there, then deletes the file and clears the pref.
pub mod legacy {
    use std::path::{Path, PathBuf};

    use crate::prefs;

    pub const FILE: &str = "zen-keyboard-shortcuts.json";
    pub const VERSION_PREF: &str = "zen.keyboard.shortcuts.version";

    pub fn path(profile_dir: &Path) -> PathBuf {
        profile_dir.join(FILE)
    }

    /// The stored shortcuts, if this profile still has a readable shortcuts file.
    pub fn read(profile_dir: &Path) -> Option<serde_json::Value> {
        super::read_json(&path(profile_dir)).flatten()
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

    /// The version a restore should record. Claiming a version newer than the
    /// local Zen understands makes it throw the restored file away and rebuild
    /// from defaults, so the lower of the two wins; an older version is safe,
    /// because Zen migrates the file forward from there.
    pub fn version_to_write(snapshot: Option<i64>, local: Option<i64>) -> Option<i64> {
        match (snapshot, local) {
            (Some(snapshot), Some(local)) => Some(snapshot.min(local)),
            (snapshot, _) => snapshot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reads_customised_keys() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            r#"{"zen-workspace-switch-1":{"modifiers":"alt","key":"1"},
                "key_reload":{}}"#,
        )
        .unwrap();
        let keys = read(dir.path()).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys["zen-workspace-switch-1"]["key"], "1");
        assert_eq!(keys["key_reload"], serde_json::json!({}));
    }

    #[test]
    fn missing_file_means_every_key_is_at_its_default() {
        let dir = tempdir().unwrap();
        assert_eq!(read(dir.path()), Some(Map::new()));
    }

    #[test]
    fn broken_file_reads_as_none() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert!(read(dir.path()).is_none());
        std::fs::write(dir.path().join(FILE), "[]").unwrap();
        assert!(read(dir.path()).is_none());
    }

    #[test]
    fn reads_legacy_shortcuts_and_counts_them() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(legacy::FILE),
            r#"{"shortcuts":[{"id":"zen-workspace-switch-1","key":"1","modifiers":{"alt":true}},
                             {"id":"zen-compact-mode-toggle","key":"S","modifiers":{"accel":true}}]}"#,
        )
        .unwrap();
        let value = legacy::read(dir.path()).unwrap();
        assert_eq!(legacy::count(&value), 2);
        assert_eq!(value["shortcuts"][0]["id"], "zen-workspace-switch-1");
    }

    #[test]
    fn missing_or_broken_legacy_file_reads_as_none() {
        let dir = tempdir().unwrap();
        assert!(legacy::read(dir.path()).is_none());
        std::fs::write(dir.path().join(legacy::FILE), "{ not json").unwrap();
        assert!(legacy::read(dir.path()).is_none());
    }

    #[test]
    fn legacy_version_comes_from_the_pref() {
        assert_eq!(
            legacy::version("user_pref(\"zen.keyboard.shortcuts.version\", 20);"),
            Some(20)
        );
        assert_eq!(legacy::version("user_pref(\"browser.tabs.warnOnClose\", false);"), None);
    }

    #[test]
    fn restore_never_records_a_version_newer_than_this_zen() {
        use super::legacy::version_to_write;
        // Snapshot from a newer Zen: stay at what this build migrates to.
        assert_eq!(version_to_write(Some(20), Some(18)), Some(18));
        // Snapshot from an older Zen: let this build migrate the file forward.
        assert_eq!(version_to_write(Some(18), Some(20)), Some(18));
        // Nothing recorded here yet, or nothing in the snapshot.
        assert_eq!(version_to_write(Some(20), None), Some(20));
        assert_eq!(version_to_write(None, Some(20)), None);
    }
}
