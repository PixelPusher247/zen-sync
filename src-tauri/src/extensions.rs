use serde::{Deserialize, Serialize};
use std::path::Path;

/// Minimal representation of an addon entry from extensions.json.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddonEntry {
    pub id: String,
    #[serde(rename = "defaultLocale")]
    pub default_locale: Option<AddonLocale>,
    #[serde(rename = "type")]
    pub addon_type: Option<String>,
    /// "app-builtin", "app-system-defaults", "app-global", "winreg-app-global" → skip
    pub location: Option<String>,
    pub version: Option<String>,
    pub active: Option<bool>,
    #[serde(rename = "userDisabled")]
    pub user_disabled: Option<bool>,
    /// Icon URLs keyed by pixel size ("48", "64", "128")
    pub icons: Option<std::collections::HashMap<String, String>>,
    /// Preserve all other fields verbatim during filter/merge
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddonLocale {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// The top-level shape of extensions.json
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExtensionsFile {
    #[serde(rename = "schemaVersion")]
    pub schema_version: Option<u32>,
    pub addons: Vec<AddonEntry>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Flat info the frontend needs for displaying the extension list.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub icon_url: Option<String>,
}

fn is_user_extension(addon: &AddonEntry) -> bool {
    // Only user-installed extensions; skip built-ins, themes, search plugins.
    let addon_type = addon.addon_type.as_deref().unwrap_or("");
    if addon_type != "extension" {
        return false;
    }
    let location = addon.location.as_deref().unwrap_or("");
    !matches!(
        location,
        "app-builtin"
            | "app-system-defaults"
            | "app-global"
            | "winreg-app-global"
            | "app-system-addons"
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
        .map(|a| {
            let name = a
                .default_locale
                .as_ref()
                .and_then(|l| l.name.clone())
                .unwrap_or_else(|| a.id.clone());
            ExtensionInfo {
                id: a.id.clone(),
                name,
                version: a.version.clone().unwrap_or_default(),
                enabled: a.active.unwrap_or(true) && !a.user_disabled.unwrap_or(false),
                icon_url: best_icon(a),
            }
        })
        .collect();

    result.sort_by_key(|a| a.name.to_lowercase());
    Ok(result)
}

/// Filter an extensions.json byte payload to only include the given addon IDs
/// (plus all non-extension entries like themes/search plugins which are always kept).
/// Returns the filtered JSON bytes.
pub fn filter_extensions(content: &[u8], selected_ids: &[String]) -> Result<Vec<u8>, String> {
    if selected_ids.is_empty() {
        // Empty selection means "all" — return as-is
        return Ok(content.to_vec());
    }
    let mut file: ExtensionsFile = serde_json::from_slice(content)
        .map_err(|e| format!("extensions.json parse error: {e}"))?;

    let id_set: std::collections::HashSet<&str> =
        selected_ids.iter().map(|s| s.as_str()).collect();

    file.addons.retain(|a| {
        // Always keep non-user-extension entries (themes, search engines, built-ins)
        !is_user_extension(a) || id_set.contains(a.id.as_str())
    });

    serde_json::to_vec(&file).map_err(|e| format!("extensions.json serialize error: {e}"))
}

/// Merge restored extension entries into the local extensions.json.
///
/// For each addon in `restored` that is a user extension:
///   - If its ID is in `selected_ids` (or selected_ids is empty = all), replace/add it
///     in the local file.
///   - All local addons not in the restored set are preserved as-is.
///
/// This prevents overwriting extensions the user chose not to sync.
pub fn merge_extensions(
    restored_content: &[u8],
    local_content: &[u8],
    selected_ids: &[String],
) -> Result<Vec<u8>, String> {
    let restored: ExtensionsFile = serde_json::from_slice(restored_content)
        .map_err(|e| format!("Restored extensions.json parse error: {e}"))?;
    let mut local: ExtensionsFile = serde_json::from_slice(local_content)
        .map_err(|e| format!("Local extensions.json parse error: {e}"))?;

    let id_set: std::collections::HashSet<&str> = if selected_ids.is_empty() {
        // All user extensions in the restored bundle are applied
        restored
            .addons
            .iter()
            .filter(|a| is_user_extension(a))
            .map(|a| a.id.as_str())
            .collect()
    } else {
        selected_ids.iter().map(|s| s.as_str()).collect()
    };

    // Build a map of restored addons by ID
    let restored_map: std::collections::HashMap<&str, &AddonEntry> = restored
        .addons
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect();

    // Replace matching local entries with restored versions
    for local_addon in &mut local.addons {
        if id_set.contains(local_addon.id.as_str()) {
            if let Some(&restored_addon) = restored_map.get(local_addon.id.as_str()) {
                *local_addon = restored_addon.clone();
            }
        }
    }

    // Append restored addons that don't exist locally yet
    let local_ids: std::collections::HashSet<String> =
        local.addons.iter().map(|a| a.id.clone()).collect();
    for restored_addon in &restored.addons {
        if id_set.contains(restored_addon.id.as_str())
            && !local_ids.contains(restored_addon.id.as_str())
        {
            local.addons.push(restored_addon.clone());
        }
    }

    serde_json::to_vec(&local).map_err(|e| format!("extensions.json serialize error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_extension(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "type": "extension",
            "location": "profile",
            "version": "1.0",
            "active": true,
            "userDisabled": false,
            "defaultLocale": { "name": name }
        })
    }

    fn make_file(addons: Vec<serde_json::Value>) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 36,
            "addons": addons
        }))
        .unwrap()
    }

    #[test]
    fn filter_keeps_selected() {
        let content = make_file(vec![
            make_extension("ext-a@test", "Ext A"),
            make_extension("ext-b@test", "Ext B"),
            make_extension("ext-c@test", "Ext C"),
        ]);
        let selected = vec!["ext-a@test".to_string(), "ext-c@test".to_string()];
        let filtered = filter_extensions(&content, &selected).unwrap();
        let file: ExtensionsFile = serde_json::from_slice(&filtered).unwrap();
        let ids: Vec<&str> = file.addons.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"ext-a@test"));
        assert!(!ids.contains(&"ext-b@test"));
        assert!(ids.contains(&"ext-c@test"));
    }

    #[test]
    fn filter_empty_selection_keeps_all() {
        let content = make_file(vec![
            make_extension("ext-a@test", "Ext A"),
            make_extension("ext-b@test", "Ext B"),
        ]);
        let filtered = filter_extensions(&content, &[]).unwrap();
        let file: ExtensionsFile = serde_json::from_slice(&filtered).unwrap();
        assert_eq!(file.addons.len(), 2);
    }

    #[test]
    fn merge_replaces_selected_preserves_others() {
        let restored_content = make_file(vec![
            make_extension("ext-a@test", "Ext A v2"),
            make_extension("ext-b@test", "Ext B v2"),
        ]);
        let local_content = make_file(vec![
            make_extension("ext-a@test", "Ext A v1"),
            make_extension("ext-c@test", "Ext C local"),
        ]);
        // Only sync ext-a; ext-b should not appear, ext-c should be preserved
        let selected = vec!["ext-a@test".to_string()];
        let merged = merge_extensions(&restored_content, &local_content, &selected).unwrap();
        let file: ExtensionsFile = serde_json::from_slice(&merged).unwrap();
        let by_id: std::collections::HashMap<&str, &AddonEntry> =
            file.addons.iter().map(|a| (a.id.as_str(), a)).collect();
        // ext-a updated to v2
        let a = by_id["ext-a@test"];
        assert_eq!(a.default_locale.as_ref().unwrap().name.as_deref(), Some("Ext A v2"));
        // ext-b not merged
        assert!(!by_id.contains_key("ext-b@test"));
        // ext-c preserved
        assert!(by_id.contains_key("ext-c@test"));
    }
}
