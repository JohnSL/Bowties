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
}
