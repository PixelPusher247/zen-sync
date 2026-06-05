use serde::{Deserialize, Serialize};
use std::path::Path;

const STATE_FILE: &str = "state.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LocalState {
    pub machine_name: String,
    pub last_backup_at: Option<String>,
    pub snapshot_count: u8,
    pub autostart_enabled: bool,
    /// Extension IDs to include in backups. Empty = sync all user extensions.
    #[serde(default)]
    pub selected_extension_ids: Vec<String>,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            machine_name: default_machine_name(),
            last_backup_at: None,
            snapshot_count: 3,
            autostart_enabled: false,
            selected_extension_ids: Vec::new(),
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
            selected_extension_ids: vec!["ext-a@test".into(), "ext-b@test".into()],
        };
        orig.save(dir.path()).unwrap();
        let loaded = LocalState::load(dir.path());
        assert_eq!(loaded, orig);
    }

    #[test]
    fn old_state_without_extension_ids_deserializes() {
        // Simulates loading a state.json written before selected_extension_ids was added
        let dir = tempdir().unwrap();
        let json = r#"{"machine_name":"PC","snapshot_count":3,"autostart_enabled":false}"#;
        std::fs::write(dir.path().join("state.json"), json).unwrap();
        let s = LocalState::load(dir.path());
        assert!(s.selected_extension_ids.is_empty());
    }
}
