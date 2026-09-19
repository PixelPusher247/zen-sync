use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::bundle::SyncOptions;

const STATE_FILE: &str = "state.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LocalState {
    pub machine_name: String,
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
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, config_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(config_dir)
            .map_err(|e| format!("Failed to create config dir: {e}"))?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialize error: {e}"))?;
        std::fs::write(config_dir.join(STATE_FILE), json)
            .map_err(|e| format!("Failed to write state: {e}"))
    }
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
        assert!(s.extension_overrides.is_empty());
        assert_eq!(s.sync_options, SyncOptions::default());
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
