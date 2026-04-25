//! Node snapshot types for layout directory persistence.

use std::collections::BTreeMap;
use lcc_rs::NodeID;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLeafValue {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SnapshotValueNode {
    Branch(BTreeMap<String, SnapshotValueNode>),
    Leaf(SnapshotLeafValue),
}

#[derive(Debug, Clone)]
pub struct SnapshotValueEntry {
    pub path: Vec<String>,
    pub leaf: SnapshotLeafValue,
}

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
    #[serde(with = "crate::layout::serde_node_id::canonical")]
    pub node_id: NodeID,
    pub captured_at: String,
    pub capture_status: CaptureStatus,
    #[serde(default)]
    pub missing: Vec<String>,
    pub snip: SnipSnapshot,
    pub cdi_ref: CdiReference,
    /// Canonical path-centric value tree keyed by CDI element hierarchy.
    #[serde(default)]
    pub config: BTreeMap<String, SnapshotValueNode>,
    #[serde(default)]
    pub producer_identified_events: Vec<String>,
}

impl NodeSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.capture_status == CaptureStatus::Partial && self.missing.is_empty() {
            return Err("partial snapshots must include missing details".to_string());
        }
        Ok(())
    }

    pub fn add_config_leaf(&mut self, path: &[String], leaf: SnapshotLeafValue) {
        insert_config_leaf(&mut self.config, path, leaf);
    }

    pub fn flattened_config_entries(&self) -> Vec<SnapshotValueEntry> {
        let mut out = Vec::new();
        flatten_config(&self.config, &mut Vec::new(), &mut out);
        out
    }
}

pub fn insert_config_leaf(
    root: &mut BTreeMap<String, SnapshotValueNode>,
    path: &[String],
    leaf: SnapshotLeafValue,
) {
    if path.is_empty() {
        return;
    }

    let key = path[0].clone();
    if path.len() == 1 {
        root.insert(key, SnapshotValueNode::Leaf(leaf));
        return;
    }

    let child = root
        .entry(key)
        .or_insert_with(|| SnapshotValueNode::Branch(BTreeMap::new()));

    match child {
        SnapshotValueNode::Branch(children) => {
            insert_config_leaf(children, &path[1..], leaf);
        }
        SnapshotValueNode::Leaf(_) => {
            let mut replacement = BTreeMap::new();
            insert_config_leaf(&mut replacement, &path[1..], leaf);
            *child = SnapshotValueNode::Branch(replacement);
        }
    }
}

fn flatten_config(
    root: &BTreeMap<String, SnapshotValueNode>,
    path: &mut Vec<String>,
    out: &mut Vec<SnapshotValueEntry>,
) {
    for (key, node) in root {
        path.push(key.clone());
        match node {
            SnapshotValueNode::Branch(children) => flatten_config(children, path, out),
            SnapshotValueNode::Leaf(leaf) => out.push(SnapshotValueEntry {
                path: path.clone(),
                leaf: leaf.clone(),
            }),
        }
        let _ = path.pop();
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

/// Update the baseline `value` for a leaf in the snapshot's config tree
/// matching the given (space, offset). Returns `true` if a leaf was updated.
pub fn update_snapshot_baseline(
    config: &mut BTreeMap<String, SnapshotValueNode>,
    space: u8,
    offset: &str,
    new_value: &str,
) -> bool {
    for node in config.values_mut() {
        match node {
            SnapshotValueNode::Branch(children) => {
                if update_snapshot_baseline(children, space, offset, new_value) {
                    return true;
                }
            }
            SnapshotValueNode::Leaf(leaf) => {
                if leaf.space == Some(space) && leaf.offset.as_deref() == Some(offset) {
                    leaf.value = new_value.to_string();
                    return true;
                }
            }
        }
    }
    false
}

pub fn update_snapshot_baseline_and_capture_time(
    snapshot: &mut NodeSnapshot,
    space: u8,
    offset: &str,
    new_value: &str,
    captured_at: &str,
) -> bool {
    let updated = update_snapshot_baseline(&mut snapshot.config, space, offset, new_value);
    if updated {
        snapshot.captured_at = captured_at.to_string();
    }
    updated
}

#[cfg(test)]
mod tests {
    use super::{
        update_snapshot_baseline_and_capture_time, CdiReference, CaptureStatus, NodeSnapshot,
        SnapshotLeafValue, SnapshotValueNode, SnipSnapshot,
    };
    use lcc_rs::NodeID;
    use std::collections::BTreeMap;

    fn make_snapshot() -> NodeSnapshot {
        let mut config = BTreeMap::new();
        config.insert(
            "Main".to_string(),
            SnapshotValueNode::Leaf(SnapshotLeafValue {
                value: "10".to_string(),
                space: Some(253),
                offset: Some("0x00000010".to_string()),
            }),
        );

        NodeSnapshot {
            node_id: NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]),
            captured_at: "2026-04-20T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot::default(),
            cdi_ref: CdiReference {
                cache_key: "cache".to_string(),
                version: "1.0".to_string(),
                fingerprint: "fp".to_string(),
            },
            config,
            producer_identified_events: Vec::new(),
        }
    }

    #[test]
    fn updates_baseline_and_capture_time_when_leaf_matches() {
        let mut snapshot = make_snapshot();

        let updated = update_snapshot_baseline_and_capture_time(
            &mut snapshot,
            253,
            "0x00000010",
            "20",
            "2026-04-25T12:34:56Z",
        );

        assert!(updated);
        assert_eq!(snapshot.captured_at, "2026-04-25T12:34:56Z");

        match snapshot.config.get("Main") {
            Some(SnapshotValueNode::Leaf(leaf)) => assert_eq!(leaf.value, "20"),
            _ => panic!("expected matching snapshot leaf"),
        }
    }

    #[test]
    fn leaves_capture_time_unchanged_when_no_leaf_matches() {
        let mut snapshot = make_snapshot();

        let updated = update_snapshot_baseline_and_capture_time(
            &mut snapshot,
            253,
            "0x00000099",
            "20",
            "2026-04-25T12:34:56Z",
        );

        assert!(!updated);
        assert_eq!(snapshot.captured_at, "2026-04-20T00:00:00Z");
    }
}
