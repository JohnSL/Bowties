//! Layout directory manifest types.

use serde::{Deserialize, Serialize};

use super::types::ConnectionConfig;

pub const LAYOUT_SCHEMA_VERSION: u32 = 4;

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
pub struct LayoutManifest {
    pub schema_version: u32,
    pub layout_id: String,
    pub captured_at: String,
    pub last_saved_at: String,
    pub active_mode: String,
    pub match_thresholds: MatchThresholds,
    /// Saved connection profiles attached to this layout (Spec 013 / S4).
    /// Serde-defaulted so older layout files without this field open
    /// cleanly with an empty list — no schema bump required.
    #[serde(default)]
    pub connections: Vec<ConnectionConfig>,
}

impl LayoutManifest {
    pub fn new(
        layout_id: String,
        captured_at: String,
        last_saved_at: String,
    ) -> Self {
        Self {
            schema_version: LAYOUT_SCHEMA_VERSION,
            layout_id,
            captured_at,
            last_saved_at,
            active_mode: "offline".to_string(),
            match_thresholds: MatchThresholds::default(),
            connections: Vec::new(),
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
        Ok(())
    }
}

/// Build the `LayoutManifest` for a save, preserving manifest fields the
/// save command does not intentionally rewrite.
///
/// Save-owned fields (always assigned from arguments): `schema_version`,
/// `layout_id`, `captured_at`, `last_saved_at`.
///
/// Preserved from `previous` when present (defaults otherwise):
/// `connections`, `match_thresholds`, `active_mode`.
pub fn build_save_manifest(
    previous: Option<&LayoutManifest>,
    layout_id: String,
    captured_at: String,
    last_saved_at: String,
) -> LayoutManifest {
    LayoutManifest {
        schema_version: LAYOUT_SCHEMA_VERSION,
        layout_id,
        captured_at,
        last_saved_at,
        active_mode: previous
            .map(|m| m.active_mode.clone())
            .unwrap_or_else(|| "offline".to_string()),
        match_thresholds: previous
            .map(|m| m.match_thresholds.clone())
            .unwrap_or_default(),
        connections: previous
            .map(|m| m.connections.clone())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::types::{AdapterType, ConnectionConfig, FlowControl};

    fn sample_connection() -> ConnectionConfig {
        ConnectionConfig {
            id: "conn-1".to_string(),
            name: "Home TCP".to_string(),
            adapter_type: AdapterType::Tcp,
            host: Some("localhost".to_string()),
            port: Some(12021),
            serial_port: None,
            baud_rate: None,
            flow_control: FlowControl::None,
        }
    }

    #[test]
    fn build_save_manifest_without_previous_uses_defaults() {
        let m = build_save_manifest(
            None,
            "layout-a".to_string(),
            "2026-06-01T00:00:00Z".to_string(),
            "2026-06-01T00:00:01Z".to_string(),
        );
        assert_eq!(m.layout_id, "layout-a");
        assert_eq!(m.active_mode, "offline");
        assert!(m.connections.is_empty());
        assert_eq!(m.match_thresholds.likely_same, 80);
    }

    #[test]
    fn build_save_manifest_preserves_connections_from_previous() {
        // Regression: saving a layout that already has a saved connection
        // must not drop it. Previously `LayoutManifest::new(...)` was used
        // unconditionally, zeroing the connections list on every save.
        let previous = LayoutManifest {
            schema_version: LAYOUT_SCHEMA_VERSION,
            layout_id: "layout-a".to_string(),
            captured_at: "2026-06-01T00:00:00Z".to_string(),
            last_saved_at: "2026-06-01T00:00:00Z".to_string(),
            active_mode: "online".to_string(),
            match_thresholds: MatchThresholds { likely_same: 90, uncertain_min: 50 },
            connections: vec![sample_connection()],
        };
        let m = build_save_manifest(
            Some(&previous),
            previous.layout_id.clone(),
            previous.captured_at.clone(),
            "2026-06-01T00:01:00Z".to_string(),
        );
        assert_eq!(m.connections, vec![sample_connection()]);
        assert_eq!(m.active_mode, "online");
        assert_eq!(m.match_thresholds.likely_same, 90);
        assert_eq!(m.match_thresholds.uncertain_min, 50);
        assert_eq!(m.last_saved_at, "2026-06-01T00:01:00Z");
    }
}
