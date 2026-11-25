//! Pending change row types for persisted pending edits.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PendingChangeKind {
    Config,
    BowtieMetadata,
    BowtieEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PendingChangeStatus {
    Pending,
    Conflict,
    Clean,
    AlreadyApplied,
    Skipped,
    Applied,
    Failed,
}

impl PendingChangeStatus {
    /// True while the board has not confirmed the change: anything except
    /// applied, already applied or clean.
    pub fn is_waiting(&self) -> bool {
        !matches!(
            self,
            PendingChangeStatus::Applied | PendingChangeStatus::AlreadyApplied | PendingChangeStatus::Clean
        )
    }
}

/// Which facility staged a Pending Change row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingChangeOrigin {
    pub facility_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingChange {
    pub change_id: String,
    pub kind: PendingChangeKind,
    /// The node this change targets, as a `NodeKey` string — canonical hex
    /// NodeID for real nodes, `"placeholder:<uuid>"` for placeholders.
    /// `None` for non-node-scoped changes (e.g. bowtie metadata).
    #[serde(alias = "nodeId")]
    pub node_key: Option<String>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub status: PendingChangeStatus,
    pub error: Option<String>,
    pub updated_at: String,
    /// The facility that staged this row; `None` for a hand edit. Absent in
    /// files written before origin existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<PendingChangeOrigin>,
}

impl PendingChange {
    pub fn validate(&self) -> Result<(), String> {
        if self.change_id.trim().is_empty() {
            return Err("changeId must not be empty".to_string());
        }
        if self.status == PendingChangeStatus::Failed && self.error.as_deref().unwrap_or_default().trim().is_empty() {
            return Err("failed rows must include error text".to_string());
        }
        Ok(())
    }
}
