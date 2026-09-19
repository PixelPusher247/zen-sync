use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Prefs a restore must never write, even if a mod declares them as a setting.
///
/// Device identity and sync-account keys caused the device-name bug in zync;
/// the remaining entries are Sine or extension bookkeeping that is specific to
/// the local install.
const PROTECTED_PREFIXES: &[&str] = &[
    "services.sync.",
    "identity.",
    "app.update.",
    "extensions.webextensions.",
    "sine.engine.pending-restart",
    "sine.is-cosine",
    "sine.fork-id",
];

pub fn is_protected(name: &str) -> bool {
    PROTECTED_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// Split a `user_pref("name", value);` line into its name and raw value literal.
fn parse_line(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim().strip_prefix("user_pref(\"")?;
    let key_end = rest.find('"')?;
    let key = &rest[..key_end];
    let value = rest[key_end + 1..].trim_start().strip_prefix(',')?;
    let value = value.trim_end().strip_suffix(';')?.trim_end().strip_suffix(')')?;
    Some((key, value.trim()))
}

fn format_line(name: &str, raw_value: &str) -> String {
    format!("user_pref(\"{name}\", {raw_value});")
}

/// Raw value literal of a single pref, if set. The last occurrence wins, as in Firefox.
pub fn get_raw<'a>(content: &'a str, name: &str) -> Option<&'a str> {
    content
        .lines()
        .rev()
        .filter_map(parse_line)
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

/// Raw value literals for every pref in `names` that is set in `content`.
pub fn read_values(content: &str, names: &BTreeSet<String>) -> BTreeMap<String, String> {
    content
        .lines()
        .filter_map(parse_line)
        .filter(|(key, _)| names.contains(*key))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// Set prefs to the given raw literals and delete the lines of prefs in `clear`
/// (resetting them to their defaults). Every other line is kept untouched.
pub fn apply(content: &str, set: &BTreeMap<String, String>, clear: &BTreeSet<String>) -> String {
    let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut written = BTreeSet::new();
    let mut lines = Vec::new();
    for line in content.lines() {
        if let Some((key, _)) = parse_line(line) {
            if let Some(value) = set.get(key) {
                if written.insert(key) {
                    lines.push(format_line(key, value));
                }
                continue;
            }
            if clear.contains(key) {
                continue;
            }
        }
        lines.push(line.to_string());
    }
    for (key, value) in set {
        if !written.contains(key.as_str()) {
            lines.push(format_line(key, value));
        }
    }
    let mut out = lines.join(newline);
    out.push_str(newline);
    out
}

/// Encode a string as a prefs.js string literal.
pub fn string_literal(value: &str) -> String {
    // prefs.js string escapes (\" \\ \n \r \uXXXX) are a subset of JSON's.
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

/// Decode a prefs.js string literal.
pub fn parse_string_literal(raw: &str) -> Option<String> {
    serde_json::from_str(raw).ok()
}

// ── about:config selection ────────────────────────────────────────────────────

/// Prefixes of prefs that describe this machine, this install or Firefox's own
/// bookkeeping rather than anything the user configured. They are never offered
/// as about:config settings.
///
/// The list errs towards leaving prefs out: a pref that is wrongly excluded is
/// one the user sets again by hand, while a wrongly included one can carry
/// another device's hardware, session or account state into this profile.
const DEVICE_PREFIXES: &[&str] = &[
    // Telemetry, experiments, first-run and update bookkeeping.
    "app.normandy.",
    "browser.contentblocking.cfr-milestone.",
    "browser.contextual-services.",
    "browser.laterrun.",
    "browser.migration.",
    "browser.newtabpage.activity-stream.impressionId",
    "browser.region.",
    "browser.safebrowsing.provider.",
    "browser.startup.homepage_override.",
    "browser.startup.lastColdStartupCheck",
    "datareporting.",
    "doh-rollout.",
    "messaging-system.",
    "nimbus.",
    "toolkit.telemetry.",
    // Session state, profile databases and crash history.
    "browser.sessionstore.",
    "browser.slowStartup.",
    "places.database.",
    "privacy.purge_trackers.",
    "privacy.sanitize.pending",
    "storage.vacuum.last.",
    "toolkit.crashreporter.",
    "toolkit.startup.",
    // Hardware, codecs and printers.
    "gfx.",
    "layers.",
    "media.benchmark.",
    "media.gmp",
    "print.",
    "print_printer",
    "printer_",
    // Extension state: the extension sync options carry what belongs to an
    // extension, and the Mozilla account installs the extensions themselves.
    "extensions.",
    // Per-profile or per-network identity.
    "distribution.",
    "dom.push.",
    "network.proxy.",
    // Owned by other sync options.
    "sine.",
];

/// Single prefs excluded for the same reasons as [`DEVICE_PREFIXES`], where the
/// surrounding branch holds settings worth syncing.
const DEVICE_KEYS: &[&str] = &[
    "browser.EULA.version",
    "browser.bookmarks.restore_default_bookmarks",
    "browser.download.dir",
    "browser.download.folderList",
    "browser.download.lastDir",
    "browser.shell.mostRecentDateSetAsDefault",
    "idle.lastDailyNotification",
    "media.hardware-video-decoding.failed",
    "pdfjs.migrationVersion",
    "pdfjs.previousHandler.alwaysAskBeforeHandling",
    "pdfjs.previousHandler.preferredAction",
    "security.sandbox.content.tempDirSuffix",
    "signon.importedFromSqlite",
    // The Zen shortcuts option writes this one alongside the shortcuts file.
    "zen.keyboard.shortcuts.version",
];

/// Whether a raw value literal points into the local filesystem. Prefs holding
/// a path are device state whatever they are called, so this catches ones the
/// name lists don't know about.
fn is_local_path(raw_value: &str) -> bool {
    let has_drive_letter = raw_value
        .as_bytes()
        .windows(3)
        .any(|w| w[0].is_ascii_alphabetic() && w[1] == b':' && w[2] == b'\\');
    has_drive_letter
        || raw_value.contains("file:///")
        || raw_value.to_ascii_lowercase().contains("appdata")
}

/// Whether a pref can travel between devices as an about:config setting.
pub fn is_syncable(name: &str, raw_value: &str) -> bool {
    !is_protected(name)
        && !DEVICE_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        && !DEVICE_KEYS.contains(&name)
        && !is_local_path(raw_value)
}

/// Every pref in `content` this device could sync as an about:config setting,
/// with its raw value literal. Names in `owned` are left out: another sync
/// option already carries them.
pub fn syncable(content: &str, owned: &BTreeSet<String>) -> BTreeMap<String, String> {
    content
        .lines()
        .filter_map(parse_line)
        .filter(|(name, value)| is_syncable(name, value) && !owned.contains(*name))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// Whether a syncable pref is included. Prefs without an explicit choice are.
pub fn is_selected(overrides: &BTreeMap<String, bool>, name: &str) -> bool {
    overrides.get(name).copied().unwrap_or(true)
}

/// Record the user's choice for the prefs currently on offer, storing only the
/// ones turned off. Choices for prefs this profile no longer has are kept.
pub fn update_overrides(
    overrides: &mut BTreeMap<String, bool>,
    candidates: &BTreeSet<String>,
    selected_names: &[String],
) {
    let selected: HashSet<&str> = selected_names.iter().map(String::as_str).collect();
    for name in candidates {
        if selected.contains(name.as_str()) {
            overrides.remove(name);
        } else {
            overrides.insert(name.clone(), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFS: &str = r#"// Mozilla User Preferences
user_pref("mod.lean.hide-zoom", true);
user_pref("uc.essentials.width", "Thin");
user_pref("services.sync.client.name", "This PC");
user_pref("zen.mods.AudioIndicatorEnhanced.audioWave.opacity", "0.2");
"#;

    fn names(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reads_declared_values_as_raw_literals() {
        let values = read_values(PREFS, &names(&["mod.lean.hide-zoom", "uc.essentials.width", "missing"]));
        assert_eq!(values.len(), 2);
        assert_eq!(values["mod.lean.hide-zoom"], "true");
        assert_eq!(values["uc.essentials.width"], "\"Thin\"");
    }

    #[test]
    fn apply_sets_clears_and_appends() {
        let mut set = BTreeMap::new();
        set.insert("uc.essentials.width".to_string(), "\"Normal\"".to_string());
        set.insert("better_findbar.textbox_width".to_string(), "300".to_string());
        let out = apply(PREFS, &set, &names(&["mod.lean.hide-zoom"]));

        assert!(out.starts_with("// Mozilla User Preferences\n"));
        assert!(!out.contains("mod.lean.hide-zoom"));
        assert!(out.contains(r#"user_pref("uc.essentials.width", "Normal");"#));
        assert!(!out.contains("Thin"));
        assert!(out.contains(r#"user_pref("better_findbar.textbox_width", 300);"#));
        // Untouched lines survive, including device identity.
        assert!(out.contains(r#"user_pref("services.sync.client.name", "This PC");"#));
        assert!(out.contains("zen.mods.AudioIndicatorEnhanced.audioWave.opacity"));
    }

    #[test]
    fn apply_preserves_crlf() {
        let content = "user_pref(\"a.b\", 1);\r\nuser_pref(\"c.d\", 2);\r\n";
        let mut set = BTreeMap::new();
        set.insert("a.b".to_string(), "5".to_string());
        let out = apply(content, &set, &BTreeSet::new());
        assert_eq!(out, "user_pref(\"a.b\", 5);\r\nuser_pref(\"c.d\", 2);\r\n");
    }

    #[test]
    fn values_containing_parens_and_escapes_round_trip() {
        let line = r#"user_pref("zen.mods.x.color", "color-mix(in srgb, -moz-dialogtext 50%, rgb(129, 0, 0) 50%)");"#;
        assert_eq!(
            get_raw(line, "zen.mods.x.color"),
            Some(r#""color-mix(in srgb, -moz-dialogtext 50%, rgb(129, 0, 0) 50%)""#)
        );

        let json = r#"{"a@b":"1234"}"#;
        let literal = string_literal(json);
        assert_eq!(literal, r#""{\"a@b\":\"1234\"}""#);
        assert_eq!(parse_string_literal(&literal).as_deref(), Some(json));
    }

    const PROFILE_PREFS: &str = r#"// Mozilla User Preferences
user_pref("zen.view.compact.hide-toolbar", true);
user_pref("zen.workspaces.container-specific-essentials-enabled", false);
user_pref("browser.tabs.loadInBackground", false);
user_pref("mod.lean.hide-zoom", true);
user_pref("sine.allow-unsafe-js", true);
user_pref("extensions.lastAppVersion", "1.15b");
user_pref("gfx.blacklist.layers.direct2d", 3);
user_pref("app.update.lastUpdateTime.background-update-timer", 1789000000);
user_pref("browser.download.lastDir", "C:\Users\dev\Downloads");
user_pref("zen.keyboard.shortcuts.version", 20);
user_pref("print_printer", "Brother HL-2030");
"#;

    #[test]
    fn syncable_keeps_settings_and_drops_device_state() {
        let owned = names(&["mod.lean.hide-zoom"]);
        let syncable = syncable(PROFILE_PREFS, &owned);

        let kept: Vec<&str> = syncable.keys().map(String::as_str).collect();
        assert_eq!(
            kept,
            vec![
                "browser.tabs.loadInBackground",
                "zen.view.compact.hide-toolbar",
                "zen.workspaces.container-specific-essentials-enabled",
            ]
        );
        assert_eq!(syncable["zen.view.compact.hide-toolbar"], "true");
    }

    #[test]
    fn unknown_prefs_holding_a_local_path_are_not_syncable() {
        assert!(!is_syncable("some.addon.cachePath", r#""C:\Users\dev\cache""#));
        assert!(!is_syncable("some.addon.source", r#""file:///C:/tmp/x.js""#));
        assert!(is_syncable("some.addon.mode", r#""C: drive""#));
        assert!(is_syncable("browser.urlbar.suggest.history", "false"));
    }

    #[test]
    fn pref_overrides_store_only_prefs_turned_off() {
        let candidates = names(&["zen.a", "zen.b", "zen.c"]);
        let mut overrides = BTreeMap::new();
        overrides.insert("gone.from.profile".to_string(), false);

        update_overrides(&mut overrides, &candidates, &["zen.a".to_string(), "zen.c".to_string()]);

        assert_eq!(overrides.len(), 2);
        assert!(!overrides["zen.b"]);
        assert!(is_selected(&overrides, "zen.a"));
        assert!(!is_selected(&overrides, "zen.b"));
        assert!(is_selected(&overrides, "never.seen"));
        // A choice about a pref that is no longer set here survives.
        assert!(!is_selected(&overrides, "gone.from.profile"));
    }

    #[test]
    fn protects_identity_and_bookkeeping_prefs() {
        assert!(is_protected("services.sync.client.name"));
        assert!(is_protected("identity.fxaccounts.account.device.name"));
        assert!(is_protected("extensions.webextensions.uuids"));
        assert!(is_protected("sine.engine.pending-restart"));
        assert!(!is_protected("sine.allow-unsafe-js"));
        assert!(!is_protected("mod.lean.hide-zoom"));
    }
}
