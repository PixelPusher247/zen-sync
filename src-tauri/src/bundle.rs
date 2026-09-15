//! Snapshot bundle, format 3: a zip archive of Sine mods, mod setting values
//! and per-extension data, described by `manifest.json`.
//!
//! Everything else in the profile (spaces, containers, bookmarks,
//! `storage.sync`, ...) is left to Zen's built-in Mozilla account sync.
//!
//! Format 3 can leave out mod files or mod settings. Format 2 always had both,
//! and zen-sync 0.2.x would wipe the mods folder restoring a snapshot without
//! them, so the version was bumped to make those builds refuse it instead.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{ext_storage, extensions, fsutil, prefs, sine};

pub const FORMAT_VERSION: u8 = 3;
/// Oldest format that can still be restored.
const MIN_FORMAT_VERSION: u8 = 2;
const MANIFEST_NAME: &str = "manifest.json";
const MODS_PREFIX: &str = "sine/mods/";
const PREFS_FILE: &str = "prefs.js";

/// Which kinds of data this device puts into backups and takes from restores.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncOptions {
    pub sine_mods: bool,
    pub mod_settings: bool,
    pub extension_storage: bool,
    pub extension_permissions: bool,
    pub extension_shortcuts: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            sine_mods: true,
            mod_settings: true,
            extension_storage: true,
            extension_permissions: true,
            extension_shortcuts: true,
        }
    }
}

impl SyncOptions {
    fn any_extension_data(&self) -> bool {
        self.extension_storage || self.extension_permissions || self.extension_shortcuts
    }
}

/// Everything this device's settings say a backup includes and a restore applies.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub options: SyncOptions,
    /// Per-extension choices, see [`extensions::is_selected`].
    pub extension_overrides: BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub format: u8,
    pub created_at: u64,
    pub sine: Option<SineSection>,
    pub extensions: Vec<ExtensionSection>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SineSection {
    pub engine_version: Option<String>,
    pub mod_count: usize,
    /// Whether the mod files are under `sine/mods/`. Always true in format 2.
    #[serde(default = "default_true")]
    pub mods_included: bool,
    /// Declared mod and Sine settings → raw prefs.js value literal.
    /// Settings at their default have no entry. None if settings were left out.
    pub prefs: Option<BTreeMap<String, String>>,
    /// Every setting declared on the source device, so a restore only resets
    /// settings the source knew about. Missing in format 2.
    #[serde(default)]
    pub declared_prefs: Option<BTreeSet<String>>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExtensionSection {
    pub id: String,
    pub name: String,
    pub version: String,
    /// UUID the extension had in the source profile.
    pub uuid: Option<String>,
    /// Archive folder holding the storage.local directory, if one was backed up.
    pub storage_prefix: Option<String>,
    /// This extension's entry from extension-preferences.json.
    pub permissions: Option<serde_json::Value>,
    /// This extension's shortcut entries from extension-settings.json.
    #[serde(default)]
    pub commands: BTreeMap<String, serde_json::Value>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub sine_installed: bool,
    pub sine_engine_version: Option<String>,
    pub mod_count: usize,
    pub mod_setting_count: usize,
    pub extension_count: usize,
    pub storage_bytes: u64,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub mod_count: usize,
    pub extension_count: usize,
    pub warnings: Vec<String>,
}

fn zip_err(e: zip::result::ZipError) -> String {
    format!("Archive error: {e}")
}

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_prefs(profile_dir: &Path) -> String {
    std::fs::read_to_string(profile_dir.join(PREFS_FILE)).unwrap_or_default()
}

/// What a backup of this profile would contain right now.
pub fn summarize(profile_dir: &Path, overrides: &BTreeMap<String, bool>) -> BackupSummary {
    let prefs_content = read_prefs(profile_dir);
    let has_mods = sine::has_mods(profile_dir);
    let uuids = ext_storage::uuid_map(&prefs_content).unwrap_or_default();

    let mut extension_count = 0;
    let mut storage_bytes = 0;
    for ext in extensions::list_extensions(profile_dir).unwrap_or_default() {
        if !extensions::is_selected(overrides, &ext.id, &ext.name) {
            continue;
        }
        extension_count += 1;
        if let Some(uuid) = uuids.get(&ext.id) {
            storage_bytes += fsutil::dir_size(&ext_storage::storage_local_dir(profile_dir, uuid));
        }
    }

    BackupSummary {
        sine_installed: sine::engine_installed(profile_dir),
        sine_engine_version: sine::engine_version(profile_dir),
        mod_count: if has_mods { sine::mod_count(profile_dir) } else { 0 },
        mod_setting_count: if has_mods {
            prefs::read_values(&prefs_content, &sine::declared_prefs(profile_dir)).len()
        } else {
            0
        },
        extension_count,
        storage_bytes,
    }
}

// ── Backup ────────────────────────────────────────────────────────────────────

/// Build a snapshot archive from the profile. Zen must be closed.
pub fn build(profile_dir: &Path, selection: &Selection) -> Result<(Vec<u8>, Manifest), String> {
    let Selection { options, extension_overrides: overrides } = selection;
    if !options.sine_mods && !options.mod_settings && !options.any_extension_data() {
        return Err("Nothing is selected to back up. Choose what to sync in Settings.".into());
    }
    let prefs_content = read_prefs(profile_dir);
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));

    let sine = if !sine::has_mods(profile_dir) {
        crate::zslog!("[bundle] no Sine mods in profile");
        None
    } else if options.sine_mods || options.mod_settings {
        if options.sine_mods {
            for entry in sine::mod_files(profile_dir) {
                add_entry(&mut zip, &format!("{MODS_PREFIX}{}", entry.rel), &entry)?;
            }
        }
        let declared = sine::declared_prefs(profile_dir);
        let section = SineSection {
            engine_version: sine::engine_version(profile_dir),
            mod_count: sine::mod_count(profile_dir),
            mods_included: options.sine_mods,
            prefs: options
                .mod_settings
                .then(|| prefs::read_values(&prefs_content, &declared)),
            declared_prefs: options.mod_settings.then_some(declared),
        };
        crate::zslog!(
            "[bundle] sine: {} mods (files: {}), settings: {:?}",
            section.mod_count,
            section.mods_included,
            section.prefs.as_ref().map(BTreeMap::len)
        );
        Some(section)
    } else {
        crate::zslog!("[bundle] Sine mods and settings turned off");
        None
    };

    let uuids = ext_storage::uuid_map(&prefs_content).unwrap_or_else(|e| {
        crate::zslog!("[bundle] {e}; extension storage skipped");
        BTreeMap::new()
    });
    let permissions = options
        .extension_permissions
        .then(|| ext_storage::read_json(&profile_dir.join(ext_storage::PERMISSIONS_FILE)))
        .flatten();
    let settings = options
        .extension_shortcuts
        .then(|| ext_storage::read_json(&profile_dir.join(ext_storage::SETTINGS_FILE)))
        .flatten();

    let installed = if options.any_extension_data() {
        extensions::list_extensions(profile_dir)?
    } else {
        Vec::new()
    };
    let mut extension_sections: Vec<ExtensionSection> = Vec::new();
    for ext in installed {
        if !extensions::is_selected(overrides, &ext.id, &ext.name) {
            continue;
        }
        let uuid = uuids.get(&ext.id).cloned();
        let storage_dir = uuid
            .as_deref()
            .filter(|_| options.extension_storage)
            .map(|u| ext_storage::storage_local_dir(profile_dir, u))
            .filter(|d| d.is_dir());
        let ext_permissions = permissions.as_ref().and_then(|p| p.get(&ext.id)).cloned();
        let commands = settings
            .as_ref()
            .map(|s| ext_storage::commands_for(s, &ext.id))
            .unwrap_or_default();
        if storage_dir.is_none() && ext_permissions.is_none() && commands.is_empty() {
            continue;
        }

        let storage_prefix = match storage_dir {
            Some(dir) => {
                ext_storage::checkpoint_databases(&dir);
                let prefix = format!("extensions/{}/storage-local/", extension_sections.len());
                for entry in ext_storage::storage_entries(&dir) {
                    add_entry(&mut zip, &format!("{prefix}{}", entry.rel), &entry)?;
                }
                Some(prefix)
            }
            None => None,
        };
        crate::zslog!(
            "[bundle] extension {} (storage.local: {})",
            ext.id,
            storage_prefix.is_some()
        );
        extension_sections.push(ExtensionSection {
            id: ext.id,
            name: ext.name,
            version: ext.version,
            uuid,
            storage_prefix,
            permissions: ext_permissions,
            commands,
        });
    }

    if sine.is_none() && extension_sections.is_empty() {
        return Err("Nothing to back up: no Sine mods and no extension data found.".into());
    }

    let manifest = Manifest {
        format: FORMAT_VERSION,
        created_at: now_epoch(),
        sine,
        extensions: extension_sections,
    };
    zip.start_file(MANIFEST_NAME, file_options(CompressionMethod::Deflated))
        .map_err(zip_err)?;
    serde_json::to_writer_pretty(&mut zip, &manifest)
        .map_err(|e| format!("Manifest write failed: {e}"))?;
    let bytes = zip.finish().map_err(zip_err)?.into_inner();
    Ok((bytes, manifest))
}

fn file_options(method: CompressionMethod) -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(method)
}

/// Store formats that are already compressed instead of deflating them again.
fn compression_for(name: &str) -> CompressionMethod {
    const COMPRESSED: &[&str] = &[
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".woff2", ".zip", ".mp4", ".webm",
    ];
    let lower = name.to_ascii_lowercase();
    if COMPRESSED.iter().any(|ext| lower.ends_with(ext)) {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    }
}

fn add_entry(
    zip: &mut ZipWriter<Cursor<Vec<u8>>>,
    name: &str,
    entry: &fsutil::WalkEntry,
) -> Result<(), String> {
    if entry.is_dir {
        return zip
            .add_directory(name, file_options(CompressionMethod::Stored))
            .map_err(zip_err);
    }
    zip.start_file(name, file_options(compression_for(name)))
        .map_err(zip_err)?;
    let mut file = std::fs::File::open(&entry.path)
        .map_err(|e| format!("Could not read {}: {e}", entry.path.display()))?;
    std::io::copy(&mut file, zip)
        .map_err(|e| format!("Could not archive {}: {e}", entry.path.display()))?;
    Ok(())
}

// ── Restore ───────────────────────────────────────────────────────────────────

/// Apply a snapshot archive to the profile. Zen must be closed.
///
/// Everything that gets overwritten is first copied to `safety_dir`.
pub fn apply(
    profile_dir: &Path,
    archive_bytes: &[u8],
    selection: &Selection,
    safety_dir: &Path,
) -> Result<RestoreReport, String> {
    let Selection { options, extension_overrides: overrides } = selection;
    let mut archive = ZipArchive::new(Cursor::new(archive_bytes))
        .map_err(|e| format!("Snapshot is not a valid archive: {e}"))?;
    let manifest = read_manifest(&mut archive)?;
    let mut report = RestoreReport::default();

    let prefs_path = profile_dir.join(PREFS_FILE);
    let prefs_content = read_prefs(profile_dir);
    let mut set_prefs: BTreeMap<String, String> = BTreeMap::new();
    let mut clear_prefs: BTreeSet<String> = BTreeSet::new();

    // What this device takes from the snapshot, given its own sync options.
    let sine_mods = manifest
        .sine
        .as_ref()
        .filter(|s| options.sine_mods && s.mods_included);
    let sine_prefs = manifest
        .sine
        .as_ref()
        .and_then(|s| s.prefs.as_ref())
        .filter(|_| options.mod_settings);
    let restores_storage = |e: &ExtensionSection| options.extension_storage && e.storage_prefix.is_some();
    let restores_permissions = |e: &ExtensionSection| options.extension_permissions && e.permissions.is_some();
    let restores_shortcuts = |e: &ExtensionSection| options.extension_shortcuts && !e.commands.is_empty();

    let selected: Vec<&ExtensionSection> = manifest
        .extensions
        .iter()
        .filter(|e| extensions::is_selected(overrides, &e.id, &e.name))
        .filter(|e| restores_storage(e) || restores_permissions(e) || restores_shortcuts(e))
        .collect();
    let installed: BTreeSet<String> = if selected.is_empty() {
        BTreeSet::new()
    } else {
        extensions::list_extensions(profile_dir)?
            .into_iter()
            .map(|e| e.id)
            .collect()
    };

    // Resolve target UUIDs up front so the safety copy covers every folder we replace.
    let mut uuid_map = ext_storage::uuid_map(&prefs_content);
    let mut uuid_map_changed = false;
    let mut storage_targets: BTreeMap<&str, String> = BTreeMap::new();
    match uuid_map.as_mut() {
        Ok(map) => {
            for ext in selected.iter().filter(|e| restores_storage(e)) {
                let uuid = match map.get(&ext.id) {
                    Some(uuid) => uuid.clone(),
                    None => {
                        let uuid = ext_storage::pick_uuid(map, ext.uuid.as_deref());
                        map.insert(ext.id.clone(), uuid.clone());
                        uuid_map_changed = true;
                        uuid
                    }
                };
                storage_targets.insert(ext.id.as_str(), uuid);
            }
        }
        Err(e) => {
            crate::zslog!("[bundle] {e}");
            if selected.iter().any(|e| restores_storage(e)) {
                report.warnings.push(
                    "Couldn't read extension IDs from prefs.js, so extension storage was not restored."
                        .into(),
                );
            }
        }
    }

    let mut to_save = vec![
        PREFS_FILE.to_string(),
        ext_storage::PERMISSIONS_FILE.to_string(),
        ext_storage::SETTINGS_FILE.to_string(),
    ];
    if sine_mods.is_some() {
        to_save.push("chrome/sine-mods".into());
    }
    to_save.extend(storage_targets.values().map(|uuid| ext_storage::storage_local_rel(uuid)));
    for rel in &to_save {
        let src = profile_dir.join(rel);
        if src.exists() {
            fsutil::copy_all(&src, &safety_dir.join(rel))?;
        }
    }
    crate::zslog!("[bundle] safety copy at {}", safety_dir.display());

    // Sine mods: mirror the snapshot, then sync the declared settings.
    if let Some(section) = sine_mods {
        let dest = sine::mods_dir(profile_dir);
        replace_dir(&dest)?;
        extract_prefix(&mut archive, MODS_PREFIX, &dest)?;
        // Sine rewrites these on startup; make sure they exist until it does.
        for generated in ["chrome.css", "content.css"] {
            let path = dest.join(generated);
            if !path.exists() {
                std::fs::write(&path, "")
                    .map_err(|e| format!("Failed to create {}: {e}", path.display()))?;
            }
        }
        report.mod_count = section.mod_count;
    }

    if let Some(values) = sine_prefs {
        for (name, value) in values {
            if !prefs::is_protected(name) {
                set_prefs.insert(name.clone(), value.clone());
            }
        }
        // A declared setting without a value is at its default on the source, so
        // reset it here too, but only if the source declared it: settings of mods
        // only this device has stay. Format 2 has no declared list; its mods, once
        // restored, declare the same settings.
        let source_declared = manifest.sine.as_ref().and_then(|s| s.declared_prefs.as_ref());
        if source_declared.is_some() || sine_mods.is_some() {
            clear_prefs.extend(sine::declared_prefs(profile_dir).into_iter().filter(|name| {
                !values.contains_key(name) && source_declared.is_none_or(|d| d.contains(name))
            }));
        }
    }

    let sine_restored = manifest
        .sine
        .as_ref()
        .filter(|_| sine_mods.is_some() || sine_prefs.is_some());
    if let Some(section) = sine_restored {
        if !sine::engine_installed(profile_dir) {
            report.warnings.push(
                "Sine isn't installed on this device. The restored mods and settings won't load until you install Sine."
                    .into(),
            );
        } else if let (Some(local), Some(backup)) =
            (sine::engine_version(profile_dir), &section.engine_version)
        {
            if &local != backup {
                report.warnings.push(format!(
                    "Sine versions differ (snapshot {backup}, this device {local}). Some mods may behave differently."
                ));
            }
        }
    }

    // Extensions: storage.local, granted permissions, shortcuts.
    let permissions_path = profile_dir.join(ext_storage::PERMISSIONS_FILE);
    let settings_path = profile_dir.join(ext_storage::SETTINGS_FILE);
    let mut permissions = ext_storage::read_json(&permissions_path);
    let mut settings = ext_storage::read_json(&settings_path);
    // Never replace a file that exists but couldn't be parsed.
    let permissions_writable = permissions.is_some() || !permissions_path.exists();
    let mut permissions_changed = false;
    let mut settings_changed = false;
    let mut storage_restored = false;

    for ext in &selected {
        if let (Some(prefix), Some(uuid)) =
            (&ext.storage_prefix, storage_targets.get(ext.id.as_str()))
        {
            let dest = ext_storage::storage_local_dir(profile_dir, uuid);
            replace_dir(&dest)?;
            extract_prefix(&mut archive, prefix, &dest)?;
            if let Some(source_uuid) = &ext.uuid {
                ext_storage::retarget_origin(&dest, source_uuid, uuid)?;
            }
            set_prefs.insert(ext_storage::migrated_pref(&ext.id), "true".into());
            storage_restored = true;
        }

        if let (Some(entry), true) = (&ext.permissions, permissions_writable && restores_permissions(ext)) {
            let root = permissions.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = root.as_object_mut() {
                obj.insert(ext.id.clone(), entry.clone());
                permissions_changed = true;
            }
        }

        if restores_shortcuts(ext) {
            match settings.as_mut() {
                Some(root) => {
                    ext_storage::merge_commands(root, &ext.id, &ext.commands);
                    settings_changed = true;
                }
                None => crate::zslog!(
                    "[bundle] {} missing or unreadable; shortcuts for {} skipped",
                    ext_storage::SETTINGS_FILE,
                    ext.id
                ),
            }
        }

        if !installed.contains(&ext.id) {
            report.warnings.push(format!(
                "{} isn't installed on this device yet. Its data was restored; check its settings once Zen's sync installs it.",
                ext.name
            ));
        }
        report.extension_count += 1;
    }

    if let (true, Ok(map)) = (uuid_map_changed, &uuid_map) {
        set_prefs.insert(ext_storage::UUIDS_PREF.into(), ext_storage::uuid_map_literal(map));
    }

    if !set_prefs.is_empty() || !clear_prefs.is_empty() {
        std::fs::write(&prefs_path, prefs::apply(&prefs_content, &set_prefs, &clear_prefs))
            .map_err(|e| format!("Failed to write prefs.js: {e}"))?;
        crate::zslog!(
            "[bundle] prefs.js: {} set, {} cleared",
            set_prefs.len(),
            clear_prefs.len()
        );
    }
    if permissions_changed {
        write_json(&permissions_path, permissions.as_ref())?;
    }
    if settings_changed {
        write_json(&settings_path, settings.as_ref())?;
    }
    if storage_restored {
        ext_storage::invalidate_quota_cache(profile_dir);
    }
    Ok(report)
}

fn read_manifest<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Manifest, String> {
    let mut bytes = Vec::new();
    archive
        .by_name(MANIFEST_NAME)
        .map_err(|_| "Snapshot has no manifest".to_string())?
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Snapshot manifest unreadable: {e}"))?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Snapshot manifest error: {e}"))?;
    let format = raw.get("format").and_then(|v| v.as_u64()).unwrap_or(0);
    if !(u64::from(MIN_FORMAT_VERSION)..=u64::from(FORMAT_VERSION)).contains(&format) {
        return Err(format!("Unsupported snapshot format {format}. Update zen-sync."));
    }
    serde_json::from_value(raw).map_err(|e| format!("Snapshot manifest error: {e}"))
}

fn replace_dir(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)
            .map_err(|e| format!("Failed to clear {}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create {}: {e}", dir.display()))
}

/// Extract every archive entry under `prefix` into `dest`.
fn extract_prefix<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    prefix: &str,
    dest: &Path,
) -> Result<(), String> {
    let indices: Vec<usize> = (0..archive.len())
        .filter(|&i| archive.name_for_index(i).is_some_and(|n| n.starts_with(prefix)))
        .collect();
    for i in indices {
        let mut file = archive.by_index(i).map_err(zip_err)?;
        let rel = file.name()[prefix.len()..].to_string();
        if rel.trim_matches('/').is_empty() {
            continue;
        }
        let target = fsutil::safe_join(dest, &rel)
            .ok_or_else(|| format!("Unsafe path in snapshot: {}", file.name()))?;
        if file.is_dir() {
            std::fs::create_dir_all(&target)
                .map_err(|e| format!("Failed to create {}: {e}", target.display()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
        let mut out = std::fs::File::create(&target)
            .map_err(|e| format!("Failed to write {}: {e}", target.display()))?;
        std::io::copy(&mut file, &mut out)
            .map_err(|e| format!("Failed to write {}: {e}", target.display()))?;
    }
    Ok(())
}

fn write_json(path: &Path, value: Option<&serde_json::Value>) -> Result<(), String> {
    let Some(value) = value else { return Ok(()) };
    let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const DARK_READER: &str = "addon@darkreader.org";
    const BITWARDEN: &str = "{446900e4-71c2-419f-a6a7-df9c091e268b}";
    const UUID_SRC: &str = "bd4235c6-e8b3-43c4-aea1-01bd3d2d3cbd";
    const UUID_DST: &str = "0f000000-1111-4222-8333-444444444444";
    const UUID_BW: &str = "5bf56911-e4f7-4ee9-a870-c80b54d2cff2";

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn uuids_line(entries: &[(&str, &str)]) -> String {
        let map: BTreeMap<String, String> =
            entries.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        format!(
            "user_pref(\"{}\", {});",
            ext_storage::UUIDS_PREF,
            ext_storage::uuid_map_literal(&map)
        )
    }

    fn extensions_json(ids: &[(&str, &str)]) -> String {
        let addons: Vec<serde_json::Value> = ids
            .iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "id": id, "type": "extension", "location": "app-profile",
                    "version": "1.0", "defaultLocale": { "name": name }
                })
            })
            .collect();
        serde_json::json!({ "schemaVersion": 37, "addons": addons }).to_string()
    }

    fn storage_folder(profile: &Path, uuid: &str, value: &str) {
        let dir = ext_storage::storage_local_dir(profile, uuid);
        let origin = format!("moz-extension://{uuid}^userContextId=4294967295");
        std::fs::create_dir_all(dir.join("idb/3647222921wleabcEoxlt-eengsairo.files")).unwrap();
        std::fs::write(dir.join(".metadata-v2"), origin.as_bytes()).unwrap();
        let conn =
            rusqlite::Connection::open(dir.join("idb/3647222921wleabcEoxlt-eengsairo.sqlite"))
                .unwrap();
        conn.execute_batch(
            "CREATE TABLE database(name TEXT PRIMARY KEY, origin TEXT NOT NULL) WITHOUT ROWID;
             CREATE TABLE object_data(key TEXT, value TEXT);",
        )
        .unwrap();
        conn.execute("INSERT INTO database VALUES ('webExtensions-storage-local', ?1)", [&origin])
            .unwrap();
        conn.execute("INSERT INTO object_data VALUES ('theme', ?1)", [value]).unwrap();
    }

    fn source_profile(root: &Path) {
        write(
            root,
            "prefs.js",
            &format!(
                "user_pref(\"mod.lean.hide-zoom\", true);\nuser_pref(\"services.sync.client.name\", \"Source PC\");\n{}\n",
                uuids_line(&[(DARK_READER, UUID_SRC), (BITWARDEN, UUID_BW)])
            ),
        );
        write(root, "chrome/JS/sine.sys.mjs", "");
        write(root, "chrome/JS/engine.json", r#"{"version":"2.3.4.1c"}"#);
        write(root, "chrome/sine-mods/mods.json", r#"{"lean":{"enabled":true}}"#);
        write(root, "chrome/sine-mods/chrome.css", "@import \"file:///C:/Users/source/...\";");
        write(root, "chrome/sine-mods/lean/chrome.css", "/* lean */");
        write(
            root,
            "chrome/sine-mods/lean/preferences.json",
            r#"[{"property":"mod.lean.hide-zoom"},{"property":"mod.lean.top-workspace"}]"#,
        );
        write(
            root,
            "extensions.json",
            &extensions_json(&[(DARK_READER, "Dark Reader"), (BITWARDEN, "Bitwarden Password Manager")]),
        );
        storage_folder(root, UUID_SRC, "dark");
        storage_folder(root, UUID_BW, "vault");
        write(
            root,
            ext_storage::PERMISSIONS_FILE,
            &serde_json::json!({
                DARK_READER: { "permissions": [], "origins": ["<all_urls>"], "data_collection": [] },
                BITWARDEN: { "permissions": ["nativeMessaging"], "origins": [], "data_collection": [] }
            })
            .to_string(),
        );
        write(
            root,
            ext_storage::SETTINGS_FILE,
            &serde_json::json!({
                "version": 3,
                "commands": { "toggle": { "precedenceList": [
                    { "id": DARK_READER, "installDate": 1, "value": { "shortcut": "Alt+Shift+D" }, "enabled": true }
                ]}}
            })
            .to_string(),
        );
    }

    fn origin_in_db(profile: &Path, uuid: &str) -> (String, String) {
        let db = ext_storage::storage_local_dir(profile, uuid)
            .join("idb/3647222921wleabcEoxlt-eengsairo.sqlite");
        let conn = rusqlite::Connection::open(db).unwrap();
        let origin = conn.query_row("SELECT origin FROM database", [], |r| r.get(0)).unwrap();
        let value = conn.query_row("SELECT value FROM object_data", [], |r| r.get(0)).unwrap();
        (origin, value)
    }

    #[test]
    fn round_trip_onto_profile_with_different_uuids() {
        let source = tempdir().unwrap();
        source_profile(source.path());

        let (bytes, manifest) = build(source.path(), &Selection::default()).unwrap();
        let sine = manifest.sine.as_ref().unwrap();
        assert_eq!(sine.prefs.as_ref().unwrap().len(), 1);
        assert_eq!(manifest.extensions.len(), 1, "Bitwarden is excluded by default");
        assert_eq!(manifest.extensions[0].id, DARK_READER);

        let target = tempdir().unwrap();
        let t = target.path();
        write(
            t,
            "prefs.js",
            &format!(
                "user_pref(\"mod.lean.top-workspace\", true);\nuser_pref(\"services.sync.client.name\", \"Target PC\");\n{}\n",
                uuids_line(&[(DARK_READER, UUID_DST)])
            ),
        );
        write(t, "chrome/JS/sine.sys.mjs", "");
        write(t, "chrome/JS/engine.json", r#"{"version":"2.3.4.1c"}"#);
        write(t, "chrome/sine-mods/mods.json", r#"{"old":{}}"#);
        write(t, "chrome/sine-mods/old/chrome.css", "/* old */");
        write(t, "extensions.json", &extensions_json(&[(DARK_READER, "Dark Reader")]));
        storage_folder(t, UUID_DST, "light");
        write(t, &format!("{}/stale.txt", ext_storage::storage_local_rel(UUID_DST)), "x");
        write(t, ext_storage::PERMISSIONS_FILE, "{}");
        write(
            t,
            ext_storage::SETTINGS_FILE,
            &serde_json::json!({
                "version": 3,
                "commands": { "toggle": { "precedenceList": [
                    { "id": DARK_READER, "installDate": 42, "value": { "shortcut": "" }, "enabled": true }
                ]}}
            })
            .to_string(),
        );
        {
            let conn = rusqlite::Connection::open(t.join("storage.sqlite")).unwrap();
            conn.execute_batch("CREATE TABLE cache(valid INTEGER, build_id TEXT); INSERT INTO cache VALUES (1, 'x');")
                .unwrap();
        }

        let safety = tempdir().unwrap();
        let report = apply(t, &bytes, &Selection::default(),safety.path()).unwrap();
        assert_eq!(report.extension_count, 1);
        assert_eq!(report.mod_count, 1);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);

        // Mods mirrored; generated stylesheet not carried over.
        assert!(!t.join("chrome/sine-mods/old").exists());
        assert!(t.join("chrome/sine-mods/lean/chrome.css").is_file());
        assert_eq!(std::fs::read_to_string(t.join("chrome/sine-mods/chrome.css")).unwrap(), "");

        // Mod settings synced, identity untouched, UUID map unchanged.
        let prefs_out = std::fs::read_to_string(t.join("prefs.js")).unwrap();
        assert!(prefs_out.contains("user_pref(\"mod.lean.hide-zoom\", true);"));
        assert!(!prefs_out.contains("mod.lean.top-workspace"));
        assert!(prefs_out.contains("\"Target PC\""));
        assert!(prefs_out.contains(&ext_storage::migrated_pref(DARK_READER)));
        assert_eq!(ext_storage::uuid_map(&prefs_out).unwrap()[DARK_READER], UUID_DST);

        // Storage replaced and retargeted to the local UUID.
        let (origin, value) = origin_in_db(t, UUID_DST);
        assert_eq!(origin, format!("moz-extension://{UUID_DST}^userContextId=4294967295"));
        assert_eq!(value, "dark");
        assert!(!ext_storage::storage_local_dir(t, UUID_DST).join("stale.txt").exists());
        assert!(!ext_storage::storage_local_dir(t, UUID_SRC).exists());

        // Permissions and shortcuts merged.
        let perms = ext_storage::read_json(&t.join(ext_storage::PERMISSIONS_FILE)).unwrap();
        assert_eq!(perms[DARK_READER]["origins"][0], "<all_urls>");
        let settings = ext_storage::read_json(&t.join(ext_storage::SETTINGS_FILE)).unwrap();
        let item = &settings["commands"]["toggle"]["precedenceList"][0];
        assert_eq!(item["value"]["shortcut"], "Alt+Shift+D");
        assert_eq!(item["installDate"], 42);

        // QuotaManager will rescan; safety copy holds the pre-restore state.
        let conn = rusqlite::Connection::open(t.join("storage.sqlite")).unwrap();
        let valid: i64 = conn.query_row("SELECT valid FROM cache", [], |r| r.get(0)).unwrap();
        assert_eq!(valid, 0);
        assert!(safety.path().join("chrome/sine-mods/old/chrome.css").is_file());
        assert!(safety
            .path()
            .join(ext_storage::storage_local_rel(UUID_DST))
            .join("stale.txt")
            .is_file());
    }

    #[test]
    fn restore_seeds_uuid_for_extension_not_installed_yet() {
        let source = tempdir().unwrap();
        source_profile(source.path());
        let (bytes, _) = build(source.path(), &Selection::default()).unwrap();

        let target = tempdir().unwrap();
        let t = target.path();
        write(t, "prefs.js", &format!("{}\n", uuids_line(&[(BITWARDEN, UUID_BW)])));
        write(t, "extensions.json", &extensions_json(&[]));

        let report =
            apply(t, &bytes, &Selection::default(),tempdir().unwrap().path())
                .unwrap();
        assert!(report.warnings.iter().any(|w| w.contains("Dark Reader isn't installed")));
        assert!(report.warnings.iter().any(|w| w.contains("Sine isn't installed")));

        let prefs_out = std::fs::read_to_string(t.join("prefs.js")).unwrap();
        let map = ext_storage::uuid_map(&prefs_out).unwrap();
        assert_eq!(map[DARK_READER], UUID_SRC);
        assert_eq!(map[BITWARDEN], UUID_BW);
        assert_eq!(origin_in_db(t, UUID_SRC).1, "dark");
    }

    #[test]
    fn deselected_extensions_are_not_restored() {
        let source = tempdir().unwrap();
        source_profile(source.path());
        let (bytes, _) = build(source.path(), &Selection::default()).unwrap();

        let target = tempdir().unwrap();
        let t = target.path();
        write(t, "prefs.js", "");
        write(t, "extensions.json", &extensions_json(&[(DARK_READER, "Dark Reader")]));

        let selection = Selection {
            extension_overrides: BTreeMap::from([(DARK_READER.to_string(), false)]),
            ..Selection::default()
        };
        let report = apply(t, &bytes, &selection, tempdir().unwrap().path()).unwrap();
        assert_eq!(report.extension_count, 0);
        assert!(!t.join("storage").exists());
    }

    /// Target with Dark Reader under its own UUID, a mod the source also has and
    /// one only the target has.
    fn target_profile(t: &Path) {
        write(
            t,
            "prefs.js",
            &format!(
                "user_pref(\"mod.lean.top-workspace\", true);\nuser_pref(\"mod.local.only\", 5);\n{}\n",
                uuids_line(&[(DARK_READER, UUID_DST)])
            ),
        );
        write(t, "chrome/JS/sine.sys.mjs", "");
        write(t, "chrome/sine-mods/mods.json", r#"{"lean":{},"local":{}}"#);
        write(
            t,
            "chrome/sine-mods/lean/preferences.json",
            r#"[{"property":"mod.lean.hide-zoom"},{"property":"mod.lean.top-workspace"}]"#,
        );
        write(t, "chrome/sine-mods/local/preferences.json", r#"[{"property":"mod.local.only"}]"#);
        write(t, "extensions.json", &extensions_json(&[(DARK_READER, "Dark Reader")]));
        storage_folder(t, UUID_DST, "light");
        write(t, ext_storage::PERMISSIONS_FILE, "{}");
    }

    fn archive_names(bytes: &[u8]) -> Vec<String> {
        let archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        archive.file_names().map(str::to_string).collect()
    }

    #[test]
    fn backup_leaves_out_what_this_device_does_not_sync() {
        let source = tempdir().unwrap();
        source_profile(source.path());
        let selection = Selection {
            options: SyncOptions {
                sine_mods: false,
                extension_storage: false,
                ..SyncOptions::default()
            },
            ..Selection::default()
        };
        let (bytes, manifest) = build(source.path(), &selection).unwrap();
        assert!(!archive_names(&bytes).iter().any(|n| n.starts_with(MODS_PREFIX)));
        let sine = manifest.sine.as_ref().unwrap();
        assert!(!sine.mods_included);
        assert!(sine.declared_prefs.as_ref().unwrap().contains("mod.lean.top-workspace"));
        assert!(manifest.extensions[0].storage_prefix.is_none());

        let target = tempdir().unwrap();
        let t = target.path();
        target_profile(t);
        let report =
            apply(t, &bytes, &Selection::default(),tempdir().unwrap().path())
                .unwrap();
        assert_eq!(report.mod_count, 0);
        assert_eq!(report.extension_count, 1);

        // Local mods and storage stay; settings follow the source.
        assert!(t.join("chrome/sine-mods/local/preferences.json").is_file());
        assert_eq!(origin_in_db(t, UUID_DST).1, "light");
        let prefs_out = std::fs::read_to_string(t.join("prefs.js")).unwrap();
        assert!(prefs_out.contains("user_pref(\"mod.lean.hide-zoom\", true);"));
        assert!(!prefs_out.contains("mod.lean.top-workspace"));
        assert!(prefs_out.contains("user_pref(\"mod.local.only\", 5);"));
        let perms = ext_storage::read_json(&t.join(ext_storage::PERMISSIONS_FILE)).unwrap();
        assert_eq!(perms[DARK_READER]["origins"][0], "<all_urls>");

        let nothing = Selection {
            options: SyncOptions {
                sine_mods: false,
                mod_settings: false,
                extension_storage: false,
                extension_permissions: false,
                extension_shortcuts: false,
            },
            ..Selection::default()
        };
        let err = build(source.path(), &nothing).unwrap_err();
        assert!(err.contains("Nothing is selected"));
    }

    #[test]
    fn restore_skips_what_this_device_does_not_sync() {
        let source = tempdir().unwrap();
        source_profile(source.path());
        let (bytes, _) = build(source.path(), &Selection::default()).unwrap();

        let target = tempdir().unwrap();
        let t = target.path();
        target_profile(t);
        let selection = Selection {
            options: SyncOptions {
                mod_settings: false,
                extension_permissions: false,
                ..SyncOptions::default()
            },
            ..Selection::default()
        };
        let report = apply(t, &bytes, &selection, tempdir().unwrap().path()).unwrap();
        assert_eq!(report.mod_count, 1);

        assert!(t.join("chrome/sine-mods/lean/chrome.css").is_file());
        assert!(!t.join("chrome/sine-mods/local").exists());
        let prefs_out = std::fs::read_to_string(t.join("prefs.js")).unwrap();
        assert!(!prefs_out.contains("mod.lean.hide-zoom"));
        assert!(prefs_out.contains("user_pref(\"mod.lean.top-workspace\", true);"));
        assert_eq!(origin_in_db(t, UUID_DST).1, "dark");
        assert_eq!(std::fs::read_to_string(t.join(ext_storage::PERMISSIONS_FILE)).unwrap(), "{}");
    }

    #[test]
    fn restores_format_2_snapshots() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let files = [
            ("sine/mods/mods.json", r#"{"lean":{}}"#),
            (
                "sine/mods/lean/preferences.json",
                r#"[{"property":"mod.lean.hide-zoom"},{"property":"mod.lean.top-workspace"}]"#,
            ),
            (
                MANIFEST_NAME,
                r#"{"format":2,"created_at":0,"extensions":[],
                    "sine":{"engine_version":null,"mod_count":1,"prefs":{"mod.lean.hide-zoom":"true"}}}"#,
            ),
        ];
        for (name, content) in files {
            zip.start_file(name, file_options(CompressionMethod::Stored)).unwrap();
            std::io::Write::write_all(&mut zip, content.as_bytes()).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();

        let target = tempdir().unwrap();
        let t = target.path();
        target_profile(t);
        let report =
            apply(t, &bytes, &Selection::default(),tempdir().unwrap().path())
                .unwrap();
        assert_eq!(report.mod_count, 1);
        assert!(!t.join("chrome/sine-mods/local").exists());
        let prefs_out = std::fs::read_to_string(t.join("prefs.js")).unwrap();
        assert!(prefs_out.contains("user_pref(\"mod.lean.hide-zoom\", true);"));
        assert!(!prefs_out.contains("mod.lean.top-workspace"));
    }

    #[test]
    fn rejects_other_formats() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(MANIFEST_NAME, file_options(CompressionMethod::Stored)).unwrap();
        std::io::Write::write_all(&mut zip, br#"{"format":4}"#).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let dir = tempdir().unwrap();
        let err = apply(dir.path(), &bytes, &Selection::default(),dir.path())
            .unwrap_err();
        assert!(err.contains("Unsupported snapshot format 4"));
    }
}
