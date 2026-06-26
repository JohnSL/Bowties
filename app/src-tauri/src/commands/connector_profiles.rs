//! Tauri IPC scaffolding for connector daughterboard operations.

use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSlotView {
    pub slot_id: String,
    pub label: String,
    pub order: u32,
    pub allow_none_installed: bool,
    pub supported_daughterboard_ids: Vec<String>,
    pub affected_paths: Vec<String>,
    pub resolved_affected_paths: Vec<Vec<String>>,
    pub base_behavior_when_empty: Option<EmptyConnectorBehaviorView>,
    pub supported_daughterboard_constraints: Vec<SlotSupportedDaughterboardView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConnectorScalarValueView {
    String(String),
    Integer(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyConnectorBehaviorView {
    pub effect: FilterEffect,
    pub allowed_values: Vec<ConnectorScalarValueView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConstraintView {
    pub target_path: String,
    pub resolved_path: Vec<String>,
    pub effect: FilterEffect,
    pub line_ordinals: Vec<u32>,
    pub replication_ordinals: Vec<u32>,
    pub allowed_values: Vec<ConnectorScalarValueView>,
    pub allowed_value_labels: Vec<String>,
    pub denied_values: Vec<ConnectorScalarValueView>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotSupportedDaughterboardView {
    pub daughterboard_id: String,
    pub validity_rules: Vec<ConnectorConstraintView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaughterboardView {
    pub daughterboard_id: String,
    pub display_name: String,
    pub kind: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorProfileView {
    pub node_id: String,
    pub carrier_key: String,
    pub slots: Vec<ConnectorSlotView>,
    pub supported_daughterboards: Vec<DaughterboardView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSelectionStatus {
    Selected,
    None,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSelection {
    pub slot_id: String,
    pub selected_daughterboard_id: Option<String>,
    pub status: ConnectorSelectionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityPreviewRequest {
    pub node_id: String,
    pub changed_slot_id: String,
    pub slot_selections: Vec<ConnectorSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterEffect {
    Show,
    Hide,
    Disable,
    AllowValues,
    DenyValues,
    ReadOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilteredTarget {
    pub target_path: String,
    pub effect: FilterEffect,
    pub allowed_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedRepair {
    pub target_path: String,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub reason: String,
    pub origin_slot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityPreviewResponse {
    pub node_id: String,
    pub changed_slot_id: String,
    pub filtered_targets: Vec<FilteredTarget>,
    pub staged_repairs: Vec<StagedRepair>,
    pub warnings: Vec<String>,
}

impl From<crate::node_tree::ConnectorSlot> for ConnectorSlotView {
    fn from(value: crate::node_tree::ConnectorSlot) -> Self {
        Self {
            slot_id: value.slot_id,
            label: value.label,
            order: value.order,
            allow_none_installed: value.allow_none_installed,
            supported_daughterboard_ids: value.supported_daughterboard_ids,
            affected_paths: value.affected_paths,
            resolved_affected_paths: value.resolved_affected_paths,
            base_behavior_when_empty: value.base_behavior_when_empty.map(EmptyConnectorBehaviorView::from),
            supported_daughterboard_constraints: value
                .supported_daughterboard_constraints
                .into_iter()
                .map(SlotSupportedDaughterboardView::from)
                .collect(),
        }
    }
}

impl From<crate::node_tree::ConnectorScalarValue> for ConnectorScalarValueView {
    fn from(value: crate::node_tree::ConnectorScalarValue) -> Self {
        match value {
            crate::node_tree::ConnectorScalarValue::String(value) => Self::String(value),
            crate::node_tree::ConnectorScalarValue::Integer(value) => Self::Integer(value),
        }
    }
}

impl From<crate::node_tree::EmptyConnectorBehavior> for EmptyConnectorBehaviorView {
    fn from(value: crate::node_tree::EmptyConnectorBehavior) -> Self {
        Self {
            effect: match value.effect {
                crate::node_tree::EmptyConnectorConstraintEffect::Hide => FilterEffect::Hide,
                crate::node_tree::EmptyConnectorConstraintEffect::Disable => FilterEffect::Disable,
                crate::node_tree::EmptyConnectorConstraintEffect::AllowValues => FilterEffect::AllowValues,
            },
            allowed_values: value.allowed_values.into_iter().map(ConnectorScalarValueView::from).collect(),
        }
    }
}

impl From<crate::node_tree::ConnectorConstraint> for ConnectorConstraintView {
    fn from(value: crate::node_tree::ConnectorConstraint) -> Self {
        Self {
            target_path: value.target_path,
            resolved_path: value.resolved_path,
            effect: match value.effect {
                crate::node_tree::ConnectorConstraintEffect::Show => FilterEffect::Show,
                crate::node_tree::ConnectorConstraintEffect::Hide => FilterEffect::Hide,
                crate::node_tree::ConnectorConstraintEffect::Disable => FilterEffect::Disable,
                crate::node_tree::ConnectorConstraintEffect::AllowValues => FilterEffect::AllowValues,
                crate::node_tree::ConnectorConstraintEffect::DenyValues => FilterEffect::DenyValues,
                crate::node_tree::ConnectorConstraintEffect::ReadOnly => FilterEffect::ReadOnly,
            },
            line_ordinals: value.line_ordinals,
            replication_ordinals: value.replication_ordinals,
            allowed_values: value.allowed_values.into_iter().map(ConnectorScalarValueView::from).collect(),
            allowed_value_labels: value.allowed_value_labels,
            denied_values: value.denied_values.into_iter().map(ConnectorScalarValueView::from).collect(),
            explanation: value.explanation,
        }
    }
}

impl From<crate::node_tree::SlotSupportedDaughterboard> for SlotSupportedDaughterboardView {
    fn from(value: crate::node_tree::SlotSupportedDaughterboard) -> Self {
        Self {
            daughterboard_id: value.daughterboard_id,
            validity_rules: value.validity_rules.into_iter().map(ConnectorConstraintView::from).collect(),
        }
    }
}

impl From<crate::node_tree::SupportedDaughterboard> for DaughterboardView {
    fn from(value: crate::node_tree::SupportedDaughterboard) -> Self {
        Self {
            daughterboard_id: value.daughterboard_id,
            display_name: value.display_name,
            kind: value.kind,
            description: value.description,
        }
    }
}

impl From<crate::node_tree::ConnectorProfile> for ConnectorProfileView {
    fn from(value: crate::node_tree::ConnectorProfile) -> Self {
        Self {
            node_id: value.node_id,
            carrier_key: value.carrier_key,
            slots: value.slots.into_iter().map(ConnectorSlotView::from).collect(),
            supported_daughterboards: value
                .supported_daughterboards
                .into_iter()
                .map(DaughterboardView::from)
                .collect(),
        }
    }
}

// NOTE: Connector-selection persistence was removed in Spec 014. The
// replacement seam is `node_mode_selections` + `placeholder_boards`; the
// frontend `connectorSelections` store / orchestrator are re-targeted in
// later slices (S6). The selection-persistence helpers
// (`canonical_node_key`, `active_layout_path`,
// `load_active_layout_metadata`, `persist_layout_metadata`) were removed
// along with the commands they supported.

#[tauri::command]
pub async fn get_connector_profile(
    node_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<ConnectorProfileView>, String> {
    let tree = crate::commands::cdi::get_node_tree(state, app_handle, node_id).await?;
    Ok(tree.connector_profile.map(ConnectorProfileView::from))
}

// NOTE: `get_connector_selections` / `put_connector_selections` were removed
// in Spec 014. The unified replacement is `node_mode_selections` mutated via
// the `SetNodeModeSelection` `LayoutEditDelta`; commands land in S4+.

#[tauri::command]
pub async fn preview_connector_compatibility(
    _request: CompatibilityPreviewRequest,
    _state: tauri::State<'_, AppState>,
) -> Result<CompatibilityPreviewResponse, String> {
    Err("Connector compatibility preview is not implemented yet".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_profile_view_default_selection_document_removed_in_v2() {
        // `default_document_from_profile` was removed in Spec 014 along with
        // `get_connector_selections` / `put_connector_selections`. Default
        // mode selections are computed at annotation time from the profile's
        // `configurationModes`, not synthesised by the IPC command surface.
    }

    #[test]
    fn connector_selection_document_roundtrip_removed_in_v2() {
        // Connector-selection persistence was removed in Spec 014. The
        // unified replacement (`node_mode_selections`) is covered by
        // `layout::types::tests::s3_*`.
    }

    #[test]
    fn connector_profile_view_converts_from_node_tree_payload() {
        let profile = ConnectorProfileView::from(crate::node_tree::ConnectorProfile {
            node_id: "05.02.01.02.03.00".to_string(),
            carrier_key: "rr-cirkits::tower-lcc".to_string(),
            slots: vec![crate::node_tree::ConnectorSlot {
                slot_id: "connector-a".to_string(),
                label: "Connector A".to_string(),
                order: 0,
                allow_none_installed: true,
                supported_daughterboard_ids: vec!["BOD4-CP".to_string()],
                affected_paths: vec!["Port I/O/Line".to_string()],
                resolved_affected_paths: vec![vec!["seg:2".to_string(), "elem:0#1".to_string()]],
                base_behavior_when_empty: None,
                supported_daughterboard_constraints: vec![],
            }],
            supported_daughterboards: vec![crate::node_tree::SupportedDaughterboard {
                daughterboard_id: "BOD4-CP".to_string(),
                display_name: "BOD4-CP".to_string(),
                kind: Some("detector".to_string()),
                description: Some("Tower-compatible input board".to_string()),
                channel_inputs: vec![],
            }],
        });

        assert_eq!(profile.node_id, "05.02.01.02.03.00");
        assert_eq!(profile.slots.len(), 1);
        assert_eq!(profile.supported_daughterboards.len(), 1);
        assert_eq!(profile.supported_daughterboards[0].daughterboard_id, "BOD4-CP");
    }
}