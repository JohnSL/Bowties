//! Channel event ID resolution — maps a channel's hardware reference to its
//! producer event IDs using the cached config tree and profile annotations.

use crate::node_tree::{
    replication_instances, ConfigNode, ConfigValue, LeafNode, LeafType, NodeConfigTree,
};
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

/// Shape-agnostic event-id resolver. Collects EventId leaves under `path_prefix`
/// whose `event_role` matches `role` (in CDI declaration order), then indexes
/// them by `leaf_index_map` (state name → leaf ordinal within the role-filtered
/// subset). Returns a map from state name to canonical event-ID hex.
///
/// The two binding shapes built on top of this helper are:
/// - `connectorInput` + `Producer` — via [`resolve_channel_event_ids`].
/// - `lampRow` + `Consumer` — via the IPC adapter that calls
///   [`resolve_lamp_row_path_prefix`] and dispatches to this function.
pub fn resolve_event_ids(
    tree: &NodeConfigTree,
    path_prefix: &[String],
    role: EventRole,
    leaf_index_map: &HashMap<String, u32>,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let leaves = collect_event_leaves_under_prefix(tree, path_prefix, role);

    for (state_name, &leaf_index) in leaf_index_map {
        if let Some(leaf) = leaves.get(leaf_index as usize) {
            if let Some(ConfigValue::EventId { hex, .. }) = &leaf.value {
                result.insert(state_name.clone(), hex.clone());
            }
        }
    }

    result
}

/// Resolve event IDs for a `connectorInput` + `Producer` channel.
///
/// Thin wrapper around [`resolve_event_ids`] that:
/// 1. Finds the connector slot matching the channel's connector
/// 2. Uses `resolved_affected_paths[input - 1]` as the Line group path prefix
/// 3. Calls [`resolve_event_ids`] with `EventRole::Producer`
pub fn resolve_channel_event_ids(
    tree: &NodeConfigTree,
    connector: &str,
    input: u32,
    event_mapping: &HashMap<String, u32>, // state_name → producerLeafIndex
) -> HashMap<String, String> {
    let path_prefix = match resolve_connector_input_path_prefix(tree, connector, input) {
        Some(p) => p,
        None => return HashMap::new(),
    };
    resolve_event_ids(tree, &path_prefix, EventRole::Producer, event_mapping)
}

/// Compute the CDI path prefix for a connector input.
///
/// Returns `None` if the tree has no connector profile, the connector id is
/// unknown, or the input ordinal is out of range.
pub fn resolve_connector_input_path_prefix(
    tree: &NodeConfigTree,
    connector: &str,
    input: u32,
) -> Option<Vec<String>> {
    let profile = tree.connector_profile.as_ref()?;
    let slot = profile.slots.iter().find(|s| s.slot_id == connector)?;
    slot.resolved_affected_paths
        .get((input as usize).saturating_sub(1))
        .cloned()
}

/// Compute the CDI path prefix for a `Direct Lamp Control` row.
///
/// Walks the tree for a segment named `"Direct Lamp Control"` and returns the
/// path of its `Lamp` group whose 1-based replication `instance` equals
/// `row_ordinal`. Returns `None` if the segment is absent or the ordinal is
/// past the last replication.
///
/// Uses [`replication_instances`] so the wrapper/sibling shape produced by
/// `build_children` is handled in one place.
pub fn resolve_lamp_row_path_prefix(
    tree: &NodeConfigTree,
    row_ordinal: u32,
) -> Option<Vec<String>> {
    for seg in &tree.segments {
        if seg.name != "Direct Lamp Control" {
            continue;
        }
        return replication_instances(&seg.children, "Lamp")
            .into_iter()
            .find(|g| g.instance == row_ordinal)
            .map(|g| g.path.clone());
    }
    None
}

/// Collect EventId leaves matching `role` under `path_prefix`, in tree-traversal
/// order (which matches CDI declaration order).
fn collect_event_leaves_under_prefix<'a>(
    tree: &'a NodeConfigTree,
    path_prefix: &[String],
    role: EventRole,
) -> Vec<&'a LeafNode> {
    let mut results = Vec::new();
    for seg in &tree.segments {
        collect_from_children(&seg.children, path_prefix, role, &mut results);
    }
    results
}

fn collect_from_children<'a>(
    children: &'a [ConfigNode],
    path_prefix: &[String],
    role: EventRole,
    results: &mut Vec<&'a LeafNode>,
) {
    for child in children {
        match child {
            ConfigNode::Leaf(leaf) => {
                if leaf.element_type == LeafType::EventId
                    && leaf.event_role == Some(role)
                    && path_starts_with(&leaf.path, path_prefix)
                {
                    results.push(leaf);
                }
            }
            ConfigNode::Group(group) => {
                collect_from_children(&group.children, path_prefix, role, results);
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

    // -- Shape-agnostic resolver tests (Spec 018 / S5) --

    /// Build a tree with a `Direct Lamp Control` segment containing N replicated
    /// `Lamp` groups, each carrying a consumer "Lamp On" + "Lamp Off" event pair.
    fn make_lamp_tree(lamp_count: u32) -> NodeConfigTree {
        let mut children = Vec::new();
        for instance in 1..=lamp_count {
            let group_path = vec![
                "seg:0".to_string(),
                format!("elem:0#{instance}"),
            ];
            children.push(ConfigNode::Group(GroupNode {
                name: format!("Lamp {instance}"),
                has_name: true,
                description: None,
                instance,
                instance_label: format!("Lamp {instance}"),
                replication_of: "Lamp".to_string(),
                replication_count: lamp_count,
                path: group_path.clone(),
                children: vec![
                    make_eventid_leaf(
                        vec![
                            "seg:0",
                            &format!("elem:0#{instance}"),
                            "elem:0",
                        ],
                        Some(EventRole::Consumer),
                        &format!("050101010100A{instance:03}"),
                    ),
                    make_eventid_leaf(
                        vec![
                            "seg:0",
                            &format!("elem:0#{instance}"),
                            "elem:1",
                        ],
                        Some(EventRole::Consumer),
                        &format!("050101010100B{instance:03}"),
                    ),
                ],
                display_name: None,
                hideable: false,
                hidden_by_default: false,
                read_only: false,
            }));
        }

        NodeConfigTree {
            node_id: "050101010100".to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: true,
            segments: vec![SegmentNode {
                name: "Direct Lamp Control".to_string(),
                description: None,
                origin: 0,
                space: 253,
                children,
            }],
        }
    }

    #[test]
    fn resolve_event_ids_with_consumer_role_collects_consumer_leaves_only() {
        let tree = make_lamp_tree(4);
        let path_prefix = resolve_lamp_row_path_prefix(&tree, 2)
            .expect("Lamp#2 path must resolve");
        let mut mapping = HashMap::new();
        mapping.insert("lit".to_string(), 0u32);
        mapping.insert("unlit".to_string(), 1u32);

        let result = resolve_event_ids(&tree, &path_prefix, EventRole::Consumer, &mapping);

        assert_eq!(result.get("lit"), Some(&"050101010100A002".to_string()));
        assert_eq!(result.get("unlit"), Some(&"050101010100B002".to_string()));
    }

    #[test]
    fn resolve_event_ids_with_producer_role_skips_consumer_leaves() {
        let tree = make_lamp_tree(1);
        let path_prefix = resolve_lamp_row_path_prefix(&tree, 1)
            .expect("Lamp#1 path must resolve");
        let mut mapping = HashMap::new();
        mapping.insert("lit".to_string(), 0u32);

        let result = resolve_event_ids(&tree, &path_prefix, EventRole::Producer, &mapping);

        assert!(
            result.is_empty(),
            "consumer-only tree must yield zero hits when asked for producer leaves"
        );
    }

    #[test]
    fn resolve_lamp_row_path_prefix_returns_path_for_instance() {
        let tree = make_lamp_tree(8);

        let prefix_1 = resolve_lamp_row_path_prefix(&tree, 1).expect("Lamp#1");
        assert_eq!(prefix_1, vec!["seg:0".to_string(), "elem:0#1".to_string()]);

        let prefix_8 = resolve_lamp_row_path_prefix(&tree, 8).expect("Lamp#8");
        assert_eq!(prefix_8, vec!["seg:0".to_string(), "elem:0#8".to_string()]);
    }

    #[test]
    fn resolve_lamp_row_path_prefix_returns_none_past_last_replication() {
        let tree = make_lamp_tree(4);
        assert!(resolve_lamp_row_path_prefix(&tree, 5).is_none());
        assert!(resolve_lamp_row_path_prefix(&tree, 999).is_none());
    }

    #[test]
    fn resolve_lamp_row_path_prefix_returns_none_when_segment_absent() {
        // make_test_tree() has only a "Port I/O" segment.
        let tree = make_test_tree();
        assert!(resolve_lamp_row_path_prefix(&tree, 1).is_none());
    }

    #[test]
    fn resolve_connector_input_path_prefix_matches_byte_identical_legacy() {
        // The wrapper's behaviour must be unchanged: resolve_channel_event_ids and
        // the standalone path-prefix helper agree on the prefix for input 1.
        let tree = make_test_tree();
        let prefix = resolve_connector_input_path_prefix(&tree, "connector-a", 1)
            .expect("connector-a input 1 prefix");
        assert_eq!(prefix, vec!["seg:0".to_string(), "elem:0#1".to_string()]);

        let prefix_4 = resolve_connector_input_path_prefix(&tree, "connector-a", 4)
            .expect("connector-a input 4 prefix");
        assert_eq!(prefix_4, vec!["seg:0".to_string(), "elem:0#4".to_string()]);

        assert!(resolve_connector_input_path_prefix(&tree, "connector-a", 99).is_none());
        assert!(resolve_connector_input_path_prefix(&tree, "connector-z", 1).is_none());
    }

    /// Regression: a real `build_children` output for `<group replication="N">`
    /// emits a wrapper at segment level with the N instances as the wrapper's
    /// children. The old hand-rolled traversal inside
    /// `resolve_lamp_row_path_prefix` only inspected segment-level siblings and
    /// silently returned `None` for every ordinal against this real shape.
    /// Spec 018 quickchange — go through `replication_instances`.
    #[test]
    fn resolve_lamp_row_path_prefix_walks_real_build_children_wrapper_shape() {
        use crate::node_tree::build_node_config_tree;
        use lcc_rs::cdi::parser::parse_cdi;

        let cdi = parse_cdi(
            r#"<cdi>
                <segment space="253" origin="8192">
                    <name>Direct Lamp Control</name>
                    <group replication="16">
                        <name>Lamp</name>
                        <repname>Lamp</repname>
                        <string size="32"><name>Lamp Description</name></string>
                        <eventid><name>Lamp On</name></eventid>
                        <eventid><name>Lamp Off</name></eventid>
                    </group>
                </segment>
            </cdi>"#,
        )
        .expect("CDI parse");
        let tree = build_node_config_tree("05.01.01.01.FF.10", &cdi);

        // Sanity: real build_children emits the wrapper shape.
        let seg = &tree.segments[0];
        assert_eq!(seg.children.len(), 1, "wrapper-shape: one wrapper");

        for ordinal in 1..=16u32 {
            let prefix = resolve_lamp_row_path_prefix(&tree, ordinal)
                .unwrap_or_else(|| panic!("no path for Lamp#{ordinal}"));
            assert_eq!(prefix[0], "seg:0");
            assert_eq!(
                prefix[1],
                format!("elem:0#{ordinal}"),
                "Lamp#{ordinal} path step"
            );
        }
        assert!(resolve_lamp_row_path_prefix(&tree, 17).is_none());
    }
}
