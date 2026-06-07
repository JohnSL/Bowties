//! Sync session classification — layout match scoring and sync row types.
//!
//! Pure functions for computing layout overlap, classifying sync rows,
//! and building sync row structs from offline changes.

use serde::{Deserialize, Serialize};

// ── Layout match types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutMatchThresholds {
    pub likely_same_min: u8,
    pub uncertain_min: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutMatchStatus {
    pub overlap_percent: f64,
    pub classification: String,
    pub expected_thresholds: LayoutMatchThresholds,
}

/// Compute the layout match status from discovered and layout node ID sets.
///
/// `layout_ids` are the canonical (uppercase, no-dots) node IDs persisted in
/// the layout. `discovered_ids` are the IDs currently seen on the bus.
pub fn compute_layout_match(
    layout_ids: &std::collections::HashSet<String>,
    discovered_ids: &std::collections::HashSet<String>,
) -> LayoutMatchStatus {
    if layout_ids.is_empty() {
        return LayoutMatchStatus {
            overlap_percent: 0.0,
            classification: "likely_different".to_string(),
            expected_thresholds: LayoutMatchThresholds {
                likely_same_min: 80,
                uncertain_min: 40,
            },
        };
    }

    let matched = layout_ids.intersection(discovered_ids).count();
    let overlap_percent = (matched as f64 / layout_ids.len() as f64) * 100.0;

    let classification = if overlap_percent >= 80.0 {
        "likely_same"
    } else if overlap_percent >= 40.0 {
        "uncertain"
    } else {
        "likely_different"
    };

    LayoutMatchStatus {
        overlap_percent,
        classification: classification.to_string(),
        expected_thresholds: LayoutMatchThresholds {
            likely_same_min: 80,
            uncertain_min: 40,
        },
    }
}

// ── Sync row types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRow {
    pub change_id: String,
    pub node_id: Option<String>,
    pub node_name: Option<String>,
    pub field_label: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub bus_value: Option<String>,
    pub resolution: String,
    pub error: Option<String>,
}

/// Build a `SyncRow` from an offline change and resolved context.
pub fn build_sync_row(
    change_id: &str,
    node_key: Option<&str>,
    node_name: Option<String>,
    field_label: Option<String>,
    baseline_value: &str,
    planned_value: &str,
    bus_value: Option<String>,
    error: Option<String>,
) -> SyncRow {
    SyncRow {
        change_id: change_id.to_string(),
        node_id: node_key.map(|s| s.to_string()),
        node_name,
        field_label,
        baseline_value: baseline_value.to_string(),
        planned_value: planned_value.to_string(),
        bus_value,
        resolution: "unresolved".to_string(),
        error,
    }
}

/// Classify a sync comparison into one of: already_applied, clean, or conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncClassification {
    /// Bus value already equals the planned value — change was applied externally.
    AlreadyApplied,
    /// Bus value matches baseline — safe to apply the planned change.
    Clean,
    /// Bus value differs from both baseline and planned — manual resolution needed.
    Conflict,
}

/// Classify a single field comparison given the three values.
pub fn classify_sync_row(
    bus_value: &str,
    baseline_value: &str,
    planned_value: &str,
) -> SyncClassification {
    if bus_value == planned_value {
        SyncClassification::AlreadyApplied
    } else if bus_value == baseline_value {
        SyncClassification::Clean
    } else {
        SyncClassification::Conflict
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSession {
    pub conflict_rows: Vec<SyncRow>,
    pub clean_rows: Vec<SyncRow>,
    pub already_applied_count: usize,
    pub node_missing_rows: Vec<SyncRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySyncFailure {
    pub change_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySyncResult {
    pub applied: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<ApplySyncFailure>,
    pub read_only_cleared: Vec<String>,
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn layout_match_empty_layout_is_likely_different() {
        let layout_ids = HashSet::new();
        let discovered = HashSet::from(["AABBCCDDEE00".to_string()]);
        let result = compute_layout_match(&layout_ids, &discovered);
        assert_eq!(result.classification, "likely_different");
        assert_eq!(result.overlap_percent, 0.0);
    }

    #[test]
    fn layout_match_full_overlap_is_likely_same() {
        let ids: HashSet<String> = HashSet::from([
            "AABBCCDDEE00".to_string(),
            "AABBCCDDEE01".to_string(),
        ]);
        let result = compute_layout_match(&ids, &ids);
        assert_eq!(result.classification, "likely_same");
        assert_eq!(result.overlap_percent, 100.0);
    }

    #[test]
    fn layout_match_partial_overlap_is_uncertain() {
        let layout = HashSet::from([
            "AABBCCDDEE00".to_string(),
            "AABBCCDDEE01".to_string(),
        ]);
        let discovered = HashSet::from(["AABBCCDDEE00".to_string()]);
        let result = compute_layout_match(&layout, &discovered);
        assert_eq!(result.classification, "uncertain");
        assert_eq!(result.overlap_percent, 50.0);
    }

    #[test]
    fn classify_already_applied() {
        assert_eq!(
            classify_sync_row("20", "10", "20"),
            SyncClassification::AlreadyApplied
        );
    }

    #[test]
    fn classify_clean() {
        assert_eq!(
            classify_sync_row("10", "10", "20"),
            SyncClassification::Clean
        );
    }

    #[test]
    fn classify_conflict() {
        assert_eq!(
            classify_sync_row("15", "10", "20"),
            SyncClassification::Conflict
        );
    }

    #[test]
    fn build_sync_row_populates_fields() {
        let row = build_sync_row(
            "change-1",
            Some("050201020300"),
            Some("My Node".to_string()),
            Some("Config.Field".to_string()),
            "10",
            "20",
            Some("10".to_string()),
            None,
        );
        assert_eq!(row.change_id, "change-1");
        assert_eq!(row.node_id.as_deref(), Some("050201020300"));
        assert_eq!(row.resolution, "unresolved");
    }
}
