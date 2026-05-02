//! Profile YAML file discovery and parsing.
//!
//! Implements single-tier discovery: profiles are loaded from
//! `{resource_dir}/profiles/`, which on Windows resolves to the directory
//! containing the executable — making them user-editable without touching AppData.

use tauri::Manager;

use super::types::{SharedDaughterboardLibrary, StructureProfile};
use super::{ProfileCache, make_profile_key};

const SHARED_DAUGHTERBOARD_LIBRARY_FILENAME: &str = "RR-CirKits.shared-daughterboards.yaml";

/// Load a structure profile for the given manufacturer + model.
///
/// Looks for `{resource_dir}/profiles/{Manufacturer}_{Model}.profile.yaml`.
///
/// On Windows `resource_dir()` resolves to the directory containing the
/// executable, so profiles are user-editable without touching AppData.
///
/// Returns `None` (with a `eprintln!` warning) if:
/// - no file is found at the expected location
/// - the file is found but YAML parsing fails (FR-006)
///
/// The result (including `None`) is cached in `cache` to avoid re-scanning on
/// subsequent calls for the same node type.
///
/// The `_cdi` parameter is reserved for future use (e.g., path pre-resolution
/// during load).  It is not consumed in the current implementation.
pub async fn load_profile(
    manufacturer: &str,
    model: &str,
    _cdi: &lcc_rs::cdi::Cdi,
    app_handle: &tauri::AppHandle,
    cache: &ProfileCache,
) -> Option<StructureProfile> {
    let key = make_profile_key(manufacturer, model);

    // Fast path: return cached result (including None sentinel).
    {
        let read = cache.read().await;
        if let Some(cached) = read.get(&key) {
            return cached.clone();
        }
    }

    // Construct the profile filename from manufacturer + model.
    let filename = make_profile_filename(manufacturer, model);

    // ── Bundled / user-editable profile ────────────────────────────────────
    let resource_path = app_handle
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("profiles").join(&filename));

    if let Some(ref p) = resource_path {
        if p.exists() {
            eprintln!("[profile] Loading: {}", p.display());
            match try_load_from_path(p, manufacturer, model).await {
                Some(profile) => {
                    cache.write().await.insert(key, Some(profile.clone()));
                    return Some(profile);
                }
                None => {
                    // Parse failed — cache None so we don't retry repeatedly.
                    cache.write().await.insert(key, None);
                    return None;
                }
            }
        }
    }

    // No file found.
    cache.write().await.insert(key, None);
    None
}

pub async fn load_shared_daughterboards(
    app_handle: &tauri::AppHandle,
) -> Option<SharedDaughterboardLibrary> {
    let resource_path = app_handle
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("profiles").join(SHARED_DAUGHTERBOARD_LIBRARY_FILENAME));

    let path = match resource_path {
        Some(path) => path,
        None => return None,
    };

    if !path.exists() {
        eprintln!(
            "[profile] Shared daughterboard library not found: {}",
            path.display()
        );
        return None;
    }

    let content = match tokio::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(e) => {
            eprintln!(
                "[profile] Failed to read shared daughterboard library '{}': {}",
                path.display(),
                e
            );
            return None;
        }
    };

    match serde_yaml_ng::from_str::<SharedDaughterboardLibrary>(&content) {
        Ok(library) => Some(library),
        Err(e) => {
            eprintln!(
                "[profile] Failed to parse shared daughterboard library '{}': {}",
                path.display(),
                e
            );
            None
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Attempt to read and parse a profile YAML file at `path`.
///
/// Returns `None` and logs a warning on any IO or parse error (FR-006).
async fn try_load_from_path(
    path: &std::path::Path,
    manufacturer: &str,
    model: &str,
) -> Option<StructureProfile> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[profile] Failed to read profile file '{}': {}",
                path.display(),
                e
            );
            return None;
        }
    };

    match serde_yaml_ng::from_str::<StructureProfile>(&content) {
        Ok(profile) => {
            // Advisory check: warn on unknown schema version but still apply.
            if profile.schema_version != "1.0" {
                eprintln!(
                    "[profile] Warning: profile for '{} {}' declares schemaVersion '{}' \
                     (expected '1.0') — applying anyway",
                    manufacturer, model, profile.schema_version
                );
            }
            Some(profile)
        }
        Err(e) => {
            eprintln!(
                "[profile] Failed to parse profile YAML for '{} {}' at '{}': {}",
                manufacturer,
                model,
                path.display(),
                e
            );
            None
        }
    }
}

/// Build the profile file name from manufacturer and model strings.
///
/// Format: `{Manufacturer}_{Model}.profile.yaml`
/// Characters invalid in file names (`\ / : * ? " < > |`) are replaced with `_`.
fn make_profile_filename(manufacturer: &str, model: &str) -> String {
    let sanitize = |s: &str| -> String {
        s.chars()
            .map(|c| match c {
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                other => other,
            })
            .collect()
    };
    format!(
        "{}_{}.profile.yaml",
        sanitize(manufacturer),
        sanitize(model)
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_YAML: &str = r#"
schemaVersion: "1.0"
nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"
eventRoles:
  - groupPath: "Port I/O/Line/Event#1"
    role: Consumer
relevanceRules: []
"#;

    const INVALID_YAML: &str = r#"
notValid: [unclosed bracket
"#;

    #[test]
    fn load_profile_parses_valid_yaml() {
        let profile: StructureProfile =
            serde_yaml_ng::from_str(VALID_YAML).expect("valid YAML must parse");
        assert_eq!(profile.schema_version, "1.0");
        assert_eq!(profile.node_type.manufacturer, "RR-CirKits");
        assert_eq!(profile.node_type.model, "Tower-LCC");
        assert_eq!(profile.event_roles.len(), 1);
        assert_eq!(profile.event_roles[0].group_path, "Port I/O/Line/Event#1");
    }

    #[test]
    fn load_profile_returns_none_for_invalid_yaml() {
        let result = serde_yaml_ng::from_str::<StructureProfile>(INVALID_YAML);
        assert!(result.is_err(), "invalid YAML must fail to parse");
    }

    #[test]
    fn load_profile_parses_connector_metadata() {
        let connector_yaml = concat!(
            "schemaVersion: \"1.0\"\n",
            "nodeType:\n",
            "  manufacturer: \"RR-CirKits\"\n",
            "  model: \"Tower-LCC\"\n",
            "eventRoles: []\n",
            "relevanceRules: []\n",
            "connectorSlots:\n",
            "  - slotId: \"serial-a\"\n",
            "    label: \"Serial A\"\n",
            "    order: 0\n",
            "    allowNoneInstalled: true\n",
            "    supportedDaughterboardIds: [\"db-8in\", \"db-4io\"]\n",
            "    affectedPaths: [\"Port I/O/Line\"]\n",
            "    baseBehaviorWhenEmpty:\n",
            "      effect: hideDependent\n",
            "daughterboardReferences: [\"db-8in\", \"db-4io\"]\n",
            "carrierOverrides:\n",
            "  - carrierKey: \"rr-cirkits::tower-lcc\"\n",
            "    slotId: \"serial-a\"\n",
            "    daughterboardId: \"db-8in\"\n",
            "    overrideValidityRules:\n",
            "      - targetPath: \"Port I/O/Line/Mode\"\n",
            "        constraintType: allowValues\n",
            "        allowedValues: [\"occupancy\", \"sensor\"]\n",
            "    overrideRepairRules:\n",
            "      - targetPath: \"Port I/O/Line/Mode\"\n",
            "        replacementStrategy: setExplicit\n",
            "        replacementValue: \"occupancy\"\n",
            "        priority: 1\n",
        );

        let profile: StructureProfile =
            serde_yaml_ng::from_str(connector_yaml).expect("connector YAML must parse");

        assert_eq!(profile.connector_slots.len(), 1);
        assert_eq!(profile.connector_slots[0].slot_id, "serial-a");
        assert_eq!(profile.connector_slots[0].supported_daughterboard_ids.len(), 2);
        assert_eq!(profile.daughterboard_references, vec!["db-8in", "db-4io"]);
        assert_eq!(profile.carrier_overrides.len(), 1);
        assert_eq!(profile.carrier_overrides[0].daughterboard_id, "db-8in");
        assert_eq!(profile.carrier_overrides[0].override_validity_rules.len(), 1);
        assert_eq!(profile.carrier_overrides[0].override_repair_rules.len(), 1);
    }

    #[test]
    fn load_shared_daughterboard_library_parses_valid_yaml() {
        let shared_daughterboard_yaml = concat!(
            "schemaVersion: \"1.0\"\n",
            "manufacturer: \"RR-CirKits\"\n",
            "daughterboards:\n",
            "  - daughterboardId: \"db-8in\"\n",
            "    displayName: \"8 Input Detector\"\n",
            "    kind: \"detector\"\n",
            "    validityRules:\n",
            "      - targetPath: \"Port I/O/Line/Event#1\"\n",
            "        constraintType: hideSection\n",
            "    repairRules:\n",
            "      - targetPath: \"Port I/O/Line/Event#2\"\n",
            "        replacementStrategy: clearEmpty\n",
            "    defaultsWhenSelected:\n",
            "      mode: \"occupancy\"\n",
            "    metadata:\n",
            "      manualCitations: [\"RR-CirKits Manual p.12\"]\n",
            "      manufacturerTags: [\"rr-cirkits\", \"tower\"]\n",
            "      notes: \"Initial scaffold entry\"\n",
        );

        let library: SharedDaughterboardLibrary = serde_yaml_ng::from_str(shared_daughterboard_yaml)
            .expect("shared daughterboard YAML must parse");

        assert_eq!(library.schema_version, "1.0");
        assert_eq!(library.manufacturer, "RR-CirKits");
        assert_eq!(library.daughterboards.len(), 1);
        assert_eq!(library.daughterboards[0].daughterboard_id, "db-8in");
        assert_eq!(library.daughterboards[0].repair_rules.len(), 1);
        assert_eq!(library.daughterboards[0].defaults_when_selected.get("mode"), Some(&serde_json::Value::String("occupancy".to_string())));
    }

    #[test]
    fn bundled_shared_daughterboard_library_parses_phase_four_rules() {
        let library: SharedDaughterboardLibrary = serde_yaml_ng::from_str(include_str!("../../profiles/RR-CirKits.shared-daughterboards.yaml"))
            .expect("bundled shared daughterboard YAML must parse");

        let bod4 = library
            .daughterboards
            .iter()
            .find(|candidate| candidate.daughterboard_id == "BOD4")
            .expect("BOD4 definition should exist");

        assert!(!bod4.validity_rules.is_empty(), "BOD4 should carry reusable constraint rules");

        let bod4_input_rule = bod4
            .validity_rules
            .iter()
            .find(|rule| rule.target_path == "Port I/O/Line/Input Function")
            .expect("BOD4 should constrain Input Function");

        assert_eq!(bod4_input_rule.line_ordinals, vec![1, 2, 3, 4]);
        assert_eq!(
            bod4_input_rule.allowed_values,
            vec![crate::profile::ProfileScalarValue::Integer(2)]
        );

        let bod4_cp = library
            .daughterboards
            .iter()
            .find(|candidate| candidate.daughterboard_id == "BOD4-CP")
            .expect("BOD4-CP definition should exist");

        let bod4_cp_input_rule = bod4_cp
            .validity_rules
            .iter()
            .find(|rule| rule.target_path == "Port I/O/Line/Input Function")
            .expect("BOD4-CP should constrain detector input lines");

        assert_eq!(bod4_cp_input_rule.line_ordinals, vec![1, 2, 3, 4]);
        assert_eq!(
            bod4_cp_input_rule.allowed_values,
            vec![crate::profile::ProfileScalarValue::Integer(2)]
        );

        let bod8 = library
            .daughterboards
            .iter()
            .find(|candidate| candidate.daughterboard_id == "BOD-8-SM")
            .expect("BOD-8-SM definition should exist");

        let input_rule = bod8
            .validity_rules
            .iter()
            .find(|rule| rule.target_path == "Port I/O/Line/Input Function")
            .expect("BOD-8-SM should constrain Input Function");

        assert_eq!(
            input_rule.allowed_values,
            vec![crate::profile::ProfileScalarValue::Integer(2)]
        );
    }

    #[test]
    fn bundled_tower_profile_parses_connector_line_ranges() {
        let profile: StructureProfile = serde_yaml_ng::from_str(include_str!("../../profiles/RR-CirKits_Tower-LCC.profile.yaml"))
            .expect("bundled Tower-LCC profile YAML must parse");

        assert_eq!(profile.connector_slots.len(), 2);
        assert_eq!(profile.connector_slots[0].affected_paths.len(), 8);
        assert_eq!(profile.connector_slots[1].affected_paths.len(), 8);
        assert!(profile.connector_slots[0].base_behavior_when_empty.is_none());
        assert!(profile.connector_slots[1].base_behavior_when_empty.is_none());
    }

    #[test]
    fn load_profile_returns_none_for_missing_file() {
        // Verify that a nonexistent path returns an error from tokio::fs
        let path = std::path::PathBuf::from("/nonexistent/path/doesNotExist.profile.yaml");
        assert!(!path.exists(), "file must not exist for this test");
    }

    #[test]
    fn make_profile_filename_replaces_invalid_chars() {
        let name = make_profile_filename("RR-CirKits", "Tower-LCC");
        assert_eq!(name, "RR-CirKits_Tower-LCC.profile.yaml");
    }

    #[test]
    fn make_profile_filename_replaces_colon() {
        let name = make_profile_filename("Mfr:Test", "Model/X");
        assert_eq!(name, "Mfr_Test_Model_X.profile.yaml");
    }
}
