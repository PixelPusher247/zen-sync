use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// Minimal representation of an addon entry from extensions.json.
#[derive(Deserialize, Debug)]
struct AddonEntry {
    id: String,
    #[serde(rename = "defaultLocale")]
    default_locale: Option<AddonLocale>,
    #[serde(rename = "type")]
    addon_type: Option<String>,
    location: Option<String>,
    version: Option<String>,
    active: Option<bool>,
    #[serde(rename = "userDisabled")]
    user_disabled: Option<bool>,
    /// Icon URLs keyed by pixel size ("48", "64", "128")
    icons: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
struct AddonLocale {
    name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ExtensionsFile {
    addons: Vec<AddonEntry>,
}

#[derive(Clone, Debug)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub icon_url: Option<String>,
}

/// Extension IDs of password managers whose local storage holds account and
/// device session state. Restoring that onto another machine can sign the
/// extension out or confuse its device binding, so they are excluded by default.
const PASSWORD_MANAGER_IDS: &[&str] = &[
    "{446900e4-71c2-419f-a6a7-df9c091e268b}", // Bitwarden
    "{d634138d-c276-4fc8-924b-40a0ea21d284}", // 1Password
    "support@lastpass.com",                   // LastPass
    "keepassxc-browser@keepassxc.org",        // KeePassXC-Browser
];

/// Name fragments that catch password managers not in the ID list.
const PASSWORD_MANAGER_NAME_HINTS: &[&str] = &[
    "password", "passwort", "bitwarden", "lastpass", "keepass", "dashlane", "proton pass",
    "nordpass", "enpass", "roboform",
];

fn is_user_extension(addon: &AddonEntry) -> bool {
    // Only user-installed extensions; skip built-ins, themes, search plugins.
    if addon.addon_type.as_deref() != Some("extension") {
        return false;
    }
    let location = addon.location.as_deref().unwrap_or("");
    !matches!(
        location,
        "app-builtin"
            | "app-builtin-addons"
            | "app-system-defaults"
            | "app-system-addons"
            | "app-system-profile"
            | "app-global"
            | "winreg-app-global"
    )
}

fn best_icon(addon: &AddonEntry) -> Option<String> {
    let icons = addon.icons.as_ref()?;
    for size in ["64", "48", "128", "32"] {
        if let Some(url) = icons.get(size) {
            if !url.is_empty() {
                return Some(url.clone());
            }
        }
    }
    icons.values().next().cloned()
}

/// Parse extensions.json and return user-installed extension info.
pub fn list_extensions(profile_dir: &Path) -> Result<Vec<ExtensionInfo>, String> {
    let path = profile_dir.join("extensions.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read extensions.json: {e}"))?;
    let file: ExtensionsFile = serde_json::from_str(&content)
        .map_err(|e| format!("Could not parse extensions.json: {e}"))?;

    let mut result: Vec<ExtensionInfo> = file
        .addons
        .iter()
        .filter(|a| is_user_extension(a))
        .map(|a| ExtensionInfo {
            id: a.id.clone(),
            name: a
                .default_locale
                .as_ref()
                .and_then(|l| l.name.clone())
                .unwrap_or_else(|| a.id.clone()),
            version: a.version.clone().unwrap_or_default(),
            enabled: a.active.unwrap_or(true) && !a.user_disabled.unwrap_or(false),
            icon_url: best_icon(a),
        })
        .collect();

    result.sort_by_key(|a| a.name.to_lowercase());
    Ok(result)
}

pub fn is_password_manager(id: &str, name: &str) -> bool {
    let name = name.to_lowercase();
    PASSWORD_MANAGER_IDS.contains(&id)
        || PASSWORD_MANAGER_NAME_HINTS.iter().any(|hint| name.contains(hint))
}

/// Whether an extension's data is included. Extensions without an explicit
/// choice are included unless they look like a password manager.
pub fn is_selected(overrides: &BTreeMap<String, bool>, id: &str, name: &str) -> bool {
    overrides
        .get(id)
        .copied()
        .unwrap_or_else(|| !is_password_manager(id, name))
}

/// Record the user's selection for the installed extensions, storing only
/// choices that differ from the default. Overrides for extensions that aren't
/// installed here are kept.
pub fn update_overrides(
    overrides: &mut BTreeMap<String, bool>,
    installed: &[ExtensionInfo],
    selected_ids: &[String],
) {
    let selected: HashSet<&str> = selected_ids.iter().map(String::as_str).collect();
    for ext in installed {
        let on = selected.contains(ext.id.as_str());
        if on != is_password_manager(&ext.id, &ext.name) {
            overrides.remove(&ext.id);
        } else {
            overrides.insert(ext.id.clone(), on);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn info(id: &str, name: &str) -> ExtensionInfo {
        ExtensionInfo {
            id: id.into(),
            name: name.into(),
            version: "1.0".into(),
            enabled: true,
            icon_url: None,
        }
    }

    #[test]
    fn lists_only_user_extensions() {
        let dir = tempdir().unwrap();
        let json = serde_json::json!({
            "schemaVersion": 37,
            "addons": [
                { "id": "uBlock0@raymondhill.net", "type": "extension", "location": "app-profile",
                  "version": "1.60", "defaultLocale": { "name": "uBlock Origin" } },
                { "id": "formautofill@mozilla.org", "type": "extension", "location": "app-builtin-addons",
                  "defaultLocale": { "name": "Form Autofill" } },
                { "id": "default-theme@mozilla.org", "type": "theme", "location": "app-builtin" }
            ]
        });
        std::fs::write(dir.path().join("extensions.json"), json.to_string()).unwrap();
        let list = list_extensions(dir.path()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "uBlock Origin");
    }

    #[test]
    fn password_managers_are_excluded_by_default() {
        let none = BTreeMap::new();
        assert!(!is_selected(&none, "{446900e4-71c2-419f-a6a7-df9c091e268b}", "Bitwarden"));
        assert!(!is_selected(&none, "pm@example", "Proton Pass: Free Password Manager"));
        assert!(is_selected(&none, "addon@darkreader.org", "Dark Reader"));
    }

    #[test]
    fn overrides_store_only_non_default_choices() {
        let installed = vec![
            info("addon@darkreader.org", "Dark Reader"),
            info("{446900e4-71c2-419f-a6a7-df9c091e268b}", "Bitwarden Password Manager"),
            info("uBlock0@raymondhill.net", "uBlock Origin"),
        ];
        let mut overrides = BTreeMap::new();
        overrides.insert("not-installed@ext".to_string(), false);

        // Opt Bitwarden in, opt uBlock out, keep Dark Reader at its default.
        let selected = vec![
            "addon@darkreader.org".to_string(),
            "{446900e4-71c2-419f-a6a7-df9c091e268b}".to_string(),
        ];
        update_overrides(&mut overrides, &installed, &selected);

        assert_eq!(overrides.len(), 3);
        assert!(overrides["{446900e4-71c2-419f-a6a7-df9c091e268b}"]);
        assert!(!overrides["uBlock0@raymondhill.net"]);
        assert!(!overrides["not-installed@ext"]);
        for ext in &installed {
            assert_eq!(
                is_selected(&overrides, &ext.id, &ext.name),
                selected.contains(&ext.id)
            );
        }
    }
}
