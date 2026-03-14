//! Unified Node Configuration Tree
//!
//! A single canonical data model that merges CDI structure, computed addresses,
//! config values, and event roles into one tree per node.  The frontend receives
//! the full tree and handles all navigation/rendering locally.
//!
//! ## Design rationale
//!
//! The previous architecture scattered CDI data across three parallel caches
//! (`node.cdi` raw XML, `CDI_PARSE_CACHE` parsed structs, `config_value_cache`
//! EventId bytes) and required 12+ purpose-built Tauri navigation commands.
//! This module replaces that fragmented state with a single tree that:
//!
//! 1. Preserves the full CDI hierarchy (fixing the "flat siblings" display bug)
//! 2. Embeds computed absolute addresses (no re-derivation needed)
//! 3. Merges config values and event roles directly onto leaf nodes
//! 4. Is cheaply serializable to the frontend in one call

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use lcc_rs::cdi::{
    Cdi, DataElement, EventRole, Identification,
};

// ─────────────────────────────────────────────────────────────────────────────
// Tree node types
// ─────────────────────────────────────────────────────────────────────────────

/// Root of the unified configuration tree for a single LCC node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfigTree {
    /// Node identifier (dotted-hex, e.g. "05.02.01.02.03.00")
    pub node_id: String,
    /// Optional identification from CDI `<identification>` element
    pub identity: Option<Identification>,
    /// Top-level segments mirroring CDI `<segment>` elements
    pub segments: Vec<SegmentNode>,
}

/// One CDI segment — a contiguous memory space.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentNode {
    /// Segment display name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Starting address in memory space
    pub origin: u32,
    /// Memory space number (e.g. 253 for configuration)
    pub space: u8,
    /// Child nodes (groups and leaves)
    pub children: Vec<ConfigNode>,
}

/// A node in the configuration tree — either a group or a leaf element.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConfigNode {
    Group(GroupNode),
    Leaf(LeafNode),
}

/// A (possibly replicated) group of child nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    /// Display name for this group instance
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// 1-based replication instance number (1 when not replicated)
    pub instance: u32,
    /// Computed instance label (e.g. "Event 3")
    pub instance_label: String,
    /// Original group name before replication (for sibling disambiguation)
    pub replication_of: String,
    /// Total number of replications for this group template
    pub replication_count: u32,
    /// Index-based path identifying this group (e.g. ["seg:0", "elem:2#3"])
    pub path: Vec<String>,
    /// Child nodes
    pub children: Vec<ConfigNode>,
    /// Profile-supplied display-name override.
    ///
    /// When `Some`, the frontend renders this instead of `name`.  Set by
    /// `annotate_tree` from the `label` field of an `EventRoleDecl` (or a
    /// future `RelevanceRule` with a label).  `None` means "use `name`".
    #[serde(default)]
    pub display_name: Option<String>,
}

/// A leaf configuration element (int, string, eventid, float, action, blob).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeafNode {
    /// Display name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Type discriminator
    pub element_type: LeafType,
    /// Absolute memory address (origin + computed offset)
    pub address: u32,
    /// Size in bytes
    pub size: u32,
    /// Memory space number
    pub space: u8,
    /// Index-based path identifying this element
    pub path: Vec<String>,
    /// Current configuration value (populated after config read)
    pub value: Option<ConfigValue>,
    /// Classified event role (only meaningful for EventId leaves)
    pub event_role: Option<EventRole>,
    /// Constraints (min, max, default, map entries)
    pub constraints: Option<LeafConstraints>,
}

/// Discriminator for leaf element types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LeafType {
    Int,
    String,
    EventId,
    Float,
    Action,
    Blob,
}

/// A configuration value read from a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConfigValue {
    /// Integer value
    Int { value: i64 },
    /// String value (UTF-8)
    String { value: String },
    /// Event ID (8 bytes, stored as dotted-hex string for JSON)
    EventId { bytes: [u8; 8], hex: String },
    /// Float value
    Float { value: f64 },
}

/// Optional constraints on a leaf element.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeafConstraints {
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub default_value: Option<String>,
    pub map_entries: Option<Vec<MapEntry>>,
}

/// A single value→label mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapEntry {
    pub value: i64,
    pub label: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree builder
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `NodeConfigTree` from a parsed `Cdi` structure.
///
/// Walks every segment, expands replicated groups, computes absolute memory
/// addresses using the CDI cursor-based offset model, and returns the full tree.
///
/// This function does **not** populate `value` or `event_role` on leaf nodes;
/// use `merge_config_values` and `merge_event_roles` for that.
pub fn build_node_config_tree(node_id: &str, cdi: &Cdi) -> NodeConfigTree {
    let segments = cdi
        .segments
        .iter()
        .enumerate()
        .map(|(seg_idx, segment)| {
            let seg_path = vec![format!("seg:{}", seg_idx)];
            let children = build_children(
                &segment.elements,
                segment.origin as i32,
                segment.space,
                &seg_path,
            );
            SegmentNode {
                name: segment
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Segment {}", seg_idx)),
                description: segment.description.clone(),
                origin: segment.origin as u32,
                space: segment.space,
                children,
            }
        })
        .collect();

    NodeConfigTree {
        node_id: node_id.to_string(),
        identity: cdi.identification.clone(),
        segments,
    }
}

/// Recursively build children for a list of CDI data elements.
///
/// Implements the CDI cursor-based address model: each element's `offset`
/// attribute is a **relative skip** from the previous element's end to
/// this element's start.  The `base_address` is the absolute address of
/// the first byte of the enclosing scope (segment or group instance).
fn build_children(
    elements: &[DataElement],
    base_address: i32,
    space: u8,
    parent_path: &[String],
) -> Vec<ConfigNode> {
    let mut children = Vec::new();
    let mut cursor: i32 = 0;

    for (i, element) in elements.iter().enumerate() {
        match element {
            DataElement::Group(g) => {
                // Apply group's own offset skip before placing it.
                cursor += g.offset;
                let group_start = base_address + cursor;

                let stride = g.calculate_size();

                // Guard: stride=0 with replication>1 → clamp to 1 instance.
                let effective_replication = if stride == 0 && g.replication > 1 {
                    1u32
                } else {
                    g.replication
                };

                // Spacer groups (offset-only, no name/description/elements) were
                // preserved by the CDI parser for address-calculation correctness.
                // We've already advanced `cursor` above; now skip tree-node creation
                // so they don't pollute the visible config tree.
                if !g.should_render() {
                    cursor += effective_replication as i32 * stride;
                    continue;
                }

                let original_name = g
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Group {}", i));

                // Build the path step for the group itself (not per instance).
                let group_path_step = format!("elem:{}", i);
                let mut group_path = parent_path.to_vec();
                group_path.push(group_path_step.clone());

                if effective_replication > 1 {
                    // Create a wrapper GroupNode that contains the replicated instances.
                    // This preserves the hierarchy and makes sibling groups distinguishable.
                    let mut replicated_children = Vec::new();

                    for instance in 0..effective_replication {
                        let inst_num = instance + 1; // 1-based
                        let instance_path_step = format!("inst:{}", inst_num);

                        let mut child_path = group_path.clone();
                        child_path.push(instance_path_step);

                        let instance_base = group_start + instance as i32 * stride;
                        let instance_label = g.compute_repname(instance);

                        let child_nodes =
                            build_children(&g.elements, instance_base, space, &child_path);

                        replicated_children.push(ConfigNode::Group(GroupNode {
                            name: original_name.clone(),
                            description: None, // Instances don't repeat the description
                            instance: inst_num,
                            instance_label,
                            replication_of: original_name.clone(),
                            replication_count: effective_replication,
                            path: child_path,
                            children: child_nodes,
                            display_name: None,
                        }));
                    }

                    // Wrapper group: instance=0 indicates it's the template/wrapper.
                    children.push(ConfigNode::Group(GroupNode {
                        name: original_name.clone(),
                        description: g.description.clone(),
                        instance: 0,
                        instance_label: original_name.clone(), // Wrapper uses the group name
                        replication_of: original_name.clone(),
                        replication_count: effective_replication,
                        path: group_path,
                        children: replicated_children,
                        display_name: None,
                    }));
                } else {
                    // Non-replicated group (replication=1): no wrapper needed.
                    let instance_base = group_start;
                    let child_nodes =
                        build_children(&g.elements, instance_base, space, &group_path);

                    children.push(ConfigNode::Group(GroupNode {
                        name: original_name.clone(),
                        description: g.description.clone(),
                        instance: 1,
                        instance_label: original_name.clone(),
                        replication_of: original_name.clone(),
                        replication_count: 1,
                        path: group_path,
                        children: child_nodes,
                        display_name: None,
                    }));
                }

                // Advance cursor past all instances.
                cursor += effective_replication as i32 * stride;
            }
            DataElement::Int(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                let mut path = parent_path.to_vec();
                path.push(format!("elem:{}", i));
                children.push(ConfigNode::Leaf(LeafNode {
                    name: e.name.clone().unwrap_or_else(|| format!("Int {}", i)),
                    description: e.description.clone(),
                    element_type: LeafType::Int,
                    address: addr,
                    size: e.size as u32,
                    space,
                    path,
                    value: None,
                    event_role: None,
                    constraints: Some(LeafConstraints {
                        min: e.min,
                        max: e.max,
                        default_value: e.default.map(|v| v.to_string()),
                        map_entries: e.map.as_ref().map(|m| {
                            m.entries
                                .iter()
                                .map(|me| MapEntry {
                                    value: me.value,
                                    label: me.label.clone(),
                                })
                                .collect()
                        }),
                    }),
                }));
                cursor += e.size as i32;
            }
            DataElement::String(s) => {
                cursor += s.offset;
                let addr = (base_address + cursor) as u32;
                let mut path = parent_path.to_vec();
                path.push(format!("elem:{}", i));
                children.push(ConfigNode::Leaf(LeafNode {
                    name: s.name.clone().unwrap_or_else(|| format!("String {}", i)),
                    description: s.description.clone(),
                    element_type: LeafType::String,
                    address: addr,
                    size: s.size as u32,
                    space,
                    path,
                    value: None,
                    event_role: None,
                    constraints: None,
                }));
                cursor += s.size as i32;
            }
            DataElement::EventId(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                let mut path = parent_path.to_vec();
                path.push(format!("elem:{}", i));
                children.push(ConfigNode::Leaf(LeafNode {
                    name: e.name.clone().unwrap_or_else(|| format!("EventId {}", i)),
                    description: e.description.clone(),
                    element_type: LeafType::EventId,
                    address: addr,
                    size: 8,
                    space,
                    path,
                    value: None,
                    event_role: None,
                    constraints: None,
                }));
                cursor += 8;
            }
            DataElement::Float(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                let mut path = parent_path.to_vec();
                path.push(format!("elem:{}", i));
                children.push(ConfigNode::Leaf(LeafNode {
                    name: e.name.clone().unwrap_or_else(|| format!("Float {}", i)),
                    description: e.description.clone(),
                    element_type: LeafType::Float,
                    address: addr,
                    size: 4,
                    space,
                    path,
                    value: None,
                    event_role: None,
                    constraints: None,
                }));
                cursor += 4;
            }
            DataElement::Action(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                let mut path = parent_path.to_vec();
                path.push(format!("elem:{}", i));
                children.push(ConfigNode::Leaf(LeafNode {
                    name: e.name.clone().unwrap_or_else(|| format!("Action {}", i)),
                    description: e.description.clone(),
                    element_type: LeafType::Action,
                    address: addr,
                    size: 1,
                    space,
                    path,
                    value: None,
                    event_role: None,
                    constraints: None,
                }));
                cursor += 1;
            }
            DataElement::Blob(b) => {
                cursor += b.offset;
                let addr = (base_address + cursor) as u32;
                let mut path = parent_path.to_vec();
                path.push(format!("elem:{}", i));
                children.push(ConfigNode::Leaf(LeafNode {
                    name: b.name.clone().unwrap_or_else(|| format!("Blob {}", i)),
                    description: b.description.clone(),
                    element_type: LeafType::Blob,
                    address: addr,
                    size: b.size as u32,
                    space,
                    path,
                    value: None,
                    event_role: None,
                    constraints: None,
                }));
                cursor += b.size as i32;
            }
        }
    }

    children
}

// ─────────────────────────────────────────────────────────────────────────────
// Value and role merging
// ─────────────────────────────────────────────────────────────────────────────

/// Format 8 bytes as dotted-hex string (e.g. "05.02.01.02.03.00.00.01").
fn bytes_to_dotted_hex(bytes: &[u8; 8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(".")
}

/// Merge configuration values into an existing tree.
///
/// `values` is keyed by absolute memory address.  Each entry contains
/// the raw bytes read from the node's configuration memory.  This function
/// walks the tree and writes the appropriate `ConfigValue` variant onto
/// every matching leaf.
pub fn merge_config_values(tree: &mut NodeConfigTree, values: &HashMap<u32, Vec<u8>>) {
    for segment in &mut tree.segments {
        merge_children_values(&mut segment.children, values);
    }
}

fn merge_children_values(children: &mut [ConfigNode], values: &HashMap<u32, Vec<u8>>) {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                merge_children_values(&mut g.children, values);
            }
            ConfigNode::Leaf(leaf) => {
                if let Some(raw) = values.get(&leaf.address) {
                    leaf.value = parse_leaf_value(leaf.element_type, leaf.size, raw);
                }
            }
        }
    }
}

/// Parse raw bytes into a typed `ConfigValue` based on the element type.
fn parse_leaf_value(leaf_type: LeafType, size: u32, raw: &[u8]) -> Option<ConfigValue> {
    match leaf_type {
        LeafType::Int => {
            let val = match size {
                1 => raw.first().map(|&b| b as i64),
                2 if raw.len() >= 2 => Some(i16::from_be_bytes([raw[0], raw[1]]) as i64),
                4 if raw.len() >= 4 => {
                    Some(i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as i64)
                }
                8 if raw.len() >= 8 => Some(i64::from_be_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ])),
                _ => None,
            };
            val.map(|v| ConfigValue::Int { value: v })
        }
        LeafType::String => {
            // CDI strings are NUL-terminated within `size` bytes.
            // 0xFF bytes are uninitialized flash on LCC nodes — strip them out.
            let s: String = raw
                .iter()
                .take(size as usize)
                .take_while(|&&b| b != 0)
                .filter(|&&b| b != 0xFF)
                .map(|&b| b as char)
                .collect();
            Some(ConfigValue::String { value: s })
        }
        LeafType::EventId => {
            if raw.len() >= 8 {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&raw[..8]);
                let hex = bytes_to_dotted_hex(&bytes);
                Some(ConfigValue::EventId { bytes, hex })
            } else {
                None
            }
        }
        LeafType::Float => {
            if raw.len() >= 4 {
                let val = f32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
                Some(ConfigValue::Float {
                    value: val as f64,
                })
            } else {
                None
            }
        }
        // Action and Blob don't carry user-editable values
        LeafType::Action | LeafType::Blob => None,
    }
}

/// Merge event roles into an existing tree.
///
/// `roles` is keyed by element path (joined by "/").  This function walks
/// the tree and writes the `EventRole` onto every matching EventId leaf.
pub fn merge_event_roles(tree: &mut NodeConfigTree, roles: &HashMap<String, EventRole>) {
    for segment in &mut tree.segments {
        merge_children_roles(&mut segment.children, roles);
    }
}

fn merge_children_roles(children: &mut [ConfigNode], roles: &HashMap<String, EventRole>) {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                merge_children_roles(&mut g.children, roles);
            }
            ConfigNode::Leaf(leaf) if leaf.element_type == LeafType::EventId => {
                let path_key = leaf.path.join("/");
                if let Some(&role) = roles.get(&path_key) {
                    leaf.event_role = Some(role);
                }
            }
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree queries
// ─────────────────────────────────────────────────────────────────────────────

/// Collect all EventId leaves from the tree, returning (path, address, value) triples.
///
/// Used by the bowtie builder to gather event slots without re-parsing CDI XML.
pub fn collect_event_id_leaves(tree: &NodeConfigTree) -> Vec<EventIdLeafInfo> {
    let mut results = Vec::new();
    for segment in &tree.segments {
        collect_event_leaves_recursive(&segment.children, &mut results);
    }
    results
}

/// Info about one EventId leaf in the tree.
#[derive(Debug, Clone)]
pub struct EventIdLeafInfo {
    /// Index-based path
    pub path: Vec<String>,
    /// Absolute memory address
    pub address: u32,
    /// Leaf display name
    pub name: String,
    /// Leaf description
    pub description: Option<String>,
    /// Memory space
    pub space: u8,
    /// Current value (if read)
    pub value: Option<[u8; 8]>,
    /// Classified role (if determined)
    pub event_role: Option<EventRole>,
}

fn collect_event_leaves_recursive(children: &[ConfigNode], results: &mut Vec<EventIdLeafInfo>) {
    for child in children {
        match child {
            ConfigNode::Group(g) => {
                collect_event_leaves_recursive(&g.children, results);
            }
            ConfigNode::Leaf(leaf) if leaf.element_type == LeafType::EventId => {
                let value = match &leaf.value {
                    Some(ConfigValue::EventId { bytes, .. }) => Some(*bytes),
                    _ => None,
                };
                results.push(EventIdLeafInfo {
                    path: leaf.path.clone(),
                    address: leaf.address,
                    name: leaf.name.clone(),
                    description: leaf.description.clone(),
                    space: leaf.space,
                    value,
                    event_role: leaf.event_role,
                });
            }
            _ => {}
        }
    }
}

/// Count all leaf nodes in the tree.
pub fn count_leaves(tree: &NodeConfigTree) -> usize {
    tree.segments
        .iter()
        .map(|s| count_children_leaves(&s.children))
        .sum()
}

fn count_children_leaves(children: &[ConfigNode]) -> usize {
    children
        .iter()
        .map(|c| match c {
            ConfigNode::Group(g) => count_children_leaves(&g.children),
            ConfigNode::Leaf(_) => 1,
        })
        .sum()
}

/// Classify EventId leaves using protocol-level node roles.
///
/// Given the protocol-level `event_roles` map (event_id bytes → `NodeRoles`),
/// examine each EventId leaf in the tree that has a value.  If the node is
/// solely a producer of that event → `Producer`; solely a consumer → `Consumer`;
/// both → `Ambiguous` (the CDI heuristic or user input will resolve it later).
///
/// Returns a `HashMap<String, EventRole>` keyed by leaf path (joined with "/")
/// suitable for passing to `merge_event_roles`.
pub fn classify_leaf_roles_from_protocol(
    tree: &NodeConfigTree,
    event_roles: &HashMap<[u8; 8], crate::state::NodeRoles>,
) -> HashMap<String, EventRole> {
    let mut result = HashMap::new();
    let node_id = &tree.node_id;

    for leaf_info in collect_event_id_leaves(tree) {
        if let Some(bytes) = leaf_info.value {
            if let Some(roles) = event_roles.get(&bytes) {
                let is_producer = roles.producers.contains(node_id);
                let is_consumer = roles.consumers.contains(node_id);
                let role = match (is_producer, is_consumer) {
                    (true, false) => EventRole::Producer,
                    (false, true) => EventRole::Consumer,
                    (true, true) => EventRole::Ambiguous,
                    (false, false) => continue, // node not involved with this event
                };
                result.insert(leaf_info.path.join("/"), role);
            }
        }
    }

    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use lcc_rs::cdi::parser::parse_cdi;

    /// Helper: build a tree from CDI XML.
    fn tree_from_xml(xml: &str) -> NodeConfigTree {
        let cdi = parse_cdi(xml).expect("CDI parse failed");
        build_node_config_tree("05.02.01.02.03.00", &cdi)
    }

    // ── Basic tree construction ─────────────────────────────────────────

    #[test]
    fn simple_segment_with_int() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="128">
                    <name>Config</name>
                    <int size="2"><name>Speed</name></int>
                    <int size="1" offset="4"><name>Mode</name></int>
                </segment>
            </cdi>"#,
        );

        assert_eq!(tree.segments.len(), 1);
        let seg = &tree.segments[0];
        assert_eq!(seg.name, "Config");
        assert_eq!(seg.origin, 128);
        assert_eq!(seg.space, 253);
        assert_eq!(seg.children.len(), 2);

        // First int: address = origin(128) + cursor(0) = 128
        match &seg.children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.name, "Speed");
                assert_eq!(l.address, 128);
                assert_eq!(l.size, 2);
                assert_eq!(l.element_type, LeafType::Int);
                assert_eq!(l.path, vec!["seg:0", "elem:0"]);
            }
            _ => panic!("Expected leaf"),
        }

        // Second int: cursor = 0+2=2, skip offset=4 → cursor=6, address=128+6=134
        match &seg.children[1] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.name, "Mode");
                assert_eq!(l.address, 134);
                assert_eq!(l.size, 1);
                assert_eq!(l.path, vec!["seg:0", "elem:1"]);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn simple_group_no_replication() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group>
                        <name>Settings</name>
                        <int size="1"><name>A</name></int>
                        <int size="1"><name>B</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );

        assert_eq!(tree.segments[0].children.len(), 1);
        match &tree.segments[0].children[0] {
            ConfigNode::Group(g) => {
                assert_eq!(g.name, "Settings");
                assert_eq!(g.instance, 1);
                assert_eq!(g.replication_count, 1);
                assert_eq!(g.children.len(), 2);
                assert_eq!(g.path, vec!["seg:0", "elem:0"]);

                // Children addresses: 0, 1
                match &g.children[0] {
                    ConfigNode::Leaf(l) => assert_eq!(l.address, 0),
                    _ => panic!("Expected leaf"),
                }
                match &g.children[1] {
                    ConfigNode::Leaf(l) => assert_eq!(l.address, 1),
                    _ => panic!("Expected leaf"),
                }
            }
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn replicated_group() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group replication="3">
                        <name>Channel</name>
                        <repname>Ch</repname>
                        <int size="2"><name>Value</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );

        // With wrapper structure: 1 wrapper group containing 3 instance groups
        assert_eq!(tree.segments[0].children.len(), 1);

        match &tree.segments[0].children[0] {
            ConfigNode::Group(wrapper) => {
                // Wrapper has instance=0
                assert_eq!(wrapper.instance, 0);
                assert_eq!(wrapper.instance_label, "Channel");
                assert_eq!(wrapper.replication_of, "Channel");
                assert_eq!(wrapper.replication_count, 3);
                assert_eq!(wrapper.path, vec!["seg:0".to_string(), "elem:0".to_string()]);

                // 3 instance children
                assert_eq!(wrapper.children.len(), 3);

                for (idx, child) in wrapper.children.iter().enumerate() {
                    match child {
                        ConfigNode::Group(g) => {
                            assert_eq!(g.instance, (idx + 1) as u32);
                            assert_eq!(g.instance_label, format!("Ch {}", idx + 1));
                            assert_eq!(g.replication_of, "Channel");
                            assert_eq!(g.replication_count, 3);

                            // Each instance has one int of size 2, stride = 2
                            // Instance addresses: 0, 2, 4
                            match &g.children[0] {
                                ConfigNode::Leaf(l) => {
                                    assert_eq!(l.address, (idx * 2) as u32);
                                }
                                _ => panic!("Expected leaf"),
                            }

                            // Path: seg:0 / elem:0 / inst:N
                            assert_eq!(
                                g.path,
                                vec![
                                    "seg:0".to_string(),
                                    "elem:0".to_string(),
                                    format!("inst:{}", idx + 1)
                                ]
                            );
                        }
                        _ => panic!("Expected group"),
                    }
                }
            }
            _ => panic!("Expected wrapper group"),
        }
    }

    /// The Tower-LCC CDI bug scenario: two sibling groups both named "Event"
    /// with replication=6, one for consumers and one for producers.
    /// With wrapper structure: 2 wrapper groups (one for consumers, one for producers),
    /// each containing 6 instance groups. Users can now visually distinguish them.
    #[test]
    fn tower_lcc_dual_event_groups() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group replication="6">
                        <name>Event</name>
                        <repname>Event</repname>
                        <description>Consumer events</description>
                        <eventid><name>Event ID</name></eventid>
                    </group>
                    <group replication="6">
                        <name>Event</name>
                        <repname>Event</repname>
                        <description>Producer events</description>
                        <eventid><name>Event ID</name></eventid>
                    </group>
                </segment>
            </cdi>"#,
        );

        // With wrapper structure: 2 wrapper groups (one for each replicated group definition)
        assert_eq!(tree.segments[0].children.len(), 2);

        // First wrapper: Consumer events (elem:0)
        match &tree.segments[0].children[0] {
            ConfigNode::Group(wrapper) => {
                assert_eq!(wrapper.instance, 0);
                assert_eq!(wrapper.instance_label, "Event");
                assert_eq!(wrapper.description.as_deref(), Some("Consumer events"));
                assert_eq!(wrapper.replication_count, 6);
                assert_eq!(wrapper.path, vec!["seg:0".to_string(), "elem:0".to_string()]);

                // 6 instance children
                assert_eq!(wrapper.children.len(), 6);

                for (i, child) in wrapper.children.iter().enumerate() {
                    match child {
                        ConfigNode::Group(g) => {
                            assert_eq!(g.instance, (i + 1) as u32);
                            assert_eq!(g.instance_label, format!("Event {}", i + 1));
                            // Instances don't repeat the description (only wrapper has it)
                            assert_eq!(g.description, None);
                            // Path: seg:0 / elem:0 / inst:N
                            assert_eq!(
                                g.path,
                                vec![
                                    "seg:0".to_string(),
                                    "elem:0".to_string(),
                                    format!("inst:{}", i + 1)
                                ]
                            );
                            // Each eventid is 8 bytes, address = i * 8
                            match &g.children[0] {
                                ConfigNode::Leaf(l) => {
                                    assert_eq!(l.address, (i * 8) as u32);
                                    assert_eq!(l.element_type, LeafType::EventId);
                                }
                                _ => panic!("Expected eventid leaf"),
                            }
                        }
                        _ => panic!("Expected instance group"),
                    }
                }
            }
            _ => panic!("Expected wrapper group"),
        }

        // Second wrapper: Producer events (elem:1)
        match &tree.segments[0].children[1] {
            ConfigNode::Group(wrapper) => {
                assert_eq!(wrapper.instance, 0);
                assert_eq!(wrapper.instance_label, "Event");
                assert_eq!(wrapper.description.as_deref(), Some("Producer events"));
                assert_eq!(wrapper.replication_count, 6);
                assert_eq!(wrapper.path, vec!["seg:0".to_string(), "elem:1".to_string()]);

                // 6 instance children
                assert_eq!(wrapper.children.len(), 6);

                for (i, child) in wrapper.children.iter().enumerate() {
                    match child {
                        ConfigNode::Group(g) => {
                            assert_eq!(g.instance, (i + 1) as u32);
                            assert_eq!(g.instance_label, format!("Event {}", i + 1));
                            // Instances don't repeat the description (only wrapper has it)
                            assert_eq!(g.description, None);
                            // Path: seg:0 / elem:1 / inst:N
                            assert_eq!(
                                g.path,
                                vec![
                                    "seg:0".to_string(),
                                    "elem:1".to_string(),
                                    format!("inst:{}", i + 1)
                                ]
                            );
                            // Address continues from 48: 48 + i*8
                            match &g.children[0] {
                                ConfigNode::Leaf(l) => {
                                    assert_eq!(l.address, (48 + i * 8) as u32);
                                }
                                _ => panic!("Expected leaf"),
                            }
                        }
                        _ => panic!("Expected instance group"),
                    }
                }
            }
            _ => panic!("Expected wrapper group"),
        }
    }

    #[test]
    fn nested_replicated_groups() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group replication="2">
                        <name>Port</name>
                        <repname>Port</repname>
                        <group replication="3">
                            <name>Channel</name>
                            <repname>Ch</repname>
                            <int size="1"><name>Value</name></int>
                        </group>
                    </group>
                </segment>
            </cdi>"#,
        );

        // With wrapper structure: 1 Port wrapper at top level
        assert_eq!(tree.segments[0].children.len(), 1);

        match &tree.segments[0].children[0] {
            ConfigNode::Group(port_wrapper) => {
                assert_eq!(port_wrapper.instance, 0);
                assert_eq!(port_wrapper.instance_label, "Port");
                assert_eq!(port_wrapper.replication_count, 2);

                // Port wrapper contains 2 Port instances
                assert_eq!(port_wrapper.children.len(), 2);

                for (port_idx, port_child) in port_wrapper.children.iter().enumerate() {
                    match port_child {
                        ConfigNode::Group(port) => {
                            assert_eq!(port.instance, (port_idx + 1) as u32);
                            assert_eq!(port.instance_label, format!("Port {}", port_idx + 1));

                            // Each Port instance has 1 Channel wrapper
                            assert_eq!(port.children.len(), 1);

                            match &port.children[0] {
                                ConfigNode::Group(ch_wrapper) => {
                                    assert_eq!(ch_wrapper.instance, 0);
                                    assert_eq!(ch_wrapper.instance_label, "Channel");
                                    assert_eq!(ch_wrapper.replication_count, 3);

                                    // Channel wrapper contains 3 Channel instances
                                    assert_eq!(ch_wrapper.children.len(), 3);

                                    for (ch_idx, ch_child) in ch_wrapper.children.iter().enumerate()
                                    {
                                        match ch_child {
                                            ConfigNode::Group(ch) => {
                                                assert_eq!(ch.instance, (ch_idx + 1) as u32);
                                                assert_eq!(
                                                    ch.instance_label,
                                                    format!("Ch {}", ch_idx + 1)
                                                );
                                                // Address: port_idx * 3 + ch_idx (each int is 1 byte)
                                                match &ch.children[0] {
                                                    ConfigNode::Leaf(l) => {
                                                        let expected =
                                                            (port_idx * 3 + ch_idx) as u32;
                                                        assert_eq!(l.address, expected);
                                                    }
                                                    _ => panic!("Expected leaf"),
                                                }
                                            }
                                            _ => panic!("Expected channel instance"),
                                        }
                                    }
                                }
                                _ => panic!("Expected channel wrapper"),
                            }
                        }
                        _ => panic!("Expected port instance"),
                    }
                }
            }
            _ => panic!("Expected port wrapper"),
        }
    }

    // ── Merge config values ─────────────────────────────────────────────

    #[test]
    fn merge_int_value() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="2"><name>Speed</name></int>
                </segment>
            </cdi>"#,
        );

        let mut values = HashMap::new();
        values.insert(0u32, vec![0x00, 0x42]); // 66 in big-endian u16
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::Int { value }) => assert_eq!(*value, 66),
                other => panic!("Expected Int value, got {:?}", other),
            },
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_eventid_value() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Test Event</name></eventid>
                </segment>
            </cdi>"#,
        );

        let mut values = HashMap::new();
        values.insert(0u32, vec![0x05, 0x02, 0x01, 0x02, 0x03, 0x00, 0x00, 0x01]);
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::EventId { hex, bytes }) => {
                    assert_eq!(hex, "05.02.01.02.03.00.00.01");
                    assert_eq!(bytes[0], 0x05);
                }
                other => panic!("Expected EventId value, got {:?}", other),
            },
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_string_value() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <string size="16"><name>Label</name></string>
                </segment>
            </cdi>"#,
        );

        let mut values = HashMap::new();
        let mut raw = b"Hello\0\0\0\0\0\0\0\0\0\0\0".to_vec();
        raw.truncate(16);
        values.insert(0u32, raw);
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::String { value }) => assert_eq!(value, "Hello"),
                other => panic!("Expected String value, got {:?}", other),
            },
            _ => panic!("Expected leaf"),
        }
    }

    // ── Merge event roles ───────────────────────────────────────────────

    #[test]
    fn merge_roles() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt A</name></eventid>
                    <eventid><name>Evt B</name></eventid>
                </segment>
            </cdi>"#,
        );

        let mut roles = HashMap::new();
        roles.insert(
            "seg:0/elem:0".to_string(),
            EventRole::Producer,
        );
        roles.insert(
            "seg:0/elem:1".to_string(),
            EventRole::Consumer,
        );
        merge_event_roles(&mut tree, &roles);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert_eq!(l.event_role, Some(EventRole::Producer)),
            _ => panic!("Expected leaf"),
        }
        match &tree.segments[0].children[1] {
            ConfigNode::Leaf(l) => assert_eq!(l.event_role, Some(EventRole::Consumer)),
            _ => panic!("Expected leaf"),
        }
    }

    // ── collect_event_id_leaves ─────────────────────────────────────────

    #[test]
    fn collect_event_leaves() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Setting</name></int>
                    <eventid><name>Evt A</name></eventid>
                    <group replication="2">
                        <name>Pair</name>
                        <eventid><name>Evt B</name></eventid>
                    </group>
                </segment>
            </cdi>"#,
        );

        let leaves = collect_event_id_leaves(&tree);
        // 1 top-level eventid + 2 from replicated group = 3
        assert_eq!(leaves.len(), 3);
        assert_eq!(leaves[0].name, "Evt A");
        assert_eq!(leaves[1].name, "Evt B"); // instance 1
        assert_eq!(leaves[2].name, "Evt B"); // instance 2
    }

    // ── count_leaves ────────────────────────────────────────────────────

    #[test]
    fn count_all_leaves() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>A</name></int>
                    <int size="1"><name>B</name></int>
                    <group replication="3">
                        <name>Ch</name>
                        <int size="1"><name>Val</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );

        // 2 top-level ints + 3 ints (one per replication) = 5
        assert_eq!(count_leaves(&tree), 5);
    }

    // ── Address computation matches legacy ──────────────────────────────

    #[test]
    fn address_with_offsets() {
        // Tests that offset skips are handled correctly
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="100">
                    <name>Config</name>
                    <int size="1"><name>A</name></int>
                    <int size="2" offset="5"><name>B</name></int>
                    <eventid offset="3"><name>C</name></eventid>
                </segment>
            </cdi>"#,
        );

        let seg = &tree.segments[0];
        // A: origin(100) + cursor(0) = 100
        match &seg.children[0] {
            ConfigNode::Leaf(l) => assert_eq!(l.address, 100),
            _ => panic!("Expected leaf"),
        }
        // B: cursor after A = 1, skip 5 → cursor=6, addr=100+6=106
        match &seg.children[1] {
            ConfigNode::Leaf(l) => assert_eq!(l.address, 106),
            _ => panic!("Expected leaf"),
        }
        // C: cursor after B = 6+2=8, skip 3 → cursor=11, addr=100+11=111
        match &seg.children[2] {
            ConfigNode::Leaf(l) => assert_eq!(l.address, 111),
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn group_with_offset_skip() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="4"><name>Header</name></int>
                    <group offset="10" replication="2">
                        <name>Pair</name>
                        <int size="2"><name>Val</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );

        // Header: addr=0, size=4, cursor after=4
        // Group offset=10: cursor = 4+10 = 14, group_start = 14
        // Instance 0: base=14, Val addr=14
        // Instance 1: base=14+2=16, Val addr=16

        // With wrapper structure: children[1] is the wrapper
        match &tree.segments[0].children[1] {
            ConfigNode::Group(wrapper) => {
                assert_eq!(wrapper.instance, 0);
                assert_eq!(wrapper.instance_label, "Pair");
                assert_eq!(wrapper.replication_count, 2);

                // Two instances inside the wrapper
                assert_eq!(wrapper.children.len(), 2);

                match &wrapper.children[0] {
                    ConfigNode::Group(g) => {
                        assert_eq!(g.instance, 1);
                        match &g.children[0] {
                            ConfigNode::Leaf(l) => assert_eq!(l.address, 14),
                            _ => panic!("Expected leaf"),
                        }
                    }
                    _ => panic!("Expected instance 1"),
                }
                match &wrapper.children[1] {
                    ConfigNode::Group(g) => {
                        assert_eq!(g.instance, 2);
                        match &g.children[0] {
                            ConfigNode::Leaf(l) => assert_eq!(l.address, 16),
                            _ => panic!("Expected leaf"),
                        }
                    }
                    _ => panic!("Expected instance 2"),
                }
            }
            _ => panic!("Expected wrapper group"),
        }
    }

    #[test]
    fn int_constraints_preserved() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1">
                        <name>Setting</name>
                        <min>0</min>
                        <max>100</max>
                        <default>50</default>
                        <map>
                            <relation><property>0</property><value>Off</value></relation>
                            <relation><property>1</property><value>On</value></relation>
                        </map>
                    </int>
                </segment>
            </cdi>"#,
        );

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                let c = l.constraints.as_ref().unwrap();
                assert_eq!(c.min, Some(0));
                assert_eq!(c.max, Some(100));
                assert_eq!(c.default_value.as_deref(), Some("50"));
                let entries = c.map_entries.as_ref().unwrap();
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].label, "Off");
                assert_eq!(entries[1].label, "On");
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn identity_preserved() {
        let tree = tree_from_xml(
            r#"<cdi>
                <identification>
                    <manufacturer>ACME</manufacturer>
                    <model>Widget</model>
                    <hardwareVersion>1.0</hardwareVersion>
                    <softwareVersion>2.0</softwareVersion>
                </identification>
                <segment space="253" origin="0">
                    <name>Config</name>
                </segment>
            </cdi>"#,
        );

        let id = tree.identity.as_ref().unwrap();
        assert_eq!(id.manufacturer.as_deref(), Some("ACME"));
        assert_eq!(id.model.as_deref(), Some("Widget"));
        assert_eq!(id.hardware_version.as_deref(), Some("1.0"));
        assert_eq!(id.software_version.as_deref(), Some("2.0"));
    }
}
