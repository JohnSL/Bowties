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
}

impl Default for LayoutFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            bowties: BTreeMap::new(),
            role_classifications: BTreeMap::new(),
        }
    }
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

        // Validate bowtie keys match event ID hex format
        for key in self.bowties.keys() {
            if !is_valid_event_id_hex(key) {
                return Err(format!(
                    "Invalid bowtie key '{}': must be dotted hex (e.g. 05.01.01.01.FF.00.00.01)",
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
}
