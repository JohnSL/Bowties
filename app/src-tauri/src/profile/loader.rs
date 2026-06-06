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
/// In debug builds, Bowties first checks the source-tree `profiles/` directory
/// under `app/src-tauri` so profile edits are visible immediately during local
/// development even when the copied runtime resources are stale.
///
/// Returns `None` (with a `eprintln!` warning) if:
/// - no file is found at the expected location
/// - the file is found but YAML parsing fails (FR-006)
///
/// The result (including `None`) is cached in `cache` to avoid re-scanning on
/// subsequent calls for the same node type.
///
/// The `_cdi` parameter is reserved for future use.
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

    let filename = make_profile_filename(manufacturer, model);

    let search_dirs = profile_search_dirs(app_handle);
    let Some(path) = find_existing_profile_path(&search_dirs, &filename) else {
        cache.write().await.insert(key, None);
        return None;
    };

    eprintln!("[profile] Loading: {}", path.display());
    let profile = try_load_from_path(&path, manufacturer, model).await;
    cache.write().await.insert(key, profile.clone());
    profile
}

pub async fn load_shared_daughterboards(
    app_handle: &tauri::AppHandle,
) -> Option<SharedDaughterboardLibrary> {
    let search_dirs = profile_search_dirs(app_handle);
    let path = match find_existing_profile_path(&search_dirs, SHARED_DAUGHTERBOARD_LIBRARY_FILENAME) {
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

fn profile_search_dirs(app_handle: &tauri::AppHandle) -> Vec<std::path::PathBuf> {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("profiles"));

    merge_profile_search_dirs(debug_source_profiles_dir(), resource_dir)
}

fn merge_profile_search_dirs(
    debug_source_dir: Option<std::path::PathBuf>,
    resource_dir: Option<std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();

    if let Some(path) = debug_source_dir.filter(|candidate| candidate.exists()) {
        dirs.push(path);
    }

    if let Some(path) = resource_dir.filter(|candidate| candidate.exists()) {
        if !dirs.iter().any(|candidate| candidate == &path) {
            dirs.push(path);
        }
    }

    dirs
}

fn find_existing_profile_path(
    search_dirs: &[std::path::PathBuf],
    file_name: &str,
) -> Option<std::path::PathBuf> {
    search_dirs
        .iter()
        .map(|dir| dir.join(file_name))
        .find(|candidate| candidate.exists())
}

#[cfg(debug_assertions)]
fn debug_source_profiles_dir() -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("profiles"))
}

#[cfg(not(debug_assertions))]
fn debug_source_profiles_dir() -> Option<std::path::PathBuf> {
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Parse / validate (v2 schema gate)
// ─────────────────────────────────────────────────────────────────────────────

/// Accepted profile schema version.
const PROFILE_SCHEMA_VERSION: &str = "2.0";

/// v1-only top-level keys that must not appear in a v2 profile YAML with
/// non-empty values. Their continued presence in the Rust struct is an
/// intentional transition affordance — slice S5 deletes them outright.
const FORBIDDEN_V1_TOP_LEVEL_KEYS: &[&str] = &[
    "connectorSlots",
    "connectorConstraintVariants",
    "daughterboardReferences",
    "carrierOverrides",
];

/// Parse a profile YAML string under the v2 schema, enforcing:
/// - `schemaVersion` must equal `"2.0"` (legacy `"1.0"` is rejected).
/// - None of the v1 connector top-level keys may carry a non-empty value.
///
/// Returns an explicit error message on failure so the caller can log it.
/// This function is sync and free of file IO so it is trivially unit-testable.
pub fn parse_profile_yaml(content: &str) -> Result<StructureProfile, String> {
    // ── Pre-parse pass: reject leftover v1 fields when they carry data. ─────
    let raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(content)
        .map_err(|e| format!("failed to parse profile YAML: {e}"))?;

    if let Some(map) = raw.as_mapping() {
        for &key in FORBIDDEN_V1_TOP_LEVEL_KEYS {
            if let Some(value) = map.get(serde_yaml_ng::Value::String(key.to_string())) {
                if !v1_field_is_empty(value) {
                    return Err(format!(
                        "profile uses removed v1 field '{key}' — re-express under \
                         configurationModes (schema v2)"
                    ));
                }
            }
        }
    }

    // ── Typed parse + schema-version gate. ──────────────────────────────────
    let profile: StructureProfile = serde_yaml_ng::from_str(content)
        .map_err(|e| format!("failed to deserialize profile: {e}"))?;

    if profile.schema_version != PROFILE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported profile schemaVersion '{}' (expected '{}')",
            profile.schema_version, PROFILE_SCHEMA_VERSION
        ));
    }

    Ok(profile)
}

/// Treat absent / null / empty sequences / empty mappings as "field not used"
/// so leftover empty stubs from a v1 export do not poison v2 loads.
fn v1_field_is_empty(value: &serde_yaml_ng::Value) -> bool {
    match value {
        serde_yaml_ng::Value::Null => true,
        serde_yaml_ng::Value::Sequence(items) => items.is_empty(),
        serde_yaml_ng::Value::Mapping(map) => map.is_empty(),
        _ => false,
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

    match parse_profile_yaml(&content) {
        Ok(profile) => Some(profile),
        Err(e) => {
            eprintln!(
                "[profile] Rejected profile for '{} {}' at '{}': {}",
                manufacturer,
                model,
                path.display(),
                e
            );
            None
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bundled-profile listing (Spec 014 / S8 — placeholder picker)
// ─────────────────────────────────────────────────────────────────────────────

/// Summary of a bundled profile, used to populate the "Add board" picker.
///
/// Carries only the picker-relevant fields — full profile parsing is deferred
/// to the existing `load_profile` path, which the placeholder add-flow drives
/// later via the profile stem. Shape is camelCase on the wire for the Tauri
/// IPC layer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledProfileSummary {
    /// Profile filename stem (e.g. `"RR-CirKits_Tower-LCC"`) — also the
    /// stable board-model identity per FR-019.
    pub stem: String,
    /// Manufacturer string from the profile's `nodeType.manufacturer`.
    pub manufacturer: String,
    /// Model string from the profile's `nodeType.model`.
    pub model: String,
}

/// Pure helper: scan the given search directories for `*.profile.yaml` files,
/// parse each one's `nodeType` block, and return picker-ready summaries.
///
/// Sorted by `(manufacturer, model)` for stable picker ordering. Files that
/// fail to parse, files without a `nodeType` block, and the
/// `RR-CirKits.shared-daughterboards.yaml` sidecar (which is not a profile)
/// are silently skipped — listing must not fail because one bundle entry is
/// malformed. If the same stem appears in multiple search dirs, the first
/// occurrence wins (mirrors the `find_existing_profile_path` discipline).
pub fn list_bundled_profiles_in_dirs(
    search_dirs: &[std::path::PathBuf],
) -> Vec<BundledProfileSummary> {
    let mut by_stem: std::collections::BTreeMap<String, BundledProfileSummary> =
        std::collections::BTreeMap::new();

    for dir in search_dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(stem) = profile_yaml_stem(&path) else {
                continue;
            };

            if by_stem.contains_key(&stem) {
                // First-dir-wins (debug-source over packaged-resource).
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };

            let Some(summary) = parse_profile_summary(&stem, &content) else {
                continue;
            };

            by_stem.insert(stem, summary);
        }
    }

    let mut summaries: Vec<_> = by_stem.into_values().collect();
    summaries.sort_by(|left, right| {
        left.manufacturer
            .cmp(&right.manufacturer)
            .then(left.model.cmp(&right.model))
    });
    summaries
}

/// Returns the stem of a path that ends in `.profile.yaml`, or `None` for
/// any other file (including the shared-daughterboard sidecar).
fn profile_yaml_stem(path: &std::path::Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".profile.yaml")?;
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_string())
}

/// Parse just enough of a profile YAML to surface a picker summary. Tolerates
/// any schema details that aren't `nodeType.{manufacturer,model}` so a single
/// malformed deeper section doesn't drop the entry from the picker.
fn parse_profile_summary(stem: &str, content: &str) -> Option<BundledProfileSummary> {
    let raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(content).ok()?;
    let node_type = raw.get("nodeType")?;
    let manufacturer = node_type.get("manufacturer")?.as_str()?.to_string();
    let model = node_type.get("model")?.as_str()?.to_string();
    if manufacturer.is_empty() || model.is_empty() {
        return None;
    }
    Some(BundledProfileSummary {
        stem: stem.to_string(),
        manufacturer,
        model,
    })
}

/// Resolve search dirs for bundled profiles and return picker-ready summaries.
///
/// IPC entrypoint for the placeholder "Add board" picker (Spec 014 / S8).
pub fn list_bundled_profiles(app_handle: &tauri::AppHandle) -> Vec<BundledProfileSummary> {
    let dirs = profile_search_dirs(app_handle);
    list_bundled_profiles_in_dirs(&dirs)
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
    fn load_profile_parses_configuration_modes() {
        let v2_yaml = concat!(
            "schemaVersion: \"2.0\"\n",
            "nodeType:\n",
            "  manufacturer: \"RR-CirKits\"\n",
            "  model: \"Tower-LCC\"\n",
            "eventRoles: []\n",
            "relevanceRules: []\n",
            "configurationModes:\n",
            "  - id: \"serial-a\"\n",
            "    label: \"Serial A\"\n",
            "    selector:\n",
            "      kind: structuralSlot\n",
            "      slotId: \"serial-a\"\n",
            "      slotOrder: 0\n",
            "      affectedPaths: [\"Port I/O/Line\"]\n",
            "      allowNoneInstalled: true\n",
            "      baseBehaviorWhenEmpty:\n",
            "        effect: hideDependent\n",
            "    variants:\n",
            "      - id: \"db-8in\"\n",
            "        label: \"8-In\"\n",
            "        overlay: {}\n",
            "      - id: \"db-4io\"\n",
            "        label: \"4-IO\"\n",
            "        overlay: {}\n",
        );

        let profile: StructureProfile =
            serde_yaml_ng::from_str(v2_yaml).expect("v2 YAML must parse");

        assert_eq!(profile.configuration_modes.len(), 1);
        let mode = &profile.configuration_modes[0];
        assert_eq!(mode.id, "serial-a");
        assert_eq!(mode.variants.len(), 2);
        match &mode.selector {
            crate::profile::types::Selector::StructuralSlot {
                slot_id,
                affected_paths,
                allow_none_installed,
                base_behavior_when_empty,
                ..
            } => {
                assert_eq!(slot_id, "serial-a");
                assert_eq!(affected_paths, &vec!["Port I/O/Line".to_string()]);
                assert!(*allow_none_installed);
                assert!(base_behavior_when_empty.is_some());
            }
            other => panic!("expected StructuralSlot selector, got {:?}", other),
        }
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
            "    constraintVariants:\n",
            "      - variantId: \"tower-lcc-c7\"\n",
            "        replaceBaseValidityRules: true\n",
            "        validityRules:\n",
            "          - targetPath: \"Port I/O/Line/Input Function\"\n",
            "            constraintType: allowValues\n",
            "            allowedValues: [1]\n",
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
        assert_eq!(library.daughterboards[0].constraint_variants.len(), 1);
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

        let bod4_producer_trigger_rule = bod4
            .validity_rules
            .iter()
            .find(|rule| rule.target_path == "Port I/O/Line/Event#2/Upon this action")
            .expect("BOD4 should constrain producer trigger actions");

        assert_eq!(bod4_producer_trigger_rule.line_ordinals, vec![1, 2, 3, 4]);
        assert_eq!(
            bod4_producer_trigger_rule.allowed_values,
            vec![
                crate::profile::ProfileScalarValue::Integer(0),
                crate::profile::ProfileScalarValue::Integer(5),
                crate::profile::ProfileScalarValue::Integer(6),
                crate::profile::ProfileScalarValue::Integer(7),
                crate::profile::ProfileScalarValue::Integer(8),
            ]
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

        let bod4_c7_variant = bod4
            .constraint_variants
            .iter()
            .find(|variant| variant.variant_id == "tower-lcc-c7")
            .expect("BOD4 should define a Tower-LCC C7 variant");

        let bod4_c7_command_drive_rule = bod4_c7_variant
            .validity_rules
            .iter()
            .find(|rule| {
                rule.target_path
                    == "Port I/O/Line/Receiving the configured Command (C) event(s) will drive or pulse the line:"
            })
            .expect("BOD4 C7 variant should hide command-drive polarity on detector lines");

        assert_eq!(
            bod4_c7_command_drive_rule.line_ordinals,
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            bod4_c7_command_drive_rule.constraint_type,
            crate::profile::types::ConnectorConstraintType::HideSection
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
        assert!(
            bod8
                .validity_rules
                .iter()
                .all(|rule| rule.target_path != "Port I/O/Line/Delay"),
            "BOD-8-SM should not hide the Delay section because Tower-LCC uses it for input debounce"
        );

        let oi_ob_8 = library
            .daughterboards
            .iter()
            .find(|candidate| candidate.daughterboard_id == "OI-OB-8")
            .expect("OI-OB-8 definition should exist");

        let output_producer_trigger_rule = oi_ob_8
            .validity_rules
            .iter()
            .find(|rule| rule.target_path == "Port I/O/Line/Event#2/Upon this action")
            .expect("OI-OB-8 should constrain output producer trigger actions");

        assert_eq!(
            output_producer_trigger_rule.allowed_values,
            vec![
                crate::profile::ProfileScalarValue::Integer(0),
                crate::profile::ProfileScalarValue::Integer(1),
                crate::profile::ProfileScalarValue::Integer(2),
                crate::profile::ProfileScalarValue::Integer(3),
                crate::profile::ProfileScalarValue::Integer(4),
            ]
        );
    }

    #[test]
    fn bundled_tower_profile_declares_v2_configuration_modes() {
        let profile: StructureProfile = serde_yaml_ng::from_str(include_str!("../../profiles/RR-CirKits_Tower-LCC.profile.yaml"))
            .expect("bundled Tower-LCC profile YAML must parse");

        assert_eq!(profile.schema_version, "2.0");
        // Tower-LCC declares: firmware-revision + connector-a + connector-b = 3 modes.
        assert_eq!(profile.configuration_modes.len(), 3);

        let slot_modes: Vec<_> = profile
            .configuration_modes
            .iter()
            .filter_map(|mode| match &mode.selector {
                crate::profile::types::Selector::StructuralSlot { affected_paths, base_behavior_when_empty, .. } => {
                    Some((affected_paths.len(), base_behavior_when_empty.is_none()))
                }
                _ => None,
            })
            .collect();

        assert_eq!(slot_modes.len(), 2, "two connector slot modes expected");
        assert_eq!(slot_modes[0].0, 8, "connector-a affects 8 lines");
        assert_eq!(slot_modes[1].0, 8, "connector-b affects 8 lines");
        assert!(slot_modes[0].1 && slot_modes[1].1, "no base_behavior_when_empty on Tower-LCC slots");
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

    #[test]
    fn merge_profile_search_dirs_prefers_source_then_resource() {
        let temp_root = std::env::temp_dir().join(format!(
            "bowties-profile-loader-test-{}",
            std::process::id()
        ));
        let source = temp_root.join("source");
        let resource = temp_root.join("resource");

        std::fs::create_dir_all(&source).expect("source dir should be created");
        std::fs::create_dir_all(&resource).expect("resource dir should be created");

        let dirs = merge_profile_search_dirs(Some(source.clone()), Some(resource.clone()));

        assert_eq!(dirs, vec![source, resource]);

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Bundled-profile listing (Spec 014 / S8 — placeholder picker)
    // ─────────────────────────────────────────────────────────────────────────

    fn s8_make_temp_profiles_dir(suffix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bowties-list-bundled-profiles-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            suffix,
        ));
        std::fs::create_dir_all(&dir).expect("temp profiles dir should be created");
        dir
    }

    #[test]
    fn list_bundled_profiles_returns_picker_summaries_sorted_and_skips_non_profiles() {
        let dir = s8_make_temp_profiles_dir("happy");

        // Two valid profiles.
        std::fs::write(
            dir.join("Mustangpeak-Engineering_TurnoutBoss.profile.yaml"),
            "schemaVersion: \"2.0\"\nnodeType:\n  manufacturer: \"Mustangpeak Engineering\"\n  model: \"TurnoutBoss\"\neventRoles: []\nrelevanceRules: []\n",
        ).expect("write turnoutboss");

        std::fs::write(
            dir.join("RR-CirKits_Tower-LCC.profile.yaml"),
            "schemaVersion: \"2.0\"\nnodeType:\n  manufacturer: \"RR-CirKits\"\n  model: \"Tower-LCC\"\neventRoles: []\nrelevanceRules: []\n",
        ).expect("write tower-lcc");

        // Shared daughterboard sidecar — must be skipped (not a `.profile.yaml`).
        std::fs::write(
            dir.join("RR-CirKits.shared-daughterboards.yaml"),
            "schemaVersion: \"1.0\"\nmanufacturer: \"RR-CirKits\"\ndaughterboards: []\n",
        ).expect("write sidecar");

        // An unrelated file — must be skipped.
        std::fs::write(dir.join("README.md"), "not a profile").expect("write readme");

        let summaries = list_bundled_profiles_in_dirs(&[dir.clone()]);
        assert_eq!(summaries.len(), 2, "two profiles expected, got {summaries:?}");

        // Sorted by (manufacturer, model): Mustangpeak before RR-CirKits.
        assert_eq!(summaries[0].stem, "Mustangpeak-Engineering_TurnoutBoss");
        assert_eq!(summaries[0].manufacturer, "Mustangpeak Engineering");
        assert_eq!(summaries[0].model, "TurnoutBoss");
        assert_eq!(summaries[1].stem, "RR-CirKits_Tower-LCC");
        assert_eq!(summaries[1].manufacturer, "RR-CirKits");
        assert_eq!(summaries[1].model, "Tower-LCC");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_bundled_profiles_skips_malformed_yaml_and_missing_node_type() {
        let dir = s8_make_temp_profiles_dir("skips");

        // Malformed YAML — must be skipped, not crash the listing.
        std::fs::write(
            dir.join("Bad_Yaml.profile.yaml"),
            "schemaVersion: \"2.0\"\nnodeType: [unclosed",
        ).expect("write malformed");

        // Valid YAML but missing nodeType block.
        std::fs::write(
            dir.join("No_NodeType.profile.yaml"),
            "schemaVersion: \"2.0\"\neventRoles: []\n",
        ).expect("write no-nodetype");

        // One valid profile so the function still has something to return.
        std::fs::write(
            dir.join("Acme_Widget.profile.yaml"),
            "schemaVersion: \"2.0\"\nnodeType:\n  manufacturer: \"Acme\"\n  model: \"Widget\"\neventRoles: []\nrelevanceRules: []\n",
        ).expect("write acme");

        let summaries = list_bundled_profiles_in_dirs(&[dir.clone()]);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].stem, "Acme_Widget");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_bundled_profiles_first_dir_wins_for_duplicate_stems() {
        let source = s8_make_temp_profiles_dir("dup-source");
        let resource = s8_make_temp_profiles_dir("dup-resource");

        std::fs::write(
            source.join("Acme_Widget.profile.yaml"),
            "schemaVersion: \"2.0\"\nnodeType:\n  manufacturer: \"Acme (source)\"\n  model: \"Widget\"\neventRoles: []\nrelevanceRules: []\n",
        ).expect("write source");

        std::fs::write(
            resource.join("Acme_Widget.profile.yaml"),
            "schemaVersion: \"2.0\"\nnodeType:\n  manufacturer: \"Acme (resource)\"\n  model: \"Widget\"\neventRoles: []\nrelevanceRules: []\n",
        ).expect("write resource");

        let summaries = list_bundled_profiles_in_dirs(&[source.clone(), resource.clone()]);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].manufacturer, "Acme (source)");

        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&resource);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // v2 schema gate (S1 — slice 014/S1)
    // ─────────────────────────────────────────────────────────────────────────

    const V2_MINIMAL_YAML: &str = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "Mustangpeak Engineering"
  model: "TurnoutBoss"
eventRoles:
  - groupPath: "Layout Configuration Setup/Some Group/Event#1"
    role: Consumer
relevanceRules:
  - id: "R001"
    affectedTarget: "Detectors/Detector#3"
    allOf:
      - field: "Layout Configuration Setup/Side"
        irrelevantWhen: [1]
    explanation: "Detector 3 is unused on the Right side."
configurationModes:
  - id: "turnoutboss-side"
    label: "Used as Left or Right"
    selector:
      kind: enumField
      fieldPath: "Layout Configuration Setup/Side"
    variants:
      - id: "0"
        label: "Left"
        overlay:
          eventRoles: []
          relevanceRules: []
          structuralConstraints: []
      - id: "1"
        label: "Right"
        overlay:
          eventRoles:
            - groupPath: "Detectors/Detector#1/Occupancy"
              role: Producer
          relevanceRules: []
          structuralConstraints: []
"#;

    #[test]
    fn parse_profile_yaml_accepts_v2_minimal() {
        let profile = parse_profile_yaml(V2_MINIMAL_YAML)
            .expect("v2 minimal profile must parse");

        assert_eq!(profile.schema_version, "2.0");
        assert_eq!(profile.node_type.model, "TurnoutBoss");

        // Per-leaf event-role override and cross-segment relevance rule survive
        // round-trip with their relaxed v2 shape.
        assert_eq!(
            profile.event_roles[0].group_path,
            "Layout Configuration Setup/Some Group/Event#1"
        );
        assert_eq!(
            profile.relevance_rules[0].affected_target,
            "Detectors/Detector#3"
        );
        assert_eq!(
            profile.relevance_rules[0].all_of[0].field,
            "Layout Configuration Setup/Side"
        );

        // Configuration mode + variants + tagged Selector round-trip cleanly.
        assert_eq!(profile.configuration_modes.len(), 1);
        let mode = &profile.configuration_modes[0];
        assert_eq!(mode.id, "turnoutboss-side");
        assert_eq!(mode.variants.len(), 2);
        match &mode.selector {
            crate::profile::Selector::EnumField { field_path } => {
                assert_eq!(field_path, "Layout Configuration Setup/Side");
            }
            other => panic!("expected EnumField selector, got {other:?}"),
        }
        assert_eq!(
            mode.variants[1].overlay.event_roles[0].group_path,
            "Detectors/Detector#1/Occupancy"
        );
    }

    #[test]
    fn parse_profile_yaml_round_trips_through_serde_yaml_ng() {
        let parsed = parse_profile_yaml(V2_MINIMAL_YAML)
            .expect("v2 minimal profile must parse");
        let reserialized =
            serde_yaml_ng::to_string(&parsed).expect("v2 profile must reserialize");
        // The re-emitted YAML hits the schema gate on the way back in,
        // proving the in-memory shape stays inside the v2 contract.
        let reparsed = parse_profile_yaml(&reserialized)
            .expect("reserialized profile must parse");
        assert_eq!(reparsed.configuration_modes.len(), 1);
        assert_eq!(
            reparsed.relevance_rules[0].affected_target,
            "Detectors/Detector#3"
        );
    }

    #[test]
    fn parse_profile_yaml_rejects_v1_schema_version() {
        let v1_yaml = r#"
schemaVersion: "1.0"
nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"
"#;
        let err = parse_profile_yaml(v1_yaml)
            .expect_err("schemaVersion '1.0' must be rejected under v2");
        assert!(
            err.contains("schemaVersion") && err.contains("1.0"),
            "error must name the offending schemaVersion, got: {err}"
        );
    }

    #[test]
    fn parse_profile_yaml_rejects_leftover_v1_connector_fields() {
        let yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"
connectorSlots:
  - slotId: "serial-a"
    label: "Serial A"
    order: 0
    allowNoneInstalled: true
    supportedDaughterboardIds: ["db-8in"]
    affectedPaths: ["Port I/O/Line"]
"#;
        let err = parse_profile_yaml(yaml)
            .expect_err("leftover v1 connectorSlots must be rejected under v2");
        assert!(
            err.contains("connectorSlots"),
            "error must name the offending v1 key, got: {err}"
        );
    }

    #[test]
    fn parse_profile_yaml_tolerates_empty_v1_field_stubs() {
        // Empty v1 stubs (e.g. left over from a hand-written template) are
        // ignored — only non-empty v1 fields are rejected. This keeps the
        // failure mode narrow to "user data still living under v1 keys".
        let yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"
connectorSlots: []
carrierOverrides: []
daughterboardReferences: []
"#;
        parse_profile_yaml(yaml)
            .expect("empty v1 stubs must not block a v2 profile from parsing");
    }

    #[test]
    fn parse_profile_yaml_accepts_legacy_affected_group_path_alias() {
        // Transitional safety net: v2 names the field `affectedTarget`, but
        // any in-flight v1 fixture using `affectedGroupPath` still
        // deserializes through serde's alias until S5 drops it.
        let yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "Test"
  model: "Demo"
relevanceRules:
  - id: "R001"
    affectedGroupPath: "SegA/Field"
    allOf:
      - field: "SegB/Selector"
        irrelevantWhen: [0]
    explanation: "legacy spelling"
"#;
        let profile = parse_profile_yaml(yaml)
            .expect("legacy affectedGroupPath alias must still parse under v2");
        assert_eq!(profile.relevance_rules[0].affected_target, "SegA/Field");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // S7: bundled TurnoutBoss profile — Left vs Right variant flip
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn bundled_turnoutboss_profile_v2_left_right_flip_reshapes_occupancy_and_detector3() {
        let profile: StructureProfile = serde_yaml_ng::from_str(include_str!(
            "../../profiles/Mustangpeak-Engineering_TurnoutBoss.profile.yaml"
        ))
        .expect("bundled TurnoutBoss profile YAML must parse");

        assert_eq!(profile.schema_version, "2.0");
        assert_eq!(profile.node_type.model, "TurnoutBoss");

        // Exactly one ConfigurationMode (Left vs Right side of the siding).
        assert_eq!(profile.configuration_modes.len(), 1);
        let side_mode = &profile.configuration_modes[0];
        assert_eq!(side_mode.variants.len(), 2);
        assert!(matches!(
            side_mode.selector,
            crate::profile::types::Selector::EnumField { .. }
        ));
        // Variant ids match the controlling int's CDI enum values (0 = Left, 1 = Right).
        let variant_ids: Vec<&str> =
            side_mode.variants.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(variant_ids, vec!["0", "1"]);

        // R001 — Detector 3 base relevance rule, irrelevant when the side
        // field is 1 ("Right" board).
        let r001 = profile
            .relevance_rules
            .iter()
            .find(|rule| rule.id == "R001")
            .expect("R001 must be declared in base relevance rules");
        assert!(
            r001.affected_target.ends_with("Detector 3"),
            "R001 must target Detector 3, got '{}'",
            r001.affected_target
        );
        assert_eq!(r001.all_of.len(), 1);
        assert_eq!(r001.all_of[0].irrelevant_when, vec![1]);

        // Variant flip: Left vs Right reshapes the Occupancy event-role group
        // (FR-006: deterministic last-write-wins).
        const OCCUPANCY_PATH: &str = "Producers and Consumers/Occupancy";

        let mut left = std::collections::BTreeMap::new();
        left.insert(side_mode.id.clone(), "0".to_string());
        let composed_left = crate::profile::compose_overlays(&profile, &left);
        let left_role = composed_left
            .event_roles
            .get(OCCUPANCY_PATH)
            .expect("Left variant overlay must declare an Occupancy role");
        assert_eq!(
            left_role.role,
            crate::profile::types::ProfileEventRole::Producer,
            "Left board produces occupancy events for the blocks it monitors"
        );

        let mut right = std::collections::BTreeMap::new();
        right.insert(side_mode.id.clone(), "1".to_string());
        let composed_right = crate::profile::compose_overlays(&profile, &right);
        let right_role = composed_right
            .event_roles
            .get(OCCUPANCY_PATH)
            .expect("Right variant overlay must declare an Occupancy role");
        assert_eq!(
            right_role.role,
            crate::profile::types::ProfileEventRole::Consumer,
            "Right board consumes occupancy events from its partner across the siding"
        );

        assert!(
            composed_left.unknown_variants.is_empty()
                && composed_right.unknown_variants.is_empty(),
            "no unknown-variant warnings expected"
        );
    }
}
