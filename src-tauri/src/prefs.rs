use std::collections::{BTreeMap, BTreeSet};

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
