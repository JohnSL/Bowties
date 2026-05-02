//! Tauri IPC scaffolding for connector daughterboard operations.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub allowed_values: Vec<ConnectorScalarValueView>,
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
pub struct ConnectorSelectionDocument {
    pub node_id: String,
    pub carrier_key: String,
    pub slot_selections: Vec<ConnectorSelection>,
    pub updated_at: Option<String>,
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
            allowed_values: value.allowed_values.into_iter().map(ConnectorScalarValueView::from).collect(),
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

fn canonical_node_key(node_id: &str) -> Result<String, String> {
    lcc_rs::NodeID::from_hex_string(node_id)
        .map(|parsed| parsed.to_canonical())
        .map_err(|e| format!("InvalidNodeId: {}", e))
}

fn selection_status_to_layout(
    status: ConnectorSelectionStatus,
) -> crate::layout::types::ConnectorSelectionStatus {
    match status {
        ConnectorSelectionStatus::Selected => crate::layout::types::ConnectorSelectionStatus::Selected,
        ConnectorSelectionStatus::None => crate::layout::types::ConnectorSelectionStatus::None,
        ConnectorSelectionStatus::Unknown => crate::layout::types::ConnectorSelectionStatus::Unknown,
    }
}

fn selection_status_from_layout(
    status: crate::layout::types::ConnectorSelectionStatus,
) -> ConnectorSelectionStatus {
    match status {
        crate::layout::types::ConnectorSelectionStatus::Selected => ConnectorSelectionStatus::Selected,
        crate::layout::types::ConnectorSelectionStatus::None => ConnectorSelectionStatus::None,
        crate::layout::types::ConnectorSelectionStatus::Unknown => ConnectorSelectionStatus::Unknown,
    }
}

fn selection_document_from_layout(
    layout: &crate::layout::types::LayoutFile,
    node_id: &str,
) -> Result<Option<ConnectorSelectionDocument>, String> {
    let node_key = canonical_node_key(node_id)?;
    let Some(saved) = layout.connector_selections.get(&node_key) else {
        return Ok(None);
    };

    let mut slot_selections: Vec<ConnectorSelection> = saved
        .slot_selections
        .iter()
        .map(|(slot_id, selection)| ConnectorSelection {
            slot_id: slot_id.clone(),
            selected_daughterboard_id: selection.selected_daughterboard_id.clone(),
            status: selection_status_from_layout(selection.status),
        })
        .collect();
    slot_selections.sort_by(|left, right| left.slot_id.cmp(&right.slot_id));

    Ok(Some(ConnectorSelectionDocument {
        node_id: node_id.to_string(),
        carrier_key: saved.carrier_key.clone(),
        slot_selections,
        updated_at: saved.updated_at.clone(),
    }))
}

fn upsert_selection_document(
    layout: &mut crate::layout::types::LayoutFile,
    document: &ConnectorSelectionDocument,
) -> Result<(), String> {
    let node_key = canonical_node_key(&document.node_id)?;

    let mut slot_selections = std::collections::BTreeMap::new();
    for selection in &document.slot_selections {
        slot_selections.insert(
            selection.slot_id.clone(),
            crate::layout::types::ConnectorSelectionRecord {
                selected_daughterboard_id: selection.selected_daughterboard_id.clone(),
                status: selection_status_to_layout(selection.status.clone()),
                source_profile_version: None,
            },
        );
    }

    layout.connector_selections.insert(
        node_key,
        crate::layout::types::NodeHardwareSelectionSet {
            carrier_key: document.carrier_key.clone(),
            slot_selections,
            updated_at: document.updated_at.clone().or_else(|| Some(chrono::Utc::now().to_rfc3339())),
        },
    );

    layout.validate()
}

fn default_document_from_profile(profile: &ConnectorProfileView) -> ConnectorSelectionDocument {
    ConnectorSelectionDocument {
        node_id: profile.node_id.clone(),
        carrier_key: profile.carrier_key.clone(),
        slot_selections: profile
            .slots
            .iter()
            .map(|slot| ConnectorSelection {
                slot_id: slot.slot_id.clone(),
                selected_daughterboard_id: None,
                status: ConnectorSelectionStatus::None,
            })
            .collect(),
        updated_at: None,
    }
}

async fn active_layout_path(state: &tauri::State<'_, AppState>) -> Result<PathBuf, String> {
    let active_layout = state.active_layout.read().await.clone();
    let context = active_layout
        .filter(|context| context.mode == crate::state::ActiveLayoutMode::OfflineFile)
        .ok_or_else(|| "No offline layout is active".to_string())?;
    Ok(PathBuf::from(context.root_path))
}

async fn load_active_layout_metadata(
    state: &tauri::State<'_, AppState>,
) -> Result<(PathBuf, crate::layout::io::LayoutDirectoryReadData), String> {
    let path = active_layout_path(state).await?;
    let loaded = crate::layout::io::read_layout_capture(&path)?;
    Ok((path, loaded))
}

fn persist_layout_metadata(
    path: &std::path::Path,
    loaded: crate::layout::io::LayoutDirectoryReadData,
    layout: crate::layout::types::LayoutFile,
) -> Result<(), String> {
    let write_data = crate::layout::io::LayoutDirectoryWriteData {
        manifest: loaded.manifest,
        node_snapshots: loaded.node_snapshots,
        bowties: layout,
        offline_changes: loaded.offline_changes,
        cdi_files: vec![],
    };

    crate::layout::io::write_layout_capture(path, &write_data)
}

#[tauri::command]
pub async fn get_connector_profile(
    node_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<ConnectorProfileView>, String> {
    let tree = crate::commands::cdi::get_node_tree(state, app_handle, node_id).await?;
    Ok(tree.connector_profile.map(ConnectorProfileView::from))
}

#[tauri::command]
pub async fn get_connector_selections(
    node_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<ConnectorSelectionDocument>, String> {
    let (_, loaded) = load_active_layout_metadata(&state).await?;
    if let Some(document) = selection_document_from_layout(&loaded.bowties, &node_id)? {
        return Ok(Some(document));
    }

    let tree = crate::commands::cdi::get_node_tree(state, app_handle, node_id).await?;
    Ok(tree
        .connector_profile
        .map(ConnectorProfileView::from)
        .map(|profile| default_document_from_profile(&profile)))
}

#[tauri::command]
pub async fn put_connector_selections(
    document: ConnectorSelectionDocument,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectorSelectionDocument, String> {
    let (path, loaded) = load_active_layout_metadata(&state).await?;
    let mut layout = loaded.bowties.clone();
    upsert_selection_document(&mut layout, &document)?;
    persist_layout_metadata(&path, loaded, layout.clone())?;

    selection_document_from_layout(&layout, &document.node_id)?
        .ok_or_else(|| "Connector selection persistence did not produce a stored document".to_string())
}

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

    fn sample_layout() -> crate::layout::types::LayoutFile {
        crate::layout::types::LayoutFile::default()
    }

    #[test]
    fn connector_profile_view_defaults_none_selection_document() {
        let document = default_document_from_profile(&ConnectorProfileView {
            node_id: "05.02.01.02.03.00".to_string(),
            carrier_key: "rr-cirkits::tower-lcc".to_string(),
            slots: vec![
                ConnectorSlotView {
                    slot_id: "connector-a".to_string(),
                    label: "Connector A".to_string(),
                    order: 0,
                    allow_none_installed: true,
                    supported_daughterboard_ids: vec!["BOD4-CP".to_string()],
                    affected_paths: vec!["Port I/O/Line".to_string()],
                    resolved_affected_paths: vec![],
                    base_behavior_when_empty: None,
                    supported_daughterboard_constraints: vec![],
                },
                ConnectorSlotView {
                    slot_id: "connector-b".to_string(),
                    label: "Connector B".to_string(),
                    order: 1,
                    allow_none_installed: true,
                    supported_daughterboard_ids: vec!["FOB-A".to_string()],
                    affected_paths: vec!["Port I/O/Line".to_string()],
                    resolved_affected_paths: vec![],
                    base_behavior_when_empty: None,
                    supported_daughterboard_constraints: vec![],
                },
            ],
            supported_daughterboards: vec![],
        });

        assert_eq!(document.slot_selections.len(), 2);
        assert!(document.slot_selections.iter().all(|selection| selection.status == ConnectorSelectionStatus::None));
    }

    #[test]
    fn connector_selection_document_roundtrips_through_layout_metadata() {
        let mut layout = sample_layout();
        let document = ConnectorSelectionDocument {
            node_id: "05.02.01.02.03.00".to_string(),
            carrier_key: "rr-cirkits::tower-lcc".to_string(),
            slot_selections: vec![
                ConnectorSelection {
                    slot_id: "connector-a".to_string(),
                    selected_daughterboard_id: Some("BOD4-CP".to_string()),
                    status: ConnectorSelectionStatus::Selected,
                },
                ConnectorSelection {
                    slot_id: "connector-b".to_string(),
                    selected_daughterboard_id: None,
                    status: ConnectorSelectionStatus::None,
                },
            ],
            updated_at: Some("2026-05-02T12:00:00Z".to_string()),
        };

        upsert_selection_document(&mut layout, &document).expect("document should persist in layout metadata");
        let restored = selection_document_from_layout(&layout, &document.node_id)
            .expect("layout lookup should succeed")
            .expect("document should be present after persist");

        assert_eq!(restored.carrier_key, document.carrier_key);
        assert_eq!(restored.slot_selections.len(), 2);
        assert_eq!(restored.slot_selections[0].slot_id, "connector-a");
        assert_eq!(restored.slot_selections[0].selected_daughterboard_id.as_deref(), Some("BOD4-CP"));
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
            }],
        });

        assert_eq!(profile.node_id, "05.02.01.02.03.00");
        assert_eq!(profile.slots.len(), 1);
        assert_eq!(profile.supported_daughterboards.len(), 1);
        assert_eq!(profile.supported_daughterboards[0].daughterboard_id, "BOD4-CP");
    }
}