//! Node snapshot types for layout directory persistence.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiReference {
    pub cache_key: String,
    pub version: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnipSnapshot {
    pub user_name: String,
    pub user_description: String,
    pub manufacturer_name: String,
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSnapshot {
    pub node_id: String,
    pub captured_at: String,
    pub capture_status: CaptureStatus,
    #[serde(default)]
    pub missing: Vec<String>,
    pub snip: SnipSnapshot,
    pub cdi_ref: CdiReference,
    #[serde(default)]
    pub values: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub producer_identified_events: Vec<String>,
}

impl NodeSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.node_id.trim().is_empty() {
            return Err("nodeId must not be empty".to_string());
        }
        if self.capture_status == CaptureStatus::Partial && self.missing.is_empty() {
            return Err("partial snapshots must include missing details".to_string());
        }
        Ok(())
    }
}

pub fn capture_status_from_missing(missing: &[String]) -> CaptureStatus {
    if missing.is_empty() {
        CaptureStatus::Complete
    } else {
        CaptureStatus::Partial
    }
}

pub fn missing_detail(space: u8, offset_hex: &str, path: &[String]) -> String {
    format!(
        "space={} offset={} path={}",
        space,
        offset_hex,
        path.join("/")
    )
}

pub fn canonical_node_filename(node_id: &str) -> String {
    format!("{}.yaml", node_id.to_uppercase())
}
