//! Extension data that Mozilla sync doesn't carry.
//!
//! `storage.local` lives in `storage/default/moz-extension+++<uuid>^userContextId=4294967295/`.
//! The UUID is assigned per profile (pref `extensions.webextensions.uuids`) and
//! is also baked into the folder's `.metadata-v2` file and the IndexedDB
//! `database.origin` column, so restoring onto a profile with a different
//! UUID means rewriting those too.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{fsutil, prefs};

pub const UUIDS_PREF: &str = "extensions.webextensions.uuids";
/// Private user context Firefox reserves for the storage.local IndexedDB backend.
const STORAGE_LOCAL_SUFFIX: &str = "^userContextId=4294967295";
/// Granted optional permissions, host permissions, and private-window access.
pub const PERMISSIONS_FILE: &str = "extension-preferences.json";
/// Holds, among other settings, user-customised extension keyboard shortcuts.
pub const SETTINGS_FILE: &str = "extension-settings.json";

/// Parse the extension id → UUID map from prefs.js.
/// A missing pref is an empty map; a pref that exists but can't be parsed is
/// an error, so callers never overwrite it with a partial map.
pub fn uuid_map(prefs_content: &str) -> Result<BTreeMap<String, String>, String> {
    let Some(raw) = prefs::get_raw(prefs_content, UUIDS_PREF) else {
        return Ok(BTreeMap::new());
    };
    let json = prefs::parse_string_literal(raw)
        .ok_or_else(|| format!("{UUIDS_PREF} is not a string"))?;
    serde_json::from_str(&json).map_err(|e| format!("{UUIDS_PREF} is not valid JSON: {e}"))
}

pub fn uuid_map_literal(map: &BTreeMap<String, String>) -> String {
    prefs::string_literal(&serde_json::to_string(map).unwrap_or_else(|_| "{}".into()))
}

/// Pick a UUID for an extension that isn't installed yet: the backup's UUID
/// unless another extension already uses it locally.
pub fn pick_uuid(map: &BTreeMap<String, String>, preferred: Option<&str>) -> String {
    match preferred {
        Some(uuid) if !map.values().any(|v| v == uuid) => uuid.to_string(),
        _ => random_uuid(),
    }
}

fn random_uuid() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

pub fn migrated_pref(extension_id: &str) -> String {
    format!("extensions.webextensions.ExtensionStorageIDB.migrated.{extension_id}")
}

/// Profile-relative path of an extension's storage.local folder.
pub fn storage_local_rel(uuid: &str) -> String {
    format!("storage/default/moz-extension+++{uuid}{STORAGE_LOCAL_SUFFIX}")
}

pub fn storage_local_dir(profile_dir: &Path, uuid: &str) -> PathBuf {
    profile_dir.join(storage_local_rel(uuid))
}

/// Fold pending WAL pages into each IndexedDB file so a plain file copy is consistent.
pub fn checkpoint_databases(dir: &Path) {
    for entry in fsutil::walk(dir) {
        if entry.is_dir || !entry.rel.ends_with(".sqlite") {
            continue;
        }
        let wal = PathBuf::from(format!("{}-wal", entry.path.display()));
        if std::fs::metadata(&wal).map(|m| m.len() == 0).unwrap_or(true) {
            continue;
        }
        let result = rusqlite::Connection::open(&entry.path).and_then(|conn| {
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        });
        match result {
            Ok(()) => crate::zslog!("[ext] checkpointed {}", entry.path.display()),
            Err(e) => crate::zslog!("[ext] checkpoint failed for {}: {e}", entry.path.display()),
        }
    }
}

/// Files of a storage.local folder worth copying: skips SQLite shared-memory
/// files and empty WAL files.
pub fn storage_entries(dir: &Path) -> Vec<fsutil::WalkEntry> {
    fsutil::walk(dir)
        .into_iter()
        .filter(|e| {
            if e.is_dir {
                return true;
            }
            if e.rel.ends_with("-shm") {
                return false;
            }
            !(e.rel.ends_with("-wal")
                && std::fs::metadata(&e.path).map(|m| m.len() == 0).unwrap_or(true))
        })
        .collect()
}

/// Rewrite the origin UUID baked into a restored storage.local folder.
pub fn retarget_origin(dir: &Path, from_uuid: &str, to_uuid: &str) -> Result<(), String> {
    if from_uuid == to_uuid {
        return Ok(());
    }
    // .metadata-v2 stores length-prefixed strings; equal lengths keep it valid.
    if from_uuid.len() != to_uuid.len() {
        return Err(format!("Cannot retarget UUID {from_uuid} to {to_uuid}: lengths differ"));
    }
    for entry in fsutil::walk(dir) {
        if entry.is_dir {
            continue;
        }
        let name = entry.rel.rsplit('/').next().unwrap_or_default();
        if name == ".metadata-v2" || name == ".metadata" {
            let bytes = std::fs::read(&entry.path)
                .map_err(|e| format!("Failed to read {}: {e}", entry.path.display()))?;
            let replaced = replace_bytes(&bytes, from_uuid.as_bytes(), to_uuid.as_bytes());
            std::fs::write(&entry.path, replaced)
                .map_err(|e| format!("Failed to write {}: {e}", entry.path.display()))?;
        } else if name.ends_with(".sqlite") {
            let conn = rusqlite::Connection::open(&entry.path)
                .map_err(|e| format!("Failed to open {}: {e}", entry.path.display()))?;
            conn.execute(
                "UPDATE database SET origin = replace(origin, ?1, ?2)",
                [from_uuid, to_uuid],
            )
            .map_err(|e| format!("Failed to update origin in {}: {e}", entry.path.display()))?;
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
                .map_err(|e| format!("Checkpoint failed for {}: {e}", entry.path.display()))?;
        }
    }
    Ok(())
}

fn replace_bytes(haystack: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

/// Make Firefox's QuotaManager rescan storage folders on next start instead of
/// trusting its cached origin list, which doesn't know about restored folders.
pub fn invalidate_quota_cache(profile_dir: &Path) {
    let path = profile_dir.join("storage.sqlite");
    if !path.is_file() {
        return;
    }
    let result = rusqlite::Connection::open(&path)
        .and_then(|conn| conn.execute("UPDATE cache SET valid = 0", []));
    match result {
        Ok(_) => crate::zslog!("[ext] invalidated QuotaManager cache"),
        Err(e) => crate::zslog!("[ext] could not invalidate QuotaManager cache: {e}"),
    }
}

/// Read a JSON file; `None` if it doesn't exist or can't be parsed.
pub fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(e) => {
            crate::zslog!("[ext] could not parse {}: {e}", path.display());
            None
        }
    }
}

/// This extension's entries from the `commands` section of extension-settings.json.
pub fn commands_for(settings: &serde_json::Value, extension_id: &str) -> BTreeMap<String, serde_json::Value> {
    let Some(commands) = settings.get("commands").and_then(|c| c.as_object()) else {
        return BTreeMap::new();
    };
    commands
        .iter()
        .filter_map(|(name, entry)| {
            let item = entry
                .get("precedenceList")?
                .as_array()?
                .iter()
                .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(extension_id))?;
            Some((name.clone(), item.clone()))
        })
        .collect()
}

/// Replace this extension's shortcut entries in extension-settings.json,
/// keeping the local install date that Firefox uses for precedence.
pub fn merge_commands(
    settings: &mut serde_json::Value,
    extension_id: &str,
    commands: &BTreeMap<String, serde_json::Value>,
) {
    let Some(root) = settings.as_object_mut() else { return };
    let section = root.entry("commands").or_insert_with(|| serde_json::json!({}));
    let Some(section) = section.as_object_mut() else { return };
    for (name, item) in commands {
        let entry = section
            .entry(name.clone())
            .or_insert_with(|| serde_json::json!({ "precedenceList": [] }));
        let Some(list) = entry.get_mut("precedenceList").and_then(|l| l.as_array_mut()) else {
            continue;
        };
        let existing = list
            .iter_mut()
            .find(|i| i.get("id").and_then(|v| v.as_str()) == Some(extension_id));
        match existing {
            Some(existing) => {
                let install_date = existing.get("installDate").cloned();
                *existing = item.clone();
                if let (Some(date), Some(obj)) = (install_date, existing.as_object_mut()) {
                    obj.insert("installDate".into(), date);
                }
            }
            None => list.push(item.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const UUID_A: &str = "bd4235c6-e8b3-43c4-aea1-01bd3d2d3cbd";
    const UUID_B: &str = "0f000000-1111-4222-8333-444444444444";

    #[test]
    fn parses_and_rewrites_uuid_map() {
        let prefs = r#"user_pref("extensions.webextensions.uuids", "{\"addon@darkreader.org\":\"bd4235c6-e8b3-43c4-aea1-01bd3d2d3cbd\"}");"#;
        let mut map = uuid_map(prefs).unwrap();
        assert_eq!(map["addon@darkreader.org"], UUID_A);

        map.insert("new@ext".into(), UUID_B.into());
        let literal = uuid_map_literal(&map);
        let round = format!("user_pref(\"{UUIDS_PREF}\", {literal});");
        assert_eq!(uuid_map(&round).unwrap(), map);

        assert!(uuid_map("").unwrap().is_empty());
        assert!(uuid_map(r#"user_pref("extensions.webextensions.uuids", "{broken");"#).is_err());
    }

    #[test]
    fn pick_uuid_avoids_collisions() {
        let mut map = BTreeMap::new();
        assert_eq!(pick_uuid(&map, Some(UUID_A)), UUID_A);
        map.insert("other@ext".to_string(), UUID_A.to_string());
        let fresh = pick_uuid(&map, Some(UUID_A));
        assert_ne!(fresh, UUID_A);
        assert_eq!(fresh.len(), 36);
        assert_eq!(&fresh[14..15], "4");
    }

    #[test]
    fn retargets_metadata_and_indexeddb_origin() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let origin_a = format!("moz-extension://{UUID_A}^userContextId=4294967295");

        // Length-prefixed strings, as in a real .metadata-v2.
        let mut metadata = vec![0u8, 6, 0x5b, 0x89];
        metadata.extend_from_slice(&(origin_a.len() as u32).to_be_bytes());
        metadata.extend_from_slice(origin_a.as_bytes());
        std::fs::write(root.join(".metadata-v2"), &metadata).unwrap();

        std::fs::create_dir_all(root.join("idb")).unwrap();
        let db = root.join("idb/3647222921wleabcEoxlt-eengsairo.sqlite");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE database(name TEXT PRIMARY KEY, origin TEXT NOT NULL) WITHOUT ROWID;",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO database VALUES ('webExtensions-storage-local', ?1)",
                [&origin_a],
            )
            .unwrap();
        }

        retarget_origin(root, UUID_A, UUID_B).unwrap();

        let origin_b = format!("moz-extension://{UUID_B}^userContextId=4294967295");
        let patched = std::fs::read(root.join(".metadata-v2")).unwrap();
        assert_eq!(patched.len(), metadata.len());
        assert!(patched.windows(origin_b.len()).any(|w| w == origin_b.as_bytes()));

        let conn = rusqlite::Connection::open(&db).unwrap();
        let origin: String = conn
            .query_row("SELECT origin FROM database", [], |r| r.get(0))
            .unwrap();
        assert_eq!(origin, origin_b);
    }

    #[test]
    fn extracts_and_merges_commands() {
        let source = serde_json::json!({
            "version": 3,
            "commands": {
                "autofill_login": { "precedenceList": [
                    { "id": "bw@ext", "installDate": 1, "value": { "shortcut": "Ctrl+Shift+Space" }, "enabled": true },
                    { "id": "other@ext", "installDate": 2, "value": { "shortcut": "Alt+L" }, "enabled": true }
                ]}
            }
        });
        let commands = commands_for(&source, "bw@ext");
        assert_eq!(commands.len(), 1);

        let mut target = serde_json::json!({
            "version": 3,
            "commands": {
                "autofill_login": { "precedenceList": [
                    { "id": "bw@ext", "installDate": 99, "value": { "shortcut": "" }, "enabled": true }
                ]}
            }
        });
        merge_commands(&mut target, "bw@ext", &commands);
        let item = &target["commands"]["autofill_login"]["precedenceList"][0];
        assert_eq!(item["value"]["shortcut"], "Ctrl+Shift+Space");
        assert_eq!(item["installDate"], 99);
        assert_eq!(target["commands"]["autofill_login"]["precedenceList"].as_array().unwrap().len(), 1);
    }
}
