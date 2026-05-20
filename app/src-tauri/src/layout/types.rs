//! Layout file types for YAML persistence.
//!
//! These types define the structure of user-managed `.bowties.yaml` layout files.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

/// Current schema version for layout files.
pub const SCHEMA_VERSION: &str = "1.0";

/// Root structure for the YAML layout file.
///
/// Example YAML:
/// ```yaml
/// schemaVersion: "1.0"
/// bowties:
///   "05.01.01.01.FF.00.00.01":
///     name: "Yard Entry Signal"
///     tags: ["yard", "signals"]
/// roleClassifications:
///   "05.02.01.02.03.00:Port I/O/Line #1/Event Produced":
///     role: "Producer"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFile {
    pub schema_version: String,
    #[serde(default)]
    pub bowties: BTreeMap<String, BowtieMetadata>,
    #[serde(default)]
    pub role_classifications: BTreeMap<String, RoleClassification>,
    #[serde(default)]
    pub connector_selections: BTreeMap<String, NodeHardwareSelectionSet>,
}

impl Default for LayoutFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            bowties: BTreeMap::new(),
            role_classifications: BTreeMap::new(),
            connector_selections: BTreeMap::new(),
        }
    }
}

/// Saved per-node connector daughterboard assumptions for one layout context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeHardwareSelectionSet {
    pub carrier_key: String,
    #[serde(default)]
    pub slot_selections: BTreeMap<String, ConnectorSelectionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// One persisted connector selection for a specific slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSelectionRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_daughterboard_id: Option<String>,
    pub status: ConnectorSelectionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_profile_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSelectionStatus {
    Selected,
    None,
    Unknown,
}

/// Metadata for a single bowtie, stored in layout YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BowtieMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// User-provided role classification for an ambiguous event slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleClassification {
    pub role: String,
}

/// A single edit operation to apply to a layout file.
///
/// The frontend sends a list of these deltas instead of a full `LayoutFile`.
/// The backend reads the current layout from disk, applies the deltas in order,
/// and writes the result (ADR-0002).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum LayoutEditDelta {
    /// Create a new bowtie entry (or no-op if it already exists).
    #[serde(rename_all = "camelCase")]
    CreateBowtie {
        event_id_hex: String,
        #[serde(default)]
        name: Option<String>,
    },
    /// Delete a bowtie entry.
    #[serde(rename_all = "camelCase")]
    DeleteBowtie { event_id_hex: String },
    /// Rename an existing bowtie (creates if absent).
    #[serde(rename_all = "camelCase")]
    RenameBowtie {
        event_id_hex: String,
        new_name: String,
    },
    /// Add a tag to a bowtie.
    #[serde(rename_all = "camelCase")]
    AddTag { event_id_hex: String, tag: String },
    /// Remove a tag from a bowtie.
    #[serde(rename_all = "camelCase")]
    RemoveTag { event_id_hex: String, tag: String },
    /// Classify an event slot's role.
    ClassifyRole { key: String, role: String },
    /// Set connector selection for a node.
    #[serde(rename_all = "camelCase")]
    SetConnectorSelection {
        node_id: String,
        selection: NodeHardwareSelectionSet,
    },
    /// Adopt a new event ID — move bowtie metadata from old key to new key.
    #[serde(rename_all = "camelCase")]
    AdoptEventId {
        old_event_id_hex: String,
        new_event_id_hex: String,
    },
}

/// Apply a sequence of edit deltas to a layout file, mutating it in place.
pub fn apply_layout_deltas(layout: &mut LayoutFile, deltas: Vec<LayoutEditDelta>) {
    for delta in deltas {
        match delta {
            LayoutEditDelta::CreateBowtie {
                event_id_hex,
                name,
            } => {
                layout
                    .bowties
                    .entry(event_id_hex)
                    .or_insert_with(|| BowtieMetadata {
                        name,
                        tags: vec![],
                    });
            }
            LayoutEditDelta::DeleteBowtie { event_id_hex } => {
                layout.bowties.remove(&event_id_hex);
            }
            LayoutEditDelta::RenameBowtie {
                event_id_hex,
                new_name,
            } => {
                let entry =
                    layout
                        .bowties
                        .entry(event_id_hex)
                        .or_insert_with(|| BowtieMetadata {
                            name: None,
                            tags: vec![],
                        });
                entry.name = Some(new_name);
            }
            LayoutEditDelta::AddTag { event_id_hex, tag } => {
                if let Some(entry) = layout.bowties.get_mut(&event_id_hex) {
                    if !entry.tags.contains(&tag) {
                        entry.tags.push(tag);
                    }
                }
            }
            LayoutEditDelta::RemoveTag { event_id_hex, tag } => {
                if let Some(entry) = layout.bowties.get_mut(&event_id_hex) {
                    entry.tags.retain(|t| t != &tag);
                }
            }
            LayoutEditDelta::ClassifyRole { key, role } => {
                layout
                    .role_classifications
                    .insert(key, RoleClassification { role });
            }
            LayoutEditDelta::SetConnectorSelection {
                node_id,
                selection,
            } => {
                layout.connector_selections.insert(node_id, selection);
            }
            LayoutEditDelta::AdoptEventId {
                old_event_id_hex,
                new_event_id_hex,
            } => {
                if let Some(meta) = layout.bowties.remove(&old_event_id_hex) {
                    layout.bowties.insert(new_event_id_hex, meta);
                }
            }
        }
    }
}

/// Recent layout file reference, stored in app data dir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentLayout {
    pub path: String,
    pub last_opened: String,
}

/// Check if a string is a valid dotted-hex event ID (e.g. "05.01.01.01.FF.00.00.01").
fn is_valid_event_id_hex(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 8 {
        return false;
    }
    parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Check if a bowtie key is acceptable: either a valid dotted-hex event ID or
/// a planning placeholder of the form "planning-<digits>".
fn is_valid_bowtie_key(s: &str) -> bool {
    is_valid_event_id_hex(s)
        || (s.starts_with("planning-") && s[9..].chars().all(|c| c.is_ascii_digit()))
}

impl LayoutFile {
    /// Validate schema version and basic structure.
    /// Returns Ok(()) if valid, Err with description otherwise.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "Unsupported layout schema version: {} (expected {})",
                self.schema_version, SCHEMA_VERSION
            ));
        }

        // Validate bowtie keys match event ID hex format or planning placeholder format
        for key in self.bowties.keys() {
            if !is_valid_bowtie_key(key) {
                return Err(format!(
                    "Invalid bowtie key '{}': must be dotted hex (e.g. 05.01.01.01.FF.00.00.01) or a planning placeholder (e.g. planning-1234567890)",
                    key
                ));
            }
        }

        // Validate role classification values
        for (key, rc) in &self.role_classifications {
            if rc.role != "Producer" && rc.role != "Consumer" {
                return Err(format!(
                    "Invalid role '{}' for classification '{}': must be 'Producer' or 'Consumer'",
                    rc.role, key
                ));
            }
        }

        for (node_id, selections) in &self.connector_selections {
            for (slot_id, selection) in &selections.slot_selections {
                if selection.status == ConnectorSelectionStatus::Selected
                    && selection.selected_daughterboard_id.is_none()
                {
                    return Err(format!(
                        "Invalid connector selection for node '{}' slot '{}': selected status requires selectedDaughterboardId",
                        node_id, slot_id
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_validates() {
        let layout = LayoutFile::default();
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn invalid_schema_version() {
        let layout = LayoutFile {
            schema_version: "2.0".to_string(),
            ..Default::default()
        };
        assert!(layout.validate().unwrap_err().contains("Unsupported"));
    }

    #[test]
    fn invalid_bowtie_key() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("not-hex".to_string(), BowtieMetadata {
            name: None,
            tags: vec![],
        });
        assert!(layout.validate().unwrap_err().contains("Invalid bowtie key"));
    }

    #[test]
    fn planning_bowtie_key_is_valid() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("planning-1774043332542".to_string(), BowtieMetadata {
            name: Some("My planning bowtie".to_string()),
            tags: vec![],
        });
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn invalid_role_classification() {
        let mut layout = LayoutFile::default();
        layout.role_classifications.insert(
            "05.02.01.02.03.00:path".to_string(),
            RoleClassification { role: "Both".to_string() },
        );
        assert!(layout.validate().unwrap_err().contains("Invalid role"));
    }

    #[test]
    fn valid_roundtrip() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert(
            "05.01.01.01.FF.00.00.01".to_string(),
            BowtieMetadata {
                name: Some("Test Bowtie".to_string()),
                tags: vec!["yard".to_string()],
            },
        );
        layout.role_classifications.insert(
            "05.02.01.02.03.00:Port/Line/Event".to_string(),
            RoleClassification { role: "Producer".to_string() },
        );
        assert!(layout.validate().is_ok());

        let yaml = serde_yaml_ng::to_string(&layout).unwrap();
        let parsed: LayoutFile = serde_yaml_ng::from_str(&yaml).unwrap();
        assert!(parsed.validate().is_ok());
        assert_eq!(parsed.bowties.len(), 1);
        assert_eq!(parsed.role_classifications.len(), 1);
    }

    #[test]
    fn connector_selection_roundtrip() {
        let mut layout = LayoutFile::default();
        let mut slot_selections = BTreeMap::new();
        slot_selections.insert(
            "serial-a".to_string(),
            ConnectorSelectionRecord {
                selected_daughterboard_id: Some("db-8in".to_string()),
                status: ConnectorSelectionStatus::Selected,
                source_profile_version: Some("1.0".to_string()),
            },
        );
        layout.connector_selections.insert(
            "0501010112345678".to_string(),
            NodeHardwareSelectionSet {
                carrier_key: "rr-cirkits::tower-lcc".to_string(),
                slot_selections,
                updated_at: Some("2026-05-02T10:30:00Z".to_string()),
            },
        );

        assert!(layout.validate().is_ok());

        let yaml = serde_yaml_ng::to_string(&layout).unwrap();
        let parsed: LayoutFile = serde_yaml_ng::from_str(&yaml).unwrap();

        assert!(parsed.validate().is_ok());
        assert_eq!(parsed.connector_selections.len(), 1);
        let node = parsed.connector_selections.get("0501010112345678").unwrap();
        assert_eq!(node.carrier_key, "rr-cirkits::tower-lcc");
        assert_eq!(node.slot_selections.len(), 1);
        assert_eq!(
            node.slot_selections.get("serial-a").unwrap().selected_daughterboard_id.as_deref(),
            Some("db-8in")
        );
    }

    #[test]
    fn connector_selection_preserves_unknown_daughterboard_records() {
        let mut layout = LayoutFile::default();
        let mut slot_selections = BTreeMap::new();
        slot_selections.insert(
            "serial-a".to_string(),
            ConnectorSelectionRecord {
                selected_daughterboard_id: Some("legacy-aux-card".to_string()),
                status: ConnectorSelectionStatus::Unknown,
                source_profile_version: Some("2026-04-30".to_string()),
            },
        );
        layout.connector_selections.insert(
            "0501010112345678".to_string(),
            NodeHardwareSelectionSet {
                carrier_key: "rr-cirkits::tower-lcc".to_string(),
                slot_selections,
                updated_at: Some("2026-05-02T10:30:00Z".to_string()),
            },
        );

        assert!(layout.validate().is_ok());

        let yaml = serde_yaml_ng::to_string(&layout).unwrap();
        let parsed: LayoutFile = serde_yaml_ng::from_str(&yaml).unwrap();

        assert!(parsed.validate().is_ok());
        let restored = parsed.connector_selections.get("0501010112345678").unwrap();
        let selection = restored.slot_selections.get("serial-a").unwrap();
        assert_eq!(selection.status, ConnectorSelectionStatus::Unknown);
        assert_eq!(selection.selected_daughterboard_id.as_deref(), Some("legacy-aux-card"));
        assert_eq!(selection.source_profile_version.as_deref(), Some("2026-04-30"));
    }

    // ── apply_layout_deltas tests (ADR-0002) ─────────────────────────────

    #[test]
    fn apply_deltas_create_bowtie() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Yard Entry".to_string()),
            },
        ]);
        assert_eq!(layout.bowties.len(), 1);
        let entry = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(entry.name.as_deref(), Some("Yard Entry"));
        assert!(entry.tags.is_empty());
    }

    #[test]
    fn apply_deltas_create_bowtie_noop_if_exists() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Original".to_string()),
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Overwrite Attempt".to_string()),
            },
        ]);
        // Should not overwrite
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().name.as_deref(), Some("Original"));
    }

    #[test]
    fn apply_deltas_delete_bowtie() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Doomed".to_string()),
            tags: vec![],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::DeleteBowtie { event_id_hex: "05.01.01.01.FF.00.00.01".to_string() },
        ]);
        assert!(layout.bowties.is_empty());
    }

    #[test]
    fn apply_deltas_rename_bowtie() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Old Name".to_string()),
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::RenameBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                new_name: "New Name".to_string(),
            },
        ]);
        let entry = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(entry.name.as_deref(), Some("New Name"));
        // Tags preserved
        assert_eq!(entry.tags, vec!["yard"]);
    }

    #[test]
    fn apply_deltas_rename_creates_if_absent() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::RenameBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                new_name: "Created via rename".to_string(),
            },
        ]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().name.as_deref(), Some("Created via rename"));
    }

    #[test]
    fn apply_deltas_add_and_remove_tag() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: None,
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::AddTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "signals".to_string(),
            },
            LayoutEditDelta::RemoveTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "yard".to_string(),
            },
        ]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().tags, vec!["signals"]);
    }

    #[test]
    fn apply_deltas_add_tag_deduplicates() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: None,
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::AddTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "yard".to_string(),
            },
        ]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().tags, vec!["yard"]);
    }

    #[test]
    fn apply_deltas_classify_role() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::ClassifyRole {
                key: "05.02.01.02.03.00:Port/Line/Event".to_string(),
                role: "Producer".to_string(),
            },
        ]);
        assert_eq!(layout.role_classifications.get("05.02.01.02.03.00:Port/Line/Event").unwrap().role, "Producer");
    }

    #[test]
    fn apply_deltas_set_connector_selection() {
        let mut layout = LayoutFile::default();
        let mut slot_selections = BTreeMap::new();
        slot_selections.insert("serial-a".to_string(), ConnectorSelectionRecord {
            selected_daughterboard_id: Some("BOD4-CP".to_string()),
            status: ConnectorSelectionStatus::Selected,
            source_profile_version: None,
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::SetConnectorSelection {
                node_id: "020157000001".to_string(),
                selection: NodeHardwareSelectionSet {
                    carrier_key: "rr-cirkits::tower-lcc".to_string(),
                    slot_selections,
                    updated_at: None,
                },
            },
        ]);
        let stored = layout.connector_selections.get("020157000001").unwrap();
        assert_eq!(stored.carrier_key, "rr-cirkits::tower-lcc");
    }

    #[test]
    fn apply_deltas_adopt_event_id() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("planning-123".to_string(), BowtieMetadata {
            name: Some("My Bowtie".to_string()),
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::AdoptEventId {
                old_event_id_hex: "planning-123".to_string(),
                new_event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
            },
        ]);
        assert!(!layout.bowties.contains_key("planning-123"));
        let moved = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(moved.name.as_deref(), Some("My Bowtie"));
        assert_eq!(moved.tags, vec!["yard"]);
    }

    #[test]
    fn apply_deltas_preserves_existing_roles_not_in_deltas() {
        let mut layout = LayoutFile::default();
        layout.role_classifications.insert(
            "existing:path".to_string(),
            RoleClassification { role: "Consumer".to_string() },
        );
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::ClassifyRole {
                key: "new:path".to_string(),
                role: "Producer".to_string(),
            },
        ]);
        // Existing role preserved
        assert_eq!(layout.role_classifications.get("existing:path").unwrap().role, "Consumer");
        // New role added
        assert_eq!(layout.role_classifications.get("new:path").unwrap().role, "Producer");
    }

    #[test]
    fn apply_deltas_multiple_operations_in_sequence() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Signal A".to_string()),
            },
            LayoutEditDelta::AddTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "yard".to_string(),
            },
            LayoutEditDelta::RenameBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                new_name: "Signal A (renamed)".to_string(),
            },
            LayoutEditDelta::ClassifyRole {
                key: "node:path".to_string(),
                role: "Producer".to_string(),
            },
        ]);
        let entry = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(entry.name.as_deref(), Some("Signal A (renamed)"));
        assert_eq!(entry.tags, vec!["yard"]);
        assert_eq!(layout.role_classifications.len(), 1);
    }

    #[test]
    fn apply_deltas_empty_vec_is_noop() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Untouched".to_string()),
            tags: vec![],
        });
        apply_layout_deltas(&mut layout, vec![]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().name.as_deref(), Some("Untouched"));
    }

    #[test]
    fn layout_edit_delta_json_roundtrip() {
        let deltas = vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Test".to_string()),
            },
            LayoutEditDelta::ClassifyRole {
                key: "node:path".to_string(),
                role: "Producer".to_string(),
            },
        ];
        let json = serde_json::to_string(&deltas).unwrap();
        let parsed: Vec<LayoutEditDelta> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
    }

    /// Verify that the exact JSON the frontend sends (camelCase, name omitted)
    /// deserializes correctly. This catches serde rename/default issues.
    #[test]
    fn layout_edit_delta_from_frontend_json() {
        // Frontend sends camelCase field names; name may be undefined (omitted)
        let json = r#"[
            {"type":"createBowtie","eventIdHex":"05.01.01.01.FF.00.00.01"},
            {"type":"createBowtie","eventIdHex":"05.01.01.01.FF.00.00.02","name":"Yard"},
            {"type":"createBowtie","eventIdHex":"05.01.01.01.FF.00.00.03","name":null},
            {"type":"deleteBowtie","eventIdHex":"05.01.01.01.FF.00.00.04"},
            {"type":"renameBowtie","eventIdHex":"05.01.01.01.FF.00.00.05","newName":"Signal B"},
            {"type":"addTag","eventIdHex":"05.01.01.01.FF.00.00.06","tag":"yard"},
            {"type":"removeTag","eventIdHex":"05.01.01.01.FF.00.00.07","tag":"yard"},
            {"type":"classifyRole","key":"node:path","role":"Producer"}
        ]"#;
        let parsed: Vec<LayoutEditDelta> = serde_json::from_str(json)
            .expect("frontend JSON should deserialize");
        assert_eq!(parsed.len(), 8);
        // name omitted → None
        match &parsed[0] {
            LayoutEditDelta::CreateBowtie { name, .. } => assert_eq!(*name, None),
            _ => panic!("expected CreateBowtie"),
        }
        // name present
        match &parsed[1] {
            LayoutEditDelta::CreateBowtie { name, .. } => assert_eq!(name.as_deref(), Some("Yard")),
            _ => panic!("expected CreateBowtie"),
        }
        // name: null → None
        match &parsed[2] {
            LayoutEditDelta::CreateBowtie { name, .. } => assert_eq!(*name, None),
            _ => panic!("expected CreateBowtie"),
        }
    }
}
