/// Per-machine prefs.js key prefixes that must never leave the device.
///
/// These are device-identity or sync-account keys.  Sending them cross-machine
/// overwrites the target device's Firefox Sync account binding and device name,
/// which was the root cause of the bug in zync.
const EXCLUDED_PREFIXES: &[&str] = &[
    // Firefox / Zen Sync account + device identity
    "services.sync.",
    "identity.fxaccounts.",
    "identity.sync.",
    // Per-device update state
    "app.update.",
    "zen.updates.",
    // Telemetry IDs — machine-specific
    "toolkit.telemetry.cachedClientID",
    "toolkit.telemetry.previousBuildID",
    "toolkit.telemetry.hybridContent.enabled",
    // New-tab impression IDs
    "browser.newtabpage.activity-stream.impressionId",
    "browser.newtabpage.activity-stream.telemetry.session.transitionedToPrivacyNotice",
    // Session-restore can contain open tabs — skip (too large, machine-specific)
    "browser.sessionstore.",
    // Build-specific migration markers
    "browser.startup.homepage_override.mstone",
    "browser.startup.homepage_override.buildID",
];

/// Returns true if the given user_pref line should be excluded from the backup.
fn is_excluded_line(line: &str) -> bool {
    let t = line.trim_start();
    if !t.starts_with("user_pref(\"") {
        return false;
    }
    // Extract key name between the first pair of quotes
    let after_open = &t["user_pref(\"".len()..];
    let key_end = match after_open.find('"') {
        Some(i) => i,
        None => return false,
    };
    let key = &after_open[..key_end];
    EXCLUDED_PREFIXES.iter().any(|prefix| key.starts_with(prefix))
}

/// Strip all per-machine keys from prefs.js content before uploading.
pub fn strip_machine_prefs(content: &str) -> String {
    content
        .lines()
        .filter(|line| !is_excluded_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// After writing a restored prefs.js, re-inject the lines from the local
/// (pre-restore) prefs.js that match the excluded prefixes so the local
/// device identity is preserved.
pub fn restore_machine_prefs(restored_content: &str, local_content: &str) -> String {
    let local_machine_lines: Vec<&str> = local_content
        .lines()
        .filter(|line| is_excluded_line(line))
        .collect();

    let mut result = restored_content
        .lines()
        .filter(|line| !is_excluded_line(line))
        .collect::<Vec<_>>();

    if !local_machine_lines.is_empty() {
        result.push("");
        result.push("// Restored by zen-sync — per-device settings preserved");
        result.extend_from_slice(&local_machine_lines);
    }
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_sync_keys() {
        let prefs = r#"user_pref("browser.tabs.warnOnClose", true);
user_pref("services.sync.client.name", "My Old PC");
user_pref("identity.fxaccounts.account.device.name", "Old Device");
user_pref("zen.workspaces.show-icon-only", false);
user_pref("app.update.channel", "release");"#;

        let stripped = strip_machine_prefs(prefs);
        assert!(!stripped.contains("services.sync."));
        assert!(!stripped.contains("identity.fxaccounts."));
        assert!(!stripped.contains("app.update."));
        assert!(stripped.contains("browser.tabs.warnOnClose"));
        assert!(stripped.contains("zen.workspaces.show-icon-only"));
    }

    #[test]
    fn restores_local_machine_prefs() {
        let restored = r#"user_pref("browser.tabs.warnOnClose", true);"#;
        let local = r#"user_pref("browser.tabs.warnOnClose", false);
user_pref("services.sync.client.name", "This PC");
user_pref("identity.fxaccounts.account.device.name", "This Device");"#;

        let merged = restore_machine_prefs(restored, local);
        // Local machine keys re-injected
        assert!(merged.contains(r#"services.sync.client.name", "This PC"#));
        assert!(merged.contains(r#"identity.fxaccounts.account.device.name", "This Device"#));
        // Restored content preserved
        assert!(merged.contains("browser.tabs.warnOnClose"));
    }

    #[test]
    fn non_pref_lines_pass_through() {
        let prefs = "// comment\n\nuser_pref(\"dom.webcomponents.enabled\", true);";
        let stripped = strip_machine_prefs(prefs);
        assert!(stripped.contains("// comment"));
        assert!(stripped.contains("dom.webcomponents.enabled"));
    }
}
