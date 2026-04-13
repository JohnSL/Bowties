//! Layout directory manifest types.

use serde::{Deserialize, Serialize};

pub const LAYOUT_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchThresholds {
    pub likely_same: u8,
    pub uncertain_min: u8,
}

impl Default for MatchThresholds {
    fn default() -> Self {
        Self {
            likely_same: 80,
            uncertain_min: 40,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFiles {
    pub bowties: String,
    pub event_names: String,
    pub offline_changes: String,
    pub nodes_dir: String,
}

impl Default for ManifestFiles {
    fn default() -> Self {
        Self {
            bowties: "bowties.yaml".to_string(),
            event_names: "event-names.yaml".to_string(),
            offline_changes: "offline-changes.yaml".to_string(),
            nodes_dir: "nodes".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutManifest {
    pub schema_version: u32,
    pub layout_id: String,
    pub captured_at: String,
    pub last_saved_at: String,
    pub active_mode: String,
    pub match_thresholds: MatchThresholds,
    #[serde(default)]
    pub companion_dir: String,
    pub files: ManifestFiles,
}

impl LayoutManifest {
    pub fn new(
        layout_id: String,
        captured_at: String,
        last_saved_at: String,
        companion_dir: String,
    ) -> Self {
        Self {
            schema_version: LAYOUT_SCHEMA_VERSION,
            layout_id,
            captured_at,
            last_saved_at,
            active_mode: "offline".to_string(),
            match_thresholds: MatchThresholds::default(),
            companion_dir,
            files: ManifestFiles::default(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != LAYOUT_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported layout schema version {} (expected {})",
                self.schema_version,
                LAYOUT_SCHEMA_VERSION
            ));
        }
        if self.layout_id.trim().is_empty() {
            return Err("layoutId must not be empty".to_string());
        }
        if self.schema_version == LAYOUT_SCHEMA_VERSION && self.companion_dir.trim().is_empty() {
            return Err("companionDir must not be empty for schema v3".to_string());
        }
        if self.files.nodes_dir.trim().is_empty() {
            return Err("files.nodesDir must not be empty".to_string());
        }
        Ok(())
    }
}
