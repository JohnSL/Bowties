//! Offline sync panel command skeletons.
//!
//! Phase 1 scaffolding only. Implementations are added in later phases.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::layout::offline_changes::{OfflineChange, OfflineChangeKind, OfflineChangeStatus};
use crate::state::{AppState, ActiveLayoutMode};

fn same_change_target(a: &OfflineChange, b: &OfflineChange) -> bool {
    a.kind == b.kind
        && a.node_id == b.node_id
        && a.space == b.space
        && a.offset == b.offset
        && a.baseline_value == b.baseline_value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineChangeInput {
    pub kind: String,
    pub node_id: Option<String>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineChangeRow {
    pub change_id: String,
    pub kind: String,
    pub node_id: Option<String>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub status: String,
    pub error: Option<String>,
    pub updated_at: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRow {
    pub change_id: String,
    pub node_id: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub bus_value: Option<String>,
    pub resolution: String,
    pub error: Option<String>,
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
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    TargetLayoutBus,
    BenchOtherBus,
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

#[tauri::command]
pub async fn set_offline_change(
    change: OfflineChangeInput,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Verify active offline layout
    {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?;
    }

    // Generate unique change ID (UUID + timestamp)
    let change_id = format!(
        "{}-{}",
        Uuid::new_v4(),
        chrono::Utc::now().timestamp_millis()
    );

    // Create the offline change record
    let offline_change = OfflineChange {
        change_id: change_id.clone(),
        kind: match change.kind.as_str() {
            "config" => OfflineChangeKind::Config,
            "bowtieMetadata" => OfflineChangeKind::BowtieMetadata,
            "bowtieEvent" => OfflineChangeKind::BowtieEvent,
            _ => return Err(format!("Invalid change kind: {}", change.kind)),
        },
        node_id: change.node_id,
        space: change.space,
        offset: change.offset,
        baseline_value: change.baseline_value,
        planned_value: change.planned_value,
        status: OfflineChangeStatus::Pending,
        error: None,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    // Validate the change
    offline_change.validate()?;

    // In-memory only pre-save path: upsert on same target.
    let mut change_id_out = change_id;
    {
        let mut cache = state.offline_changes_cache.write().await;
        if let Some(existing) = cache.iter_mut().find(|c| same_change_target(c, &offline_change)) {
            existing.planned_value = offline_change.planned_value;
            existing.updated_at = offline_change.updated_at;
            existing.status = OfflineChangeStatus::Pending;
            existing.error = None;
            change_id_out = existing.change_id.clone();
        } else {
            cache.push(offline_change);
        }
    }

    // Update pending count in active layout context
    {
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_offline_change_count = state.offline_changes_cache.read().await.len();
        }
    }

    Ok(change_id_out)
}

#[tauri::command]
pub async fn revert_offline_change(
    change_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    // Verify active offline layout
    {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?;
    }

    // Remove from in-memory cache
    let removed = {
        let mut cache = state.offline_changes_cache.write().await;
        let initial_len = cache.len();
        cache.retain(|c| c.change_id != change_id);
        cache.len() < initial_len
    };

    if !removed {
        return Err(format!("Change not found: {}", change_id));
    }

    // Update pending count in active layout context
    {
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_offline_change_count = state.offline_changes_cache.read().await.len();
        }
    }

    Ok(true)
}

#[tauri::command]
pub async fn list_offline_changes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<OfflineChangeRow>, String> {
    // Verify active offline layout
    let _context = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?
    };

    let changes = state.offline_changes_cache.read().await;
    let rows = changes
        .iter()
        .map(|c| OfflineChangeRow {
            change_id: c.change_id.clone(),
            kind: format!("{:?}", c.kind).to_lowercase(),
            node_id: c.node_id.clone(),
            space: c.space,
            offset: c.offset.clone(),
            baseline_value: c.baseline_value.clone(),
            planned_value: c.planned_value.clone(),
            status: format!("{:?}", c.status).to_lowercase(),
            error: c.error.clone(),
            updated_at: c.updated_at.clone(),
        })
        .collect();

    Ok(rows)
}

#[tauri::command]
pub async fn replace_offline_changes(
    changes: Vec<OfflineChangeInput>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    // Verify active offline layout
    {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut rows = Vec::with_capacity(changes.len());
    for change in changes {
        let kind = match change.kind.as_str() {
            "config" => OfflineChangeKind::Config,
            "bowtieMetadata" => OfflineChangeKind::BowtieMetadata,
            "bowtieEvent" => OfflineChangeKind::BowtieEvent,
            _ => return Err(format!("Invalid change kind: {}", change.kind)),
        };

        let row = OfflineChange {
            change_id: format!("{}-{}", Uuid::new_v4(), chrono::Utc::now().timestamp_millis()),
            kind,
            node_id: change.node_id,
            space: change.space,
            offset: change.offset,
            baseline_value: change.baseline_value,
            planned_value: change.planned_value,
            status: OfflineChangeStatus::Pending,
            error: None,
            updated_at: now.clone(),
        };
        row.validate()?;
        rows.push(row);
    }

    let count = rows.len();
    *state.offline_changes_cache.write().await = rows;

    let mut guard = state.active_layout.write().await;
    if let Some(ctx) = &mut *guard {
        ctx.pending_offline_change_count = count;
    }

    Ok(count)
}

#[tauri::command]
pub async fn compute_layout_match_status(discovered_node_ids: Vec<String>) -> Result<LayoutMatchStatus, String> {
    let _ = discovered_node_ids;
    Err("compute_layout_match_status is not implemented yet".to_string())
}

#[tauri::command]
pub async fn build_sync_session() -> Result<SyncSession, String> {
    Err("build_sync_session is not implemented yet".to_string())
}

#[tauri::command]
pub async fn set_sync_mode(mode: SyncMode) -> Result<String, String> {
    let _ = mode;
    Err("set_sync_mode is not implemented yet".to_string())
}

#[tauri::command]
pub async fn apply_sync_changes(apply_change_ids: Vec<String>, skip_change_ids: Vec<String>) -> Result<ApplySyncResult, String> {
    let _ = (apply_change_ids, skip_change_ids);
    Err("apply_sync_changes is not implemented yet".to_string())
}
