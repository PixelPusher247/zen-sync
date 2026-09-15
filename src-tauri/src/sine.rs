//! Sine mod manager layout.
//!
//! `chrome/sine-mods/` holds one folder per installed mod plus `mods.json`
//! (the registry). Mod setting *values* are ordinary prefs in prefs.js, under
//! whatever names each mod declares as `property` in its `preferences.json`.
//! Sine's own engine lives in `chrome/JS/` and is installed per machine.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::{fsutil, prefs};

/// Files Sine regenerates on every startup. They contain absolute `file:///`
/// paths into the local profile, so they must not travel between machines.
const GENERATED_FILES: &[&str] = &["chrome.css", "content.css"];

pub fn mods_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("chrome").join("sine-mods")
}

fn engine_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("chrome").join("JS")
}

/// True if the profile has a Sine mod registry worth backing up.
pub fn has_mods(profile_dir: &Path) -> bool {
    mods_dir(profile_dir).join("mods.json").is_file()
}

pub fn engine_installed(profile_dir: &Path) -> bool {
    engine_dir(profile_dir).join("sine.sys.mjs").is_file()
}

pub fn engine_version(profile_dir: &Path) -> Option<String> {
    let value = read_json(&engine_dir(profile_dir).join("engine.json"))?;
    value.get("version")?.as_str().map(str::to_string)
}

pub fn mod_count(profile_dir: &Path) -> usize {
    read_json(&mods_dir(profile_dir).join("mods.json"))
        .and_then(|v| v.as_object().map(|m| m.len()))
        .unwrap_or(0)
}

/// Everything under `chrome/sine-mods/` except Sine's generated stylesheets.
pub fn mod_files(profile_dir: &Path) -> Vec<fsutil::WalkEntry> {
    fsutil::walk(&mods_dir(profile_dir))
        .into_iter()
        .filter(|e| !GENERATED_FILES.contains(&e.rel.as_str()))
        .collect()
}

/// Names of all prefs declared as settings by installed mods and by Sine itself.
///
/// Every `preferences.json` under `sine-mods/` is scanned, including ones that
/// `mods.json` doesn't reference (multi-mod repos keep them in subfolders).
/// Files that fail to parse are skipped.
pub fn declared_prefs(profile_dir: &Path) -> BTreeSet<String> {
    let mut sources: Vec<PathBuf> = fsutil::walk(&mods_dir(profile_dir))
        .into_iter()
        .filter(|e| !e.is_dir && e.rel.rsplit('/').next() == Some("preferences.json"))
        .map(|e| e.path)
        .collect();
    sources.push(engine_dir(profile_dir).join("core").join("settings.json"));

    let mut names = BTreeSet::new();
    for path in sources {
        match read_json(&path) {
            Some(value) => collect_properties(&value, &mut names),
            None if path.exists() => {
                crate::zslog!("[sine] skipping unreadable {}", path.display());
            }
            None => {}
        }
    }
    names.retain(|name| !prefs::is_protected(name));
    names
}

fn collect_properties(value: &serde_json::Value, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(name)) = map.get("property") {
                out.insert(name.clone());
            }
            map.values().for_each(|v| collect_properties(v, out));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect_properties(v, out)),
        _ => {}
    }
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn collects_declared_prefs_from_all_preference_files() {
        let profile = tempdir().unwrap();
        let p = profile.path();
        write(p, "chrome/sine-mods/mods.json", r#"{"lean":{},"multi":{}}"#);
        write(
            p,
            "chrome/sine-mods/lean/preferences.json",
            r#"[{"type":"text","label":"x"},{"property":"mod.lean.hide-zoom","type":"checkbox"}]"#,
        );
        // Nested in a multi-mod repo and referenced only through conditions.
        write(
            p,
            "chrome/sine-mods/multi/Sub/preferences.json",
            r#"[{"property":"uc.sub.a","conditions":[{"if":{"property":"uc.sub.b","value":true}}]}]"#,
        );
        write(p, "chrome/sine-mods/multi/[Config] X/preferences.json", "[{ broken json");
        write(
            p,
            "chrome/JS/core/settings.json",
            r#"[{"property":"sine.allow-unsafe-js"},{"property":"sine.engine.pending-restart"}]"#,
        );
        write(p, "chrome/JS/engine.json", r#"{"version":"2.3.4.1c","type":0}"#);

        let names = declared_prefs(p);
        let expected: BTreeSet<String> =
            ["mod.lean.hide-zoom", "uc.sub.a", "uc.sub.b", "sine.allow-unsafe-js"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        assert_eq!(names, expected);
        assert_eq!(mod_count(p), 2);
        assert_eq!(engine_version(p).as_deref(), Some("2.3.4.1c"));
    }

    #[test]
    fn mod_files_skip_generated_stylesheets() {
        let profile = tempdir().unwrap();
        let p = profile.path();
        write(p, "chrome/sine-mods/mods.json", "{}");
        write(p, "chrome/sine-mods/chrome.css", "@import \"file:///C:/Users/x/...\";");
        write(p, "chrome/sine-mods/content.css", "");
        write(p, "chrome/sine-mods/lean/chrome.css", "/* mod css */");

        let rels: Vec<String> = mod_files(p).into_iter().map(|e| e.rel).collect();
        assert_eq!(rels, vec!["lean", "lean/chrome.css", "mods.json"]);
    }
}
