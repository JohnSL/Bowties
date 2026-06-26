//! Channel event ID resolution — maps a channel's hardware reference to its
//! producer event IDs using the cached config tree and profile annotations.

use crate::node_tree::{ConfigNode, ConfigValue, LeafNode, LeafType, NodeConfigTree};
use lcc_rs::cdi::EventRole;
use std::collections::HashMap;

/// Result of resolving event IDs for a single channel.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelEventIds {
    pub channel_id: String,
    /// Map from state name (e.g. "occupied", "clear") to event ID hex string.
    pub event_ids: HashMap<String, String>,
}

/// Resolve event IDs for a batch of channels from their cached config trees.
///
/// For each channel:
/// 1. Find the connector slot matching the channel's connector
/// 2. Use `resolved_affected_paths[input - 1]` as the Line group path prefix
/// 3. Find all EventId leaves with `event_role == Producer` under that prefix
/// 4. Index into them with the eventMapping's `producerLeafIndex`
///
/// Channels whose tree isn't available, whose config hasn't been read, or whose
/// event IDs can't be resolved are returned with an empty `event_ids` map.
pub fn resolve_channel_event_ids(
    tree: &NodeConfigTree,
    connector: &str,
    input: u32,
    event_mapping: &HashMap<String, u32>, // state_name → producerLeafIndex
) -> HashMap<String, String> {
    let mut result = HashMap::new();

    // Find the connector slot
    let profile = match &tree.connector_profile {
        Some(p) => p,
        None => return result,
    };

    let slot = match profile.slots.iter().find(|s| s.slot_id == connector) {
        Some(s) => s,
        None => return result,
    };

    // Get the resolved path prefix for this input (1-based → 0-based index)
    let path_prefix = match slot.resolved_affected_paths.get((input as usize).saturating_sub(1)) {
        Some(p) => p,
        None => return result,
    };

    // Collect all producer EventId leaves under this path prefix
    let producer_leaves = collect_producer_leaves_under_prefix(tree, path_prefix);

    // Index into the producer leaves by producerLeafIndex for each state
    for (state_name, &leaf_index) in event_mapping {
        if let Some(leaf) = producer_leaves.get(leaf_index as usize) {
            if let Some(ConfigValue::EventId { hex, .. }) = &leaf.value {
                result.insert(state_name.clone(), hex.clone());
            }
        }
    }

    result
}

/// Collect all EventId leaves marked as Producer under the given path prefix,
/// in tree-traversal order (which matches CDI declaration order).
fn collect_producer_leaves_under_prefix<'a>(
    tree: &'a NodeConfigTree,
    path_prefix: &[String],
) -> Vec<&'a LeafNode> {
    let mut results = Vec::new();
    for seg in &tree.segments {
        collect_from_children(&seg.children, path_prefix, &mut results);
    }
    results
}

fn collect_from_children<'a>(
    children: &'a [ConfigNode],
    path_prefix: &[String],
    results: &mut Vec<&'a LeafNode>,
) {
    for child in children {
        match child {
            ConfigNode::Leaf(leaf) => {
                if leaf.element_type == LeafType::EventId
                    && leaf.event_role == Some(EventRole::Producer)
                    && path_starts_with(&leaf.path, path_prefix)
                {
                    results.push(leaf);
                }
            }
            ConfigNode::Group(group) => {
                collect_from_children(&group.children, path_prefix, results);
            }
        }
    }
}

/// Check if a leaf's path starts with the given prefix.
fn path_starts_with(path: &[String], prefix: &[String]) -> bool {
    if path.len() < prefix.len() {
        return false;
    }
    path[..prefix.len()] == *prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_tree::*;

    fn make_eventid_leaf(path: Vec<&str>, role: Option<EventRole>, hex: &str) -> ConfigNode {
        ConfigNode::Leaf(LeafNode {
            name: "Event".to_string(),
            description: None,
            element_type: LeafType::EventId,
            address: 0,
            size: 8,
            space: 253,
            path: path.into_iter().map(String::from).collect(),
            value: Some(ConfigValue::EventId {
                bytes: [0; 8],
                hex: hex.to_string(),
            }),
            event_role: role,
            constraints: None,
            button_text: None,
            dialog_text: None,
            action_value: 0,
            hint_slider: None,
            hint_radio: false,
            modified_value: None,
            write_state: None,
            write_error: None,
            read_only: false,
        })
    }

    fn make_test_tree() -> NodeConfigTree {
        // Simulate a Tower-LCC tree with connector-a owning Lines 1-4
        // Line#1 has two producer events and one consumer event
        NodeConfigTree {
            node_id: "050101010100".to_string(),
            identity: None,
            connector_profile: Some(ConnectorProfile {
                node_id: "050101010100".to_string(),
                carrier_key: "rr-cirkits::tower-lcc".to_string(),
                slots: vec![ConnectorSlot {
                    slot_id: "connector-a".to_string(),
                    label: "Connector A".to_string(),
                    order: 0,
                    allow_none_installed: true,
                    supported_daughterboard_ids: vec!["BOD4".to_string()],
                    affected_paths: vec!["Port I/O/Line".to_string()],
                    resolved_affected_paths: vec![
                        vec!["seg:0".to_string(), "elem:0#1".to_string()],
                        vec!["seg:0".to_string(), "elem:0#2".to_string()],
                        vec!["seg:0".to_string(), "elem:0#3".to_string()],
                        vec!["seg:0".to_string(), "elem:0#4".to_string()],
                    ],
                    base_behavior_when_empty: None,
                    supported_daughterboard_constraints: vec![],
                }],
                supported_daughterboards: vec![SupportedDaughterboard {
                    daughterboard_id: "BOD4".to_string(),
                    display_name: "BOD4".to_string(),
                    kind: Some("detection".to_string()),
                    description: None,
                    channel_inputs: vec![],
                }],
            }),
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: true,
            segments: vec![SegmentNode {
                name: "Port I/O".to_string(),
                description: None,
                origin: 0,
                space: 253,
                children: vec![
                    // Line#1 group
                    ConfigNode::Group(GroupNode {
                        name: "Line".to_string(),
                        has_name: true,
                        description: None,
                        instance: 1,
                        instance_label: "Line 1".to_string(),
                        replication_of: "Line".to_string(),
                        replication_count: 16,
                        path: vec!["seg:0".to_string(), "elem:0#1".to_string()],
                        children: vec![
                            // Consumer event (Event#1) - should be skipped
                            make_eventid_leaf(
                                vec!["seg:0", "elem:0#1", "elem:0", "elem:0"],
                                Some(EventRole::Consumer),
                                "0501010101FF0001",
                            ),
                            // Producer events (Event#2)
                            make_eventid_leaf(
                                vec!["seg:0", "elem:0#1", "elem:1", "elem:0"],
                                Some(EventRole::Producer),
                                "0501010101000001", // occupied (leaf 0)
                            ),
                            make_eventid_leaf(
                                vec!["seg:0", "elem:0#1", "elem:1", "elem:1"],
                                Some(EventRole::Producer),
                                "0501010101000002", // clear (leaf 1)
                            ),
                        ],
                        display_name: None,
                        hideable: false,
                        hidden_by_default: false,
                        read_only: false,
                    }),
                    // Line#2 group
                    ConfigNode::Group(GroupNode {
                        name: "Line".to_string(),
                        has_name: true,
                        description: None,
                        instance: 2,
                        instance_label: "Line 2".to_string(),
                        replication_of: "Line".to_string(),
                        replication_count: 16,
                        path: vec!["seg:0".to_string(), "elem:0#2".to_string()],
                        children: vec![
                            make_eventid_leaf(
                                vec!["seg:0", "elem:0#2", "elem:1", "elem:0"],
                                Some(EventRole::Producer),
                                "0501010102000001", // occupied (leaf 0)
                            ),
                            make_eventid_leaf(
                                vec!["seg:0", "elem:0#2", "elem:1", "elem:1"],
                                Some(EventRole::Producer),
                                "0501010102000002", // clear (leaf 1)
                            ),
                        ],
                        display_name: None,
                        hideable: false,
                        hidden_by_default: false,
                        read_only: false,
                    }),
                ],
            }],
        }
    }

    #[test]
    fn resolves_occupied_and_clear_for_input_1() {
        let tree = make_test_tree();
        let mut event_mapping = HashMap::new();
        event_mapping.insert("occupied".to_string(), 0u32);
        event_mapping.insert("clear".to_string(), 1u32);

        let result = resolve_channel_event_ids(&tree, "connector-a", 1, &event_mapping);

        assert_eq!(result.get("occupied"), Some(&"0501010101000001".to_string()));
        assert_eq!(result.get("clear"), Some(&"0501010101000002".to_string()));
    }

    #[test]
    fn resolves_for_input_2() {
        let tree = make_test_tree();
        let mut event_mapping = HashMap::new();
        event_mapping.insert("occupied".to_string(), 0u32);
        event_mapping.insert("clear".to_string(), 1u32);

        let result = resolve_channel_event_ids(&tree, "connector-a", 2, &event_mapping);

        assert_eq!(result.get("occupied"), Some(&"0501010102000001".to_string()));
        assert_eq!(result.get("clear"), Some(&"0501010102000002".to_string()));
    }

    #[test]
    fn returns_empty_for_unknown_connector() {
        let tree = make_test_tree();
        let mut event_mapping = HashMap::new();
        event_mapping.insert("occupied".to_string(), 0u32);

        let result = resolve_channel_event_ids(&tree, "connector-z", 1, &event_mapping);

        assert!(result.is_empty());
    }

    #[test]
    fn returns_empty_for_out_of_range_input() {
        let tree = make_test_tree();
        let mut event_mapping = HashMap::new();
        event_mapping.insert("occupied".to_string(), 0u32);

        let result = resolve_channel_event_ids(&tree, "connector-a", 99, &event_mapping);

        assert!(result.is_empty());
    }

    #[test]
    fn returns_empty_when_no_connector_profile() {
        let mut tree = make_test_tree();
        tree.connector_profile = None;
        let mut event_mapping = HashMap::new();
        event_mapping.insert("occupied".to_string(), 0u32);

        let result = resolve_channel_event_ids(&tree, "connector-a", 1, &event_mapping);

        assert!(result.is_empty());
    }

    /// Regression test: resolved event IDs use the same canonical contiguous format
    /// as PCER events from the EventRouter (ADR-0010). Without this, deriveChannelState()
    /// silently fails because string equality on different formats never matches.
    #[test]
    fn resolved_event_ids_match_pcer_canonical_format() {
        let tree = make_test_tree();
        let mut event_mapping = HashMap::new();
        event_mapping.insert("occupied".to_string(), 0u32);

        let result = resolve_channel_event_ids(&tree, "connector-a", 1, &event_mapping);
        let occupied = result.get("occupied").expect("occupied event ID should be resolved");

        // Canonical contiguous hex: 16 uppercase chars, no dots
        assert_eq!(occupied.len(), 16, "event ID must be 16 contiguous hex chars");
        assert!(!occupied.contains('.'), "event ID must not contain dots (canonical form)");
        assert_eq!(occupied, &occupied.to_uppercase(), "event ID must be uppercase");

        // The EventID::to_canonical() format must produce the same output
        let bytes = [0x05, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x01];
        let pcer_format = lcc_rs::EventID::new(bytes).to_canonical();
        assert_eq!(occupied, &pcer_format, "resolved event ID must match PCER canonical format");
    }

    /// Regression test (Spec 017 / S3): resolution requires that producer
    /// event ID leaves carry `event_role = Some(Producer)`. A tree built from
    /// saved snapshot values via `build_node_config_tree + merge_snapshot_path_values`
    /// has the right structure and values but no `event_role` annotations —
    /// `find_producer_leaves` filters every leaf out and the result is empty.
    /// `open_layout_directory` must call `apply_profile_metadata_to_tree` on
    /// each saved tree before stuffing it into `node_registry.saved_trees`,
    /// otherwise live discovery of a saved node seeds the proxy with an
    /// unannotated tree and channel indicators stay at `'no-config'` until
    /// the user forces a CDI read.
    #[test]
    fn returns_empty_when_producer_leaves_lack_event_role_annotations() {
        let mut tree = make_test_tree();
        // Strip event_role from every leaf in the tree, mimicking the state of
        // a saved tree that has not been run through `apply_profile_metadata_to_tree`.
        fn strip_roles(children: &mut Vec<ConfigNode>) {
            for child in children {
                match child {
                    ConfigNode::Leaf(leaf) => { leaf.event_role = None; }
                    ConfigNode::Group(group) => { strip_roles(&mut group.children); }
                }
            }
        }
        for seg in &mut tree.segments {
            strip_roles(&mut seg.children);
        }

        let mut event_mapping = HashMap::new();
        event_mapping.insert("occupied".to_string(), 0u32);
        event_mapping.insert("clear".to_string(), 1u32);

        let result = resolve_channel_event_ids(&tree, "connector-a", 1, &event_mapping);

        assert!(
            result.is_empty(),
            "without event_role annotations, resolve must return empty — this is the gap S3 fixes at the layout-open seam"
        );
    }
}
