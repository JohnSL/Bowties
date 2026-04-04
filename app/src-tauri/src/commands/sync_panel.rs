//! Offline sync panel command skeletons.
//!
//! Phase 1 scaffolding only. Implementations are added in later phases.

use serde::{Deserialize, Serialize};

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
pub async fn set_offline_change(change: OfflineChangeInput) -> Result<String, String> {
    let _ = change;
    Err("set_offline_change is not implemented yet".to_string())
}

#[tauri::command]
pub async fn revert_offline_change(change_id: String) -> Result<bool, String> {
    let _ = change_id;
    Err("revert_offline_change is not implemented yet".to_string())
}

#[tauri::command]
pub async fn list_offline_changes() -> Result<Vec<OfflineChangeRow>, String> {
    Err("list_offline_changes is not implemented yet".to_string())
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
