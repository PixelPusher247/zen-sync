use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::bundle::SyncOptions;

const STATE_FILE: &str = "state.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LocalState {
    pub machine_name: String,
    /// Id this device's snapshots are filed under on GitHub. Unset while the
    /// id still follows the name; frozen when a device that has backups is
    /// renamed, so its earlier snapshots stay with it.
    #[serde(default)]
    pub machine_id: Option<String>,
    pub last_backup_at: Option<String>,
    pub snapshot_count: u8,
    pub autostart_enabled: bool,
    /// Per-extension include/exclude choices that differ from the default
    /// (included, unless it's a password manager). Keyed by extension ID.
    #[serde(default)]
    pub extension_overrides: BTreeMap<String, bool>,
    /// about:config prefs the user turned off, keyed by pref name. Prefs
    /// without an entry are included.
    #[serde(default)]
    pub pref_overrides: BTreeMap<String, bool>,
    #[serde(default)]
    pub sync_options: SyncOptions,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            machine_name: default_machine_name(),
            machine_id: None,
            last_backup_at: None,
            snapshot_count: 3,
            autostart_enabled: false,
            extension_overrides: BTreeMap::new(),
            pref_overrides: BTreeMap::new(),
            sync_options: SyncOptions::default(),
        }
    }
}

impl LocalState {
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(STATE_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str(&text) {
            Ok(state) => state,
            Err(e) => {
                // Keep the damaged file for inspection instead of silently
                // overwriting it with defaults on the next save.
                let kept = config_dir.join(format!("{STATE_FILE}.damaged"));
                let _ = std::fs::copy(&path, &kept);
                crate::zslog!("[state] {STATE_FILE} unreadable ({e}); kept as {}", kept.display());
                Self::default()
            }
        }
    }

    /// Write atomically: a crash mid-save leaves the previous file, never a
    /// truncated one.
    pub fn save(&self, config_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(config_dir)
            .map_err(|e| format!("Failed to create config dir: {e}"))?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialize error: {e}"))?;
        let tmp = config_dir.join(format!("{STATE_FILE}.tmp"));
        std::fs::write(&tmp, json).map_err(|e| format!("Failed to write state: {e}"))?;
        std::fs::rename(&tmp, config_dir.join(STATE_FILE))
            .map_err(|e| format!("Failed to save state: {e}"))
    }

    /// The id this device's snapshots are filed under: the one frozen by a
    /// rename if there is one, otherwise derived from the current name.
    pub fn machine_id(&self) -> String {
        self.machine_id
            .clone()
            .unwrap_or_else(|| derive_machine_id(&self.machine_name))
    }
}

/// Asset-name-safe id for a device name: lowercase ASCII letters and digits,
/// with every other character as a dash. GitHub renames asset files with
/// other characters, which would make the snapshot impossible to find again.
/// For ASCII names this matches the ids of earlier versions, so existing
/// snapshots stay with their device. A name with no ASCII letters or digits
/// gets a stable id from its hash.
pub fn derive_machine_id(name: &str) -> String {
    let id: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let id = id.trim_matches('-').to_string();
    if !id.is_empty() {
        return id;
    }
    // FNV-1a: stable across builds and platforms, unlike std's hasher.
    let hash = name
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    format!("device-{:08x}", hash as u32)
}

fn default_machine_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "My Device".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_missing_returns_default() {
        let dir = tempdir().unwrap();
        let s = LocalState::load(dir.path());
        assert_eq!(s.snapshot_count, 3);
        assert!(!s.autostart_enabled);
        assert!(!s.machine_name.is_empty());
    }

    #[test]
    fn save_load_round_trip() {
        let dir = tempdir().unwrap();
        let orig = LocalState {
            machine_name: "Test PC".into(),
            machine_id: Some("old-name".into()),
            last_backup_at: Some("2026-01-01T00:00:00Z".into()),
            snapshot_count: 5,
            autostart_enabled: true,
            extension_overrides: BTreeMap::from([
                ("ext-a@test".to_string(), false),
                ("ext-b@test".to_string(), true),
            ]),
            pref_overrides: BTreeMap::from([("browser.tabs.warnOnClose".to_string(), false)]),
            sync_options: SyncOptions {
                sine_mods: false,
                ..SyncOptions::default()
            },
        };
        orig.save(dir.path()).unwrap();
        let loaded = LocalState::load(dir.path());
        assert_eq!(loaded, orig);
    }

    #[test]
    fn old_state_with_selected_extension_ids_deserializes() {
        // state.json written by 0.1.x, whose selection referred to extensions.json entries
        let dir = tempdir().unwrap();
        let json = r#"{"machine_name":"PC","snapshot_count":3,"autostart_enabled":false,"selected_extension_ids":["a@b"]}"#;
        std::fs::write(dir.path().join("state.json"), json).unwrap();
        let s = LocalState::load(dir.path());
        assert_eq!(s.machine_name, "PC");
        // No stored id: it keeps following the name, as before.
        assert_eq!(s.machine_id, None);
        assert_eq!(s.machine_id(), "pc");
        assert!(s.extension_overrides.is_empty());
        assert_eq!(s.sync_options, SyncOptions::default());
    }

    #[test]
    fn machine_ids_are_ascii_and_match_earlier_versions() {
        assert_eq!(derive_machine_id("Picolaptop-007"), "picolaptop-007");
        assert_eq!(derive_machine_id("Work  Laptop!"), "work--laptop");
        assert_eq!(derive_machine_id("Büro-PC"), "b-ro-pc");
        let cyrillic = derive_machine_id("Ноутбук");
        assert!(cyrillic.starts_with("device-"));
        assert_eq!(cyrillic, derive_machine_id("Ноутбук"));
        assert_ne!(cyrillic, derive_machine_id("Компьютер"));
    }

    #[test]
    fn damaged_state_is_kept_and_defaults_load() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(STATE_FILE), r#"{"machine_name":"PC","snapsh"#).unwrap();
        let s = LocalState::load(dir.path());
        assert_eq!(s.snapshot_count, 3);
        assert!(dir.path().join(format!("{STATE_FILE}.damaged")).exists());
    }

    #[test]
    fn sync_options_missing_from_state_default_to_on() {
        let dir = tempdir().unwrap();
        let json = r#"{"machine_name":"PC","snapshot_count":3,"autostart_enabled":false,"sync_options":{"sineMods":false}}"#;
        std::fs::write(dir.path().join("state.json"), json).unwrap();
        let s = LocalState::load(dir.path());
        assert!(!s.sync_options.sine_mods);
        assert!(s.sync_options.extension_shortcuts);
        assert!(s.sync_options.zen_shortcuts);
        // about:config stays off until it is turned on deliberately.
        assert!(!s.sync_options.about_config);
    }
}
