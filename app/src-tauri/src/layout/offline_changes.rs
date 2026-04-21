//! Offline change row types for persisted pending edits.

use lcc_rs::NodeID;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OfflineChangeKind {
    Config,
    BowtieMetadata,
    BowtieEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OfflineChangeStatus {
    Pending,
    Conflict,
    Clean,
    AlreadyApplied,
    Skipped,
    Applied,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineChange {
    pub change_id: String,
    pub kind: OfflineChangeKind,
    #[serde(with = "crate::layout::serde_node_id::canonical_option")]
    pub node_id: Option<NodeID>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub status: OfflineChangeStatus,
    pub error: Option<String>,
    pub updated_at: String,
}

impl OfflineChange {
    pub fn validate(&self) -> Result<(), String> {
        if self.change_id.trim().is_empty() {
            return Err("changeId must not be empty".to_string());
        }
        if self.status == OfflineChangeStatus::Failed && self.error.as_deref().unwrap_or_default().trim().is_empty() {
            return Err("failed rows must include error text".to_string());
        }
        Ok(())
    }
}
