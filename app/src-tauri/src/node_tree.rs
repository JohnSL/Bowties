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
use std::collections::{BTreeMap, HashMap};

use lcc_rs::cdi::{
    Cdi, DataElement, EventRole, Identification, SliderHints,
};
use crate::layout::node_snapshot::SnapshotValueNode;

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
    /// Optional connector daughterboard profile for supported modular boards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_profile: Option<ConnectorProfile>,
    /// Optional warning when connector filtering is disabled for safety.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_profile_warning: Option<String>,
    /// Top-level segments mirroring CDI `<segment>` elements
    pub segments: Vec<SegmentNode>,
}

/// Profile-authored connector-slot metadata attached to a node tree payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorProfile {
    pub node_id: String,
    pub carrier_key: String,
    pub slots: Vec<ConnectorSlot>,
    pub supported_daughterboards: Vec<SupportedDaughterboard>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConnectorScalarValue {
    String(String),
    Integer(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectorConstraintEffect {
    Show,
    Hide,
    Disable,
    AllowValues,
    DenyValues,
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmptyConnectorConstraintEffect {
    Hide,
    Disable,
    AllowValues,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConstraint {
    pub target_path: String,
    pub resolved_path: Vec<String>,
    pub effect: ConnectorConstraintEffect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_ordinals: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_values: Vec<ConnectorScalarValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_values: Vec<ConnectorScalarValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyConnectorBehavior {
    pub effect: EmptyConnectorConstraintEffect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_values: Vec<ConnectorScalarValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotSupportedDaughterboard {
    pub daughterboard_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validity_rules: Vec<ConnectorConstraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSlot {
    pub slot_id: String,
    pub label: String,
    pub order: u32,
    pub allow_none_installed: bool,
    pub supported_daughterboard_ids: Vec<String>,
    pub affected_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_affected_paths: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_behavior_when_empty: Option<EmptyConnectorBehavior>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_daughterboard_constraints: Vec<SlotSupportedDaughterboard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedDaughterboard {
    pub daughterboard_id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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

fn default_true() -> bool { true }

/// A (possibly replicated) group of child nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    /// Display name for this group instance
    pub name: String,
    /// True when the CDI group had an explicit <name> element.
    /// When false the UI should suppress the group header.
    #[serde(default = "default_true")]
    pub has_name: bool,
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
    #[serde(default)]
    pub display_name: Option<String>,
    /// When true the group can be collapsed/expanded by the user
    #[serde(default)]
    pub hideable: bool,
    /// When true the group starts collapsed
    #[serde(default)]
    pub hidden_by_default: bool,
    /// When true all child fields are read-only (Write button disabled)
    #[serde(default)]
    pub read_only: bool,
}

/// Write lifecycle state for a pending modification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WriteState {
    /// User has set a modified value that differs from the on-node value.
    Dirty,
    /// A write to the node is in progress.
    Writing,
    /// The last write attempt failed.
    Error,
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
    /// Action trigger: label for the button (action elements only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button_text: Option<String>,
    /// Action trigger: confirmation dialog text (action elements only; None = no dialog)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialog_text: Option<String>,
    /// Action trigger: value to write on trigger (action elements only)
    #[serde(default)]
    pub action_value: i64,
    /// Slider hint (int elements with slider hint only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint_slider: Option<SliderHints>,
    /// Radio-button hint (int elements with radiobutton hint only)
    #[serde(default)]
    pub hint_radio: bool,
    /// User-modified value not yet written to the node.
    /// When `Some`, this is the value the user intends to write.
    /// `value` retains the last-confirmed on-node value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_value: Option<ConfigValue>,
    /// Write lifecycle state.  `None` when no modification is pending.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_state: Option<WriteState>,
    /// Error message from the last failed write attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_error: Option<String>,
    /// Set to true at runtime when the device rejects a write with a read-only
    /// error (0x1083).  Disables the control for the rest of the session.
    #[serde(default)]
    pub read_only: bool,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl ConfigValue {
    /// Convert to the string representation used in layout node snapshots.
    pub fn to_snapshot_string(&self) -> String {
        match self {
            ConfigValue::Int { value } => value.to_string(),
            ConfigValue::String { value } => value.clone(),
            ConfigValue::EventId { hex, .. } => hex.clone(),
            ConfigValue::Float { value } => value.to_string(),
        }
    }
}

/// Optional constraints on a leaf element.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeafConstraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
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
        connector_profile: None,
        connector_profile_warning: None,
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

                let has_name = g.name.is_some();
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

                        // Use CDI-walker-compatible format: "elem:I#N"
                        let mut child_path = parent_path.to_vec();
                        child_path.push(format!("elem:{}#{}", i, inst_num));

                        let instance_base = group_start + instance as i32 * stride;
                        let instance_label = g.compute_repname(instance);

                        let child_nodes =
                            build_children(&g.elements, instance_base, space, &child_path);

                        replicated_children.push(ConfigNode::Group(GroupNode {
                            name: original_name.clone(),
                            has_name,
                            description: None, // Instances don't repeat the description
                            instance: inst_num,
                            instance_label,
                            replication_of: original_name.clone(),
                            replication_count: effective_replication,
                            path: child_path,
                            children: child_nodes,
                            display_name: None,
                            hideable: g.hints.as_ref().map(|h| h.hideable).unwrap_or(false),
                            hidden_by_default: g.hints.as_ref().map(|h| h.hidden).unwrap_or(false),
                            read_only: g.hints.as_ref().map(|h| h.read_only).unwrap_or(false),
                        }));
                    }

                    // Wrapper group: instance=0 indicates it's the template/wrapper.
                    children.push(ConfigNode::Group(GroupNode {
                        name: original_name.clone(),
                        has_name,
                        description: g.description.clone(),
                        instance: 0,
                        instance_label: original_name.clone(), // Wrapper uses the group name
                        replication_of: original_name.clone(),
                        replication_count: effective_replication,
                        path: group_path,
                        children: replicated_children,
                        display_name: None,
                        hideable: g.hints.as_ref().map(|h| h.hideable).unwrap_or(false),
                        hidden_by_default: g.hints.as_ref().map(|h| h.hidden).unwrap_or(false),
                        read_only: g.hints.as_ref().map(|h| h.read_only).unwrap_or(false),
                    }));
                } else {
                    // Non-replicated group (replication=1): no wrapper needed.
                    let instance_base = group_start;
                    let child_nodes =
                        build_children(&g.elements, instance_base, space, &group_path);

                    children.push(ConfigNode::Group(GroupNode {
                        name: original_name.clone(),
                        has_name,
                        description: g.description.clone(),
                        instance: 1,
                        instance_label: original_name.clone(),
                        replication_of: original_name.clone(),
                        replication_count: 1,
                        path: group_path,
                        children: child_nodes,
                        display_name: None,
                        hideable: g.hints.as_ref().map(|h| h.hideable).unwrap_or(false),
                        hidden_by_default: g.hints.as_ref().map(|h| h.hidden).unwrap_or(false),
                        read_only: g.hints.as_ref().map(|h| h.read_only).unwrap_or(false),
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
                    value: e.default.map(|d| ConfigValue::Int { value: d }),
                    event_role: None,
                    constraints: Some(LeafConstraints {
                        min: e.min.map(|v| v as f64),
                        max: e.max.map(|v| v as f64),
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
                    button_text: None,
                    dialog_text: None,
                    action_value: 0,
                    hint_slider: e.hints.as_ref().and_then(|h| h.slider.clone()),
                    hint_radio: e.hints.as_ref().map(|h| h.radiobutton).unwrap_or(false),
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
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
                    button_text: None,
                    dialog_text: None,
                    action_value: 0,
                    hint_slider: None,
                    hint_radio: false,
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
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
                    button_text: None,
                    dialog_text: None,
                    action_value: 0,
                    hint_slider: None,
                    hint_radio: false,
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
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
                    size: e.size as u32,
                    space,
                    path,
                    value: e.default.map(|d| ConfigValue::Float { value: d }),
                    event_role: None,
                    constraints: Some(LeafConstraints {
                        min: e.min,
                        max: e.max,
                        default_value: e.default.map(|v| v.to_string()),
                        map_entries: None,
                    }),
                    button_text: None,
                    dialog_text: None,
                    action_value: 0,
                    hint_slider: None,
                    hint_radio: false,
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
                }));
                cursor += e.size as i32;
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
                    size: e.size as u32,
                    space,
                    path,
                    value: None,
                    event_role: None,
                    constraints: None,
                    button_text: e.button_text.clone(),
                    dialog_text: e.dialog_text.clone(),
                    action_value: e.value,
                    hint_slider: None,
                    hint_radio: false,
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
                }));
                cursor += e.size as i32;
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
                    button_text: None,
                    dialog_text: None,
                    action_value: 0,
                    hint_slider: None,
                    hint_radio: false,
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
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

/// Decode an IEEE 754 half-precision (float16) bit pattern to f64.
fn f16_bits_to_f64(bits: u16) -> f64 {
    let sign = ((bits as u32) >> 15) << 31;
    let exp = ((bits >> 10) & 0x1F) as u32;
    let mantissa = (bits & 0x3FF) as u32;
    let f32_bits: u32 = if exp == 0 {
        if mantissa == 0 {
            sign // ±zero
        } else {
            // Denormal: normalize
            let mut m = mantissa;
            let mut e = 0u32;
            while (m & 0x400) == 0 { m <<= 1; e += 1; }
            sign | ((127 - 14 + 1 - e) << 23) | ((m & 0x3FF) << 13)
        }
    } else if exp == 31 {
        sign | 0x7F80_0000 | (mantissa << 13) // Inf/NaN
    } else {
        sign | ((exp + 112) << 23) | (mantissa << 13)
    };
    f32::from_bits(f32_bits) as f64
}

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
///
/// NOTE: This legacy helper ignores address space and can collide when
/// multiple spaces use the same offset. Prefer `merge_config_values_by_space`
/// for live config reads.
pub fn merge_config_values(tree: &mut NodeConfigTree, values: &HashMap<u32, Vec<u8>>) {
    for segment in &mut tree.segments {
        merge_children_values(&mut segment.children, values);
    }
}

/// Merge configuration values into a tree using `(space, address)` keys.
///
/// This avoids collisions when different CDI segments share the same absolute
/// offset in different memory spaces (e.g. LT-50 status space 1 vs macros
/// space 20 both using address 0x00000000).
pub fn merge_config_values_by_space(
    tree: &mut NodeConfigTree,
    values: &HashMap<(u8, u32), Vec<u8>>,
) {
    for segment in &mut tree.segments {
        merge_children_values_by_space(&mut segment.children, values);
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

fn merge_children_values_by_space(children: &mut [ConfigNode], values: &HashMap<(u8, u32), Vec<u8>>) {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                merge_children_values_by_space(&mut g.children, values);
            }
            ConfigNode::Leaf(leaf) => {
                if let Some(raw) = values.get(&(leaf.space, leaf.address)) {
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
            match size {
                2 if raw.len() >= 2 => {
                    let bits = u16::from_be_bytes([raw[0], raw[1]]);
                    Some(ConfigValue::Float { value: f16_bits_to_f64(bits) })
                }
                4 if raw.len() >= 4 => {
                    let val = f32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
                    Some(ConfigValue::Float { value: val as f64 })
                }
                8 if raw.len() >= 8 => {
                    let val = f64::from_be_bytes([
                        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                    ]);
                    Some(ConfigValue::Float { value: val })
                }
                _ => None,
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

/// Merge string-encoded snapshot values (from a saved layout YAML) into an
/// existing tree.
///
/// `values` uses the same format written by the capture pipeline:
/// `space_number → "0xOFFSET" → string-encoded value`.
/// Each string is re-parsed into the appropriate `ConfigValue` variant based
/// on the leaf's `element_type`.  Leaves with no matching entry are left
/// with whatever default value `build_node_config_tree` assigned.
/// Parse a string-encoded snapshot value into a typed `ConfigValue`.
fn parse_snapshot_value(leaf_type: LeafType, s: &str) -> Option<ConfigValue> {
    match leaf_type {
        LeafType::Int => s
            .parse::<i64>()
            .ok()
            .map(|value| ConfigValue::Int { value }),
        LeafType::String => Some(ConfigValue::String {
            value: s.to_owned(),
        }),
        LeafType::EventId => {
            // Stored as dotted hex: "05.02.01.02.03.04.05.06"
            let parts: Vec<&str> = s.split('.').collect();
            if parts.len() != 8 {
                return None;
            }
            let mut bytes = [0u8; 8];
            for (i, part) in parts.iter().enumerate() {
                bytes[i] = u8::from_str_radix(part, 16).ok()?;
            }
            let hex = bytes_to_dotted_hex(&bytes);
            Some(ConfigValue::EventId { bytes, hex })
        }
        LeafType::Float => s
            .parse::<f64>()
            .ok()
            .map(|value| ConfigValue::Float { value }),
        LeafType::Action | LeafType::Blob => None,
    }
}

/// Merge path-centric snapshot values (new layout format) into an existing tree.
fn canonical_offset_key(offset: &str) -> String {
    let trimmed = offset.trim();
    let hex = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed)
        .to_uppercase();
    format!("0x{}", hex)
}

///
/// Values are matched by exact CDI path first, then by space+offset metadata.
pub fn merge_snapshot_path_values(
    tree: &mut NodeConfigTree,
    config: &BTreeMap<String, SnapshotValueNode>,
) {
    let mut path_values: HashMap<String, String> = HashMap::new();
    let mut addr_values: HashMap<(u8, String), String> = HashMap::new();
    flatten_snapshot_config(config, &mut Vec::new(), &mut path_values, &mut addr_values);

    for segment in &mut tree.segments {
        merge_children_snapshot_path(&mut segment.children, &path_values, &addr_values);
    }
}

fn flatten_snapshot_config(
    root: &BTreeMap<String, SnapshotValueNode>,
    path: &mut Vec<String>,
    path_values: &mut HashMap<String, String>,
    addr_values: &mut HashMap<(u8, String), String>,
) {
    for (key, node) in root {
        path.push(key.clone());
        match node {
            SnapshotValueNode::Branch(children) => {
                flatten_snapshot_config(children, path, path_values, addr_values);
            }
            SnapshotValueNode::Leaf(leaf) => {
                path_values.insert(path.join("/"), leaf.value.clone());
                if let (Some(space), Some(offset)) = (leaf.space, leaf.offset.clone()) {
                    addr_values.insert((space, canonical_offset_key(&offset)), leaf.value.clone());
                }
            }
        }
        let _ = path.pop();
    }
}

fn merge_children_snapshot_path(
    children: &mut [ConfigNode],
    path_values: &HashMap<String, String>,
    addr_values: &HashMap<(u8, String), String>,
) {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                merge_children_snapshot_path(&mut g.children, path_values, addr_values);
            }
            ConfigNode::Leaf(leaf) => {
                let path_key = leaf.path.join("/");
                if let Some(s) = path_values.get(&path_key) {
                    leaf.value = parse_snapshot_value(leaf.element_type, s);
                    continue;
                }
                let offset_key = canonical_offset_key(&format!("0x{:08X}", leaf.address));
                if let Some(s) = addr_values.get(&(leaf.space, offset_key)) {
                    leaf.value = parse_snapshot_value(leaf.element_type, s);
                }
            }
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
                let value = match effective_value(leaf) {
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
// Modified value operations
// ─────────────────────────────────────────────────────────────────────────────

/// Set a modified value on a leaf identified by `(space, address)`.
///
/// If `new_value` equals the leaf's committed `value`, the modification is
/// automatically cleared (the user reverted to the original).
/// Returns `true` if the leaf was found.
pub fn set_modified_value(
    tree: &mut NodeConfigTree,
    space: u8,
    address: u32,
    new_value: ConfigValue,
) -> bool {
    for segment in &mut tree.segments {
        if set_modified_in_children(&mut segment.children, space, address, &new_value) {
            return true;
        }
    }
    false
}

fn set_modified_in_children(
    children: &mut [ConfigNode],
    space: u8,
    address: u32,
    new_value: &ConfigValue,
) -> bool {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                if set_modified_in_children(&mut g.children, space, address, new_value) {
                    return true;
                }
            }
            ConfigNode::Leaf(leaf) if leaf.space == space && leaf.address == address => {
                // Auto-revert: if the new value equals the committed value, clear the edit.
                if leaf.value.as_ref() == Some(new_value) {
                    leaf.modified_value = None;
                    leaf.write_state = None;
                    leaf.write_error = None;
                } else {
                    leaf.modified_value = Some(new_value.clone());
                    leaf.write_state = Some(WriteState::Dirty);
                    leaf.write_error = None;
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Clear all modified values in the tree (discard all pending edits).
pub fn discard_all_modified(tree: &mut NodeConfigTree) {
    for segment in &mut tree.segments {
        discard_in_children(&mut segment.children);
    }
}

fn discard_in_children(children: &mut [ConfigNode]) {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => discard_in_children(&mut g.children),
            ConfigNode::Leaf(leaf) => {
                leaf.modified_value = None;
                leaf.write_state = None;
                leaf.write_error = None;
            }
        }
    }
}

/// Returns `true` if any leaf in the tree has a pending modification.
pub fn has_modified_values(tree: &NodeConfigTree) -> bool {
    tree.segments
        .iter()
        .any(|s| has_modified_in_children(&s.children))
}

fn has_modified_in_children(children: &[ConfigNode]) -> bool {
    children.iter().any(|c| match c {
        ConfigNode::Group(g) => has_modified_in_children(&g.children),
        ConfigNode::Leaf(leaf) => leaf.modified_value.is_some(),
    })
}

/// Info about a leaf with a pending modification, used by the write pipeline.
#[derive(Debug, Clone)]
pub struct ModifiedLeafInfo {
    pub address: u32,
    pub space: u8,
    pub size: u32,
    pub element_type: LeafType,
    pub path: Vec<String>,
    pub name: String,
    pub value: ConfigValue,
}

/// Collect all leaves that have pending modifications (write_state == Dirty or Error).
pub fn collect_modified_leaves(tree: &NodeConfigTree) -> Vec<ModifiedLeafInfo> {
    let mut results = Vec::new();
    for segment in &tree.segments {
        collect_modified_recursive(&segment.children, &mut results);
    }
    results
}

fn collect_modified_recursive(children: &[ConfigNode], results: &mut Vec<ModifiedLeafInfo>) {
    for child in children {
        match child {
            ConfigNode::Group(g) => collect_modified_recursive(&g.children, results),
            ConfigNode::Leaf(leaf) => {
                if let Some(ref modified) = leaf.modified_value {
                    match leaf.write_state {
                        Some(WriteState::Dirty) | Some(WriteState::Error) => {
                            results.push(ModifiedLeafInfo {
                                address: leaf.address,
                                space: leaf.space,
                                size: leaf.size,
                                element_type: leaf.element_type,
                                path: leaf.path.clone(),
                                name: leaf.name.clone(),
                                value: modified.clone(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Update the write_state for a specific leaf identified by (space, address).
pub fn set_leaf_write_state(
    tree: &mut NodeConfigTree,
    space: u8,
    address: u32,
    state: WriteState,
    error: Option<String>,
) -> bool {
    for segment in &mut tree.segments {
        if set_write_state_in_children(&mut segment.children, space, address, state, &error) {
            return true;
        }
    }
    false
}

fn set_write_state_in_children(
    children: &mut [ConfigNode],
    space: u8,
    address: u32,
    state: WriteState,
    error: &Option<String>,
) -> bool {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                if set_write_state_in_children(&mut g.children, space, address, state, error) {
                    return true;
                }
            }
            ConfigNode::Leaf(leaf) if leaf.space == space && leaf.address == address => {
                leaf.write_state = Some(state);
                leaf.write_error = error.clone();
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Promote a leaf's `modified_value` to `value` after a successful write,
/// clearing the modification state.
pub fn commit_leaf_value(
    tree: &mut NodeConfigTree,
    space: u8,
    address: u32,
) -> bool {
    for segment in &mut tree.segments {
        if commit_in_children(&mut segment.children, space, address) {
            return true;
        }
    }
    false
}

fn commit_in_children(children: &mut [ConfigNode], space: u8, address: u32) -> bool {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                if commit_in_children(&mut g.children, space, address) {
                    return true;
                }
            }
            ConfigNode::Leaf(leaf) if leaf.space == space && leaf.address == address => {
                if let Some(modified) = leaf.modified_value.take() {
                    leaf.value = Some(modified);
                }
                leaf.write_state = None;
                leaf.write_error = None;
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Revert a leaf's pending modification and mark it as permanently read-only.
///
/// Called when the device rejects a write with error 0x1083 (address is read-only).
/// Clears `modified_value`, `write_state`, and `write_error`, and sets
/// `read_only = true` so the control is disabled for the rest of the session.
pub fn revert_and_mark_leaf_read_only(
    tree: &mut NodeConfigTree,
    space: u8,
    address: u32,
) -> bool {
    for segment in &mut tree.segments {
        if revert_read_only_in_children(&mut segment.children, space, address) {
            return true;
        }
    }
    false
}

fn revert_read_only_in_children(
    children: &mut [ConfigNode],
    space: u8,
    address: u32,
) -> bool {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                if revert_read_only_in_children(&mut g.children, space, address) {
                    return true;
                }
            }
            ConfigNode::Leaf(leaf) if leaf.space == space && leaf.address == address => {
                leaf.modified_value = None;
                leaf.write_state = None;
                leaf.write_error = None;
                leaf.read_only = true;
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Return the "effective" value for a leaf: `modified_value` if present, else `value`.
pub fn effective_value(leaf: &LeafNode) -> Option<&ConfigValue> {
    leaf.modified_value.as_ref().or(leaf.value.as_ref())
}

/// Directly set a leaf's committed `value` (without going through modified_value).
///
/// Used by sync apply to reflect a successfully written value in the tree.
pub fn update_leaf_value(
    tree: &mut NodeConfigTree,
    space: u8,
    address: u32,
    value: ConfigValue,
) -> bool {
    for segment in &mut tree.segments {
        if update_leaf_in_children(&mut segment.children, space, address, &value) {
            return true;
        }
    }
    false
}

fn update_leaf_in_children(children: &mut [ConfigNode], space: u8, address: u32, value: &ConfigValue) -> bool {
    for child in children.iter_mut() {
        match child {
            ConfigNode::Group(g) => {
                if update_leaf_in_children(&mut g.children, space, address, value) {
                    return true;
                }
            }
            ConfigNode::Leaf(leaf) if leaf.space == space && leaf.address == address => {
                leaf.value = Some(value.clone());
                leaf.modified_value = None;
                leaf.write_state = None;
                leaf.write_error = None;
                return true;
            }
            _ => {}
        }
    }
    false
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

                            // Path: seg:0 / elem:0#N (CDI-walker-compatible)
                            assert_eq!(
                                g.path,
                                vec![
                                    "seg:0".to_string(),
                                    format!("elem:0#{}", idx + 1)
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
                            // Path: seg:0 / elem:0#N (CDI-walker-compatible)
                            assert_eq!(
                                g.path,
                                vec![
                                    "seg:0".to_string(),
                                    format!("elem:0#{}", i + 1)
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
                            // Path: seg:0 / elem:1#N (CDI-walker-compatible)
                            assert_eq!(
                                g.path,
                                vec![
                                    "seg:0".to_string(),
                                    format!("elem:1#{}", i + 1)
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
                assert_eq!(c.min, Some(0.0_f64));
                assert_eq!(c.max, Some(100.0_f64));
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

    // ── parse_leaf_value ────────────────────────────────────────────────────

    #[test]
    fn parse_leaf_value_int_1byte() {
        let result = parse_leaf_value(LeafType::Int, 1, &[0xAB]);
        match result {
            Some(ConfigValue::Int { value }) => assert_eq!(value, 0xAB),
            other => panic!("Expected Int, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_int_2byte_signed() {
        // 0xFF, 0xFE = -2 as big-endian i16
        let result = parse_leaf_value(LeafType::Int, 2, &[0xFF, 0xFE]);
        match result {
            Some(ConfigValue::Int { value }) => assert_eq!(value, -2_i64),
            other => panic!("Expected Int, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_int_4byte() {
        let result = parse_leaf_value(LeafType::Int, 4, &[0x00, 0x00, 0x03, 0xE8]);
        match result {
            Some(ConfigValue::Int { value }) => assert_eq!(value, 1000),
            other => panic!("Expected Int, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_int_8byte_signed() {
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let result = parse_leaf_value(LeafType::Int, 8, &bytes);
        match result {
            Some(ConfigValue::Int { value }) => assert_eq!(value, -1_i64),
            other => panic!("Expected Int, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_int_short_data_returns_none() {
        // size=2 but only 1 byte of data → None
        let result = parse_leaf_value(LeafType::Int, 2, &[0xAA]);
        assert!(result.is_none(), "Expected None for insufficient data");
    }

    #[test]
    fn parse_leaf_value_string_nul_termination() {
        let raw = b"Hello\0Extra\0".to_vec();
        let result = parse_leaf_value(LeafType::String, 12, &raw);
        match result {
            Some(ConfigValue::String { value }) => assert_eq!(value, "Hello"),
            other => panic!("Expected String, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_string_ff_pruning() {
        // 0xFF bytes (flash-uninitialized) must be filtered out before NUL-stop
        let raw = vec![b'H', b'i', 0xFF, b'C', 0x00];
        let result = parse_leaf_value(LeafType::String, 5, &raw);
        match result {
            Some(ConfigValue::String { value }) => assert_eq!(value, "HiC"),
            other => panic!("Expected String, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_event_id_8byte() {
        let raw: Vec<u8> = vec![0x05, 0x02, 0x01, 0x02, 0x03, 0x00, 0x00, 0x01];
        let result = parse_leaf_value(LeafType::EventId, 8, &raw);
        match result {
            Some(ConfigValue::EventId { bytes, hex }) => {
                assert_eq!(bytes, [0x05, 0x02, 0x01, 0x02, 0x03, 0x00, 0x00, 0x01]);
                assert_eq!(hex, "05.02.01.02.03.00.00.01");
            }
            other => panic!("Expected EventId, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_event_id_too_short_returns_none() {
        let result = parse_leaf_value(LeafType::EventId, 8, &[0x01, 0x02, 0x03]);
        assert!(result.is_none());
    }

    #[test]
    fn parse_leaf_value_float() {
        // 1.0f32 in big-endian bytes = [0x3F, 0x80, 0x00, 0x00]
        let raw: Vec<u8> = vec![0x3F, 0x80, 0x00, 0x00];
        let result = parse_leaf_value(LeafType::Float, 4, &raw);
        match result {
            Some(ConfigValue::Float { value }) => {
                assert!((value - 1.0_f64).abs() < 1e-6, "Expected 1.0, got {}", value);
            }
            other => panic!("Expected Float, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_action_returns_none() {
        let result = parse_leaf_value(LeafType::Action, 1, &[0x01]);
        assert!(result.is_none(), "Action should always return None");
    }

    #[test]
    fn parse_leaf_value_blob_returns_none() {
        let result = parse_leaf_value(LeafType::Blob, 4, &[0x01, 0x02, 0x03, 0x04]);
        assert!(result.is_none(), "Blob should always return None");
    }

    #[test]
    fn parse_leaf_value_float_f16() {
        // 0x3C00 = 1.0 in IEEE 754 half-precision (big-endian)
        let raw: Vec<u8> = vec![0x3C, 0x00];
        let result = parse_leaf_value(LeafType::Float, 2, &raw);
        match result {
            Some(ConfigValue::Float { value }) => {
                assert!((value - 1.0_f64).abs() < 1e-3, "Expected ≈1.0, got {}", value);
            }
            other => panic!("Expected Float, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_float_f16_zero() {
        let raw: Vec<u8> = vec![0x00, 0x00];
        let result = parse_leaf_value(LeafType::Float, 2, &raw);
        match result {
            Some(ConfigValue::Float { value }) => assert_eq!(value, 0.0_f64),
            other => panic!("Expected Float, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_float_f64() {
        // 1.5f64 in big-endian
        let raw = 1.5f64.to_be_bytes().to_vec();
        let result = parse_leaf_value(LeafType::Float, 8, &raw);
        match result {
            Some(ConfigValue::Float { value }) => {
                assert!((value - 1.5_f64).abs() < f64::EPSILON, "Expected 1.5, got {}", value);
            }
            other => panic!("Expected Float, got {:?}", other),
        }
    }

    #[test]
    fn parse_leaf_value_float_insufficient_data_returns_none() {
        // size=4 but only 2 bytes → None
        let result = parse_leaf_value(LeafType::Float, 4, &[0x3F, 0x80]);
        assert!(result.is_none(), "Insufficient data should return None");
    }

    // ── classify_leaf_roles_from_protocol ───────────────────────────────────

    #[test]
    fn classify_leaf_roles_producer_only() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt</name></eventid>
                </segment>
            </cdi>"#,
        );
        let event_bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let mut values = HashMap::new();
        values.insert(0u32, event_bytes.to_vec());
        merge_config_values(&mut tree, &values);

        let mut event_roles_map = HashMap::new();
        event_roles_map.insert(event_bytes, crate::state::NodeRoles {
            producers: ["05.02.01.02.03.00".to_string()].iter().cloned().collect(),
            consumers: std::collections::HashSet::new(),
        });
        let result = classify_leaf_roles_from_protocol(&tree, &event_roles_map);
        assert_eq!(result.get("seg:0/elem:0"), Some(&EventRole::Producer));
    }

    #[test]
    fn classify_leaf_roles_consumer_only() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt</name></eventid>
                </segment>
            </cdi>"#,
        );
        let event_bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x09];
        let mut values = HashMap::new();
        values.insert(0u32, event_bytes.to_vec());
        merge_config_values(&mut tree, &values);

        let mut event_roles_map = HashMap::new();
        event_roles_map.insert(event_bytes, crate::state::NodeRoles {
            producers: std::collections::HashSet::new(),
            consumers: ["05.02.01.02.03.00".to_string()].iter().cloned().collect(),
        });
        let result = classify_leaf_roles_from_protocol(&tree, &event_roles_map);
        assert_eq!(result.get("seg:0/elem:0"), Some(&EventRole::Consumer));
    }

    #[test]
    fn classify_leaf_roles_both_is_ambiguous() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt</name></eventid>
                </segment>
            </cdi>"#,
        );
        let event_bytes = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11];
        let mut values = HashMap::new();
        values.insert(0u32, event_bytes.to_vec());
        merge_config_values(&mut tree, &values);

        let node_id_str = "05.02.01.02.03.00".to_string();
        let mut event_roles_map = HashMap::new();
        event_roles_map.insert(event_bytes, crate::state::NodeRoles {
            producers: [node_id_str.clone()].iter().cloned().collect(),
            consumers: [node_id_str].iter().cloned().collect(),
        });
        let result = classify_leaf_roles_from_protocol(&tree, &event_roles_map);
        assert_eq!(result.get("seg:0/elem:0"), Some(&EventRole::Ambiguous));
    }

    #[test]
    fn classify_leaf_roles_uninvolved_not_included() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt</name></eventid>
                </segment>
            </cdi>"#,
        );
        let event_bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let mut values = HashMap::new();
        values.insert(0u32, event_bytes.to_vec());
        merge_config_values(&mut tree, &values);

        // A third-party node is the producer; this tree's node is uninvolved
        let mut event_roles_map = HashMap::new();
        event_roles_map.insert(event_bytes, crate::state::NodeRoles {
            producers: ["99.99.99.99.99.01".to_string()].iter().cloned().collect(),
            consumers: std::collections::HashSet::new(),
        });
        let result = classify_leaf_roles_from_protocol(&tree, &event_roles_map);
        assert!(result.is_empty(), "Uninvolved node must produce no result");
    }

    #[test]
    fn classify_leaf_roles_no_value_is_skipped() {
        // EventId leaf has no value — leaf should be skipped entirely
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt</name></eventid>
                </segment>
            </cdi>"#,
        );
        let event_bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        // No merge_config_values — leaf.value is None
        let mut event_roles_map = HashMap::new();
        event_roles_map.insert(event_bytes, crate::state::NodeRoles {
            producers: ["05.02.01.02.03.00".to_string()].iter().cloned().collect(),
            consumers: std::collections::HashSet::new(),
        });
        let result = classify_leaf_roles_from_protocol(&tree, &event_roles_map);
        assert!(result.is_empty(), "Leaf without value must be skipped");
    }

    // ── merge_config_values extensions ────────────────────────────────────────

    #[test]
    fn merge_config_values_4byte_int() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="4"><name>Val</name></int>
                </segment>
            </cdi>"#,
        );
        let mut values = HashMap::new();
        values.insert(0u32, vec![0x00, 0x01, 0x86, 0xA0]); // 100000
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::Int { value }) => assert_eq!(*value, 100000),
                other => panic!("Expected Int, got {:?}", other),
            },
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_config_values_8byte_int() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="8"><name>Val</name></int>
                </segment>
            </cdi>"#,
        );
        let val_bytes = 1_000_000_000_000i64.to_be_bytes();
        let mut values = HashMap::new();
        values.insert(0u32, val_bytes.to_vec());
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::Int { value }) => assert_eq!(*value, 1_000_000_000_000),
                other => panic!("Expected Int, got {:?}", other),
            },
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_config_values_float() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <float><name>Temp</name></float>
                </segment>
            </cdi>"#,
        );
        let raw = std::f32::consts::PI.to_be_bytes();
        let mut values = HashMap::new();
        values.insert(0u32, raw.to_vec());
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::Float { value }) => {
                    assert!((*value as f32 - std::f32::consts::PI).abs() < 1e-5);
                }
                other => panic!("Expected Float, got {:?}", other),
            },
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_config_values_string_with_embedded_ff() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <string size="8"><name>Label</name></string>
                </segment>
            </cdi>"#,
        );
        // 0xFF bytes (flash-uninitialised) are stripped; NUL terminates
        let raw = vec![b'A', b'B', 0xFF, b'C', 0x00, 0x00, 0x00, 0x00];
        let mut values = HashMap::new();
        values.insert(0u32, raw);
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::String { value }) => assert_eq!(value, "ABC"),
                other => panic!("Expected String, got {:?}", other),
            },
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_config_values_address_not_found_stays_none() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="10">
                    <name>Config</name>
                    <int size="1"><name>A</name></int>
                </segment>
            </cdi>"#,
        );
        let mut values = HashMap::new();
        values.insert(20u32, vec![0x42]); // Wrong address
        merge_config_values(&mut tree, &values);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert!(l.value.is_none(), "Value should remain None"),
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_config_values_in_replicated_group_instance() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group replication="3">
                        <name>Ch</name>
                        <int size="1"><name>Val</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );
        // Instance addresses: 0, 1, 2 (stride = 1 byte)
        let mut values = HashMap::new();
        values.insert(0u32, vec![0x10]);
        values.insert(1u32, vec![0x20]);
        values.insert(2u32, vec![0x30]);
        merge_config_values(&mut tree, &values);

        let expected = [0x10i64, 0x20, 0x30];
        match &tree.segments[0].children[0] {
            ConfigNode::Group(wrapper) => {
                for (i, instance) in wrapper.children.iter().enumerate() {
                    match instance {
                        ConfigNode::Group(g) => match &g.children[0] {
                            ConfigNode::Leaf(l) => match &l.value {
                                Some(ConfigValue::Int { value }) => {
                                    assert_eq!(*value, expected[i]);
                                }
                                other => panic!("Instance {}: expected Int, got {:?}", i, other),
                            },
                            _ => panic!("Expected leaf at instance {}", i),
                        },
                        _ => panic!("Expected instance group"),
                    }
                }
            }
            _ => panic!("Expected wrapper group"),
        }
    }

    #[test]
    fn lt50_status_trace_bytes_decode_to_non_zero_ints() {
        // From LT-50 trace reply at space=1, address=0x00000000:
        // 20.50.00.00.00.00.01.00.00.3B.EF.00.00.00.12...
        let track_voltage = parse_leaf_value(LeafType::Int, 4, &[0x00, 0x00, 0x3B, 0xEF]);
        let track_current = parse_leaf_value(LeafType::Int, 4, &[0x00, 0x00, 0x00, 0x12]);

        match track_voltage {
            Some(ConfigValue::Int { value }) => assert_eq!(value, 15_343),
            other => panic!("Expected non-zero Int for track voltage, got {:?}", other),
        }

        match track_current {
            Some(ConfigValue::Int { value }) => assert_eq!(value, 18),
            other => panic!("Expected non-zero Int for track current, got {:?}", other),
        }
    }

    #[test]
    fn merge_config_values_by_space_avoids_lt50_address_collision() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="1" origin="0">
                    <name>Status</name>
                    <int size="4"><name>Track Voltage</name></int>
                    <int size="4"><name>Track Current</name></int>
                </segment>
                <segment space="20" origin="0">
                    <name>Macros</name>
                    <int size="4"><name>Startup Macro</name></int>
                    <int size="4"><name>Macro Name Prefix</name></int>
                </segment>
            </cdi>"#,
        );

        // LT-50 status bytes from trace (space=1) are non-zero.
        // Space=20 has zeros at the same offsets in this capture.
        // By-space merge must keep each segment's value isolated.
        let mut values = HashMap::new();
        values.insert((1u8, 0u32), vec![0x00, 0x00, 0x3B, 0xEF]); // Track Voltage = 15343
        values.insert((1u8, 4u32), vec![0x00, 0x00, 0x00, 0x12]); // Track Current = 18
        values.insert((20u8, 0u32), vec![0x00, 0x00, 0x00, 0x00]);
        values.insert((20u8, 4u32), vec![0x00, 0x00, 0x00, 0x00]);

        merge_config_values_by_space(&mut tree, &values);

        let status = &tree.segments[0];
        let voltage = match &status.children[0] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::Int { value }) => *value,
                other => panic!("Expected Int for voltage, got {:?}", other),
            },
            _ => panic!("Expected voltage leaf"),
        };
        let current = match &status.children[1] {
            ConfigNode::Leaf(l) => match &l.value {
                Some(ConfigValue::Int { value }) => *value,
                other => panic!("Expected Int for current, got {:?}", other),
            },
            _ => panic!("Expected current leaf"),
        };

        assert_eq!(voltage, 15_343);
        assert_eq!(current, 18);
    }

    // ── merge_event_roles extensions ──────────────────────────────────────────

    #[test]
    fn merge_event_roles_in_group() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group>
                        <name>Output</name>
                        <eventid><name>Trigger</name></eventid>
                    </group>
                </segment>
            </cdi>"#,
        );
        let mut roles = HashMap::new();
        roles.insert("seg:0/elem:0/elem:0".to_string(), EventRole::Producer);
        merge_event_roles(&mut tree, &roles);

        match &tree.segments[0].children[0] {
            ConfigNode::Group(g) => match &g.children[0] {
                ConfigNode::Leaf(l) => assert_eq!(l.event_role, Some(EventRole::Producer)),
                _ => panic!("Expected leaf"),
            },
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn merge_event_roles_int_leaf_unchanged() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Setting</name></int>
                </segment>
            </cdi>"#,
        );
        let mut roles = HashMap::new();
        // Path matches an Int leaf — must be silently skipped (only EventId leaves get roles)
        roles.insert("seg:0/elem:0".to_string(), EventRole::Producer);
        merge_event_roles(&mut tree, &roles);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Int);
                assert!(l.event_role.is_none(), "Int leaf must not receive an event_role");
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_event_roles_nonmatching_path_is_noop() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt</name></eventid>
                </segment>
            </cdi>"#,
        );
        let mut roles = HashMap::new();
        roles.insert("seg:0/elem:99".to_string(), EventRole::Consumer);
        merge_event_roles(&mut tree, &roles);

        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert!(l.event_role.is_none(), "Unmatched path must not set event_role");
            }
            _ => panic!("Expected leaf"),
        }
    }

    // ── collect_event_id_leaves extensions ────────────────────────────────────

    #[test]
    fn collect_event_leaves_with_populated_value() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <eventid><name>Evt</name></eventid>
                </segment>
            </cdi>"#,
        );
        let event_bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let mut values = HashMap::new();
        values.insert(0u32, event_bytes.to_vec());
        merge_config_values(&mut tree, &values);

        let leaves = collect_event_id_leaves(&tree);
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].value, Some(event_bytes));
    }

    #[test]
    fn collect_event_leaves_deeply_nested() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group>
                        <name>Outer</name>
                        <group>
                            <name>Inner</name>
                            <eventid><name>Deep Event</name></eventid>
                        </group>
                    </group>
                </segment>
            </cdi>"#,
        );
        let leaves = collect_event_id_leaves(&tree);
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].name, "Deep Event");
    }

    // ── build_node_config_tree extensions ─────────────────────────────────────

    #[test]
    fn build_tree_string_leaf_size_preserved() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <string size="32"><name>Label</name></string>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::String);
                assert_eq!(l.size, 32);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_float_leaf() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <float><name>Temperature</name></float>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Float);
                assert_eq!(l.size, 4);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_action_leaf_size_one() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <action><name>Reset</name></action>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Action);
                assert_eq!(l.size, 1);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_blob_leaf() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <blob size="64"><name>Raw Data</name></blob>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Blob);
                assert_eq!(l.size, 64);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_negative_offset_arithmetic() {
        // Negative offsets are valid per CDI spec; cursor arithmetic must use i32.
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="200">
                    <name>Config</name>
                    <int size="4"><name>Header</name></int>
                    <int size="1" offset="-2"><name>Overlap</name></int>
                </segment>
            </cdi>"#,
        );
        // Header: cursor=0, addr=200, size=4 → cursor=4
        // Overlap: cursor=4+(-2)=2, addr=200+2=202
        let seg = &tree.segments[0];
        match &seg.children[0] {
            ConfigNode::Leaf(l) => assert_eq!(l.address, 200, "Header should be at 200"),
            _ => panic!("Expected leaf"),
        }
        match &seg.children[1] {
            ConfigNode::Leaf(l) => assert_eq!(l.address, 202, "Overlap should be at 202"),
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_map_constraints_survive() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1">
                        <name>Mode</name>
                        <map>
                            <relation><property>0</property><value>Off</value></relation>
                            <relation><property>1</property><value>On</value></relation>
                            <relation><property>2</property><value>Auto</value></relation>
                        </map>
                    </int>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                let entries = l.constraints.as_ref().unwrap().map_entries.as_ref().unwrap();
                assert_eq!(entries.len(), 3);
                assert_eq!(entries[0].label, "Off");
                assert_eq!(entries[1].label, "On");
                assert_eq!(entries[2].label, "Auto");
            }
            _ => panic!("Expected leaf"),
        }
    }

    // ── Float size variants ─────────────────────────────────────────────────

    #[test]
    fn build_tree_float_size2() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <float size="2"><name>Half</name></float>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Float);
                assert_eq!(l.size, 2);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_float_size8() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <float size="8"><name>Double</name></float>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Float);
                assert_eq!(l.size, 8);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_float_default_prepopulated() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <float size="4">
                        <name>Gain</name>
                        <default>3.14</default>
                    </float>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                match &l.value {
                    Some(ConfigValue::Float { value }) => {
                        assert!((value - 3.14f64).abs() < 1e-6, "Expected ≈3.14, got {}", value);
                    }
                    other => panic!("Expected ConfigValue::Float, got {:?}", other),
                }
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_float_with_min_max_constraints() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <float size="4">
                        <name>Vol</name>
                        <min>-1.0</min>
                        <max>1.0</max>
                    </float>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                let c = l.constraints.as_ref().unwrap();
                assert!((c.min.unwrap() - (-1.0f64)).abs() < f64::EPSILON);
                assert!((c.max.unwrap() - 1.0f64).abs() < f64::EPSILON);
            }
            _ => panic!("Expected leaf"),
        }
    }

    // ── Int default pre-population ──────────────────────────────────────────

    #[test]
    fn build_tree_int_default_prepopulated() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1">
                        <name>Speed</name>
                        <default>42</default>
                    </int>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                match &l.value {
                    Some(ConfigValue::Int { value }) => assert_eq!(*value, 42),
                    other => panic!("Expected ConfigValue::Int, got {:?}", other),
                }
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_int_no_default_is_none() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Mode</name></int>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert!(l.value.is_none(), "No default → value should be None"),
            _ => panic!("Expected leaf"),
        }
    }

    // ── Integer hints ───────────────────────────────────────────────────────

    #[test]
    fn build_tree_int_with_slider_hint() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1">
                        <name>Brightness</name>
                        <hints><slider immediate="1" tickSpacing="5" showValue="1"/></hints>
                    </int>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                let slider = l.hint_slider.as_ref().expect("Expected hint_slider");
                assert!(slider.immediate);
                assert_eq!(slider.tick_spacing, 5);
                assert!(slider.show_value);
                assert!(!l.hint_radio, "hint_radio should be false");
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_int_with_radio_hint() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1">
                        <name>Mode</name>
                        <hints><radiobutton/></hints>
                    </int>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert!(l.hint_radio, "hint_radio should be true");
                assert!(l.hint_slider.is_none(), "hint_slider should be None");
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_int_no_hint_fields_are_default() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>X</name></int>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert!(l.hint_slider.is_none());
                assert!(!l.hint_radio);
            }
            _ => panic!("Expected leaf"),
        }
    }

    // ── Action with button_text / dialog_text ───────────────────────────────

    #[test]
    fn build_tree_action_with_button_and_dialog_text() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <action size="1">
                        <name>Factory Reset</name>
                        <buttonText>Reset Now</buttonText>
                        <dialogText>Are you sure?</dialogText>
                        <value>255</value>
                    </action>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Action);
                assert_eq!(l.button_text.as_deref(), Some("Reset Now"));
                assert_eq!(l.dialog_text.as_deref(), Some("Are you sure?"));
                assert_eq!(l.action_value, 255);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_action_custom_size() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <action size="4"><name>Trigger</name></action>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.element_type, LeafType::Action);
                assert_eq!(l.size, 4);
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn build_tree_action_cursor_advances_by_size() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <action size="4"><name>Cmd</name></action>
                    <int size="1"><name>After</name></int>
                </segment>
            </cdi>"#,
        );
        let seg = &tree.segments[0];
        // Action at address=0, size=4 → next element starts at 4
        match &seg.children[1] {
            ConfigNode::Leaf(l) => assert_eq!(l.address, 4, "Int after 4-byte action should be at address 4"),
            _ => panic!("Expected leaf"),
        }
    }

    // ── Group hints ─────────────────────────────────────────────────────────

    #[test]
    fn build_tree_group_hideable() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group>
                        <name>Advanced</name>
                        <hints><visibility hideable="1" hidden="1"/></hints>
                        <int size="1"><name>X</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Group(g) => {
                assert!(g.hideable, "hideable should be true");
                assert!(g.hidden_by_default, "hidden_by_default should be true");
                assert!(!g.read_only);
            }
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn build_tree_group_readonly() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group>
                        <name>Status</name>
                        <hints><readOnly/></hints>
                        <int size="1"><name>Y</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Group(g) => {
                assert!(g.read_only, "read_only should be true");
                assert!(!g.hideable);
                assert!(!g.hidden_by_default);
            }
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn build_tree_group_no_hints_defaults_false() {
        let tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group>
                        <name>Plain</name>
                        <int size="1"><name>Z</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );
        match &tree.segments[0].children[0] {
            ConfigNode::Group(g) => {
                assert!(!g.hideable);
                assert!(!g.hidden_by_default);
                assert!(!g.read_only);
            }
            _ => panic!("Expected group"),
        }
    }

    // ── ConfigValue::to_snapshot_string ──────────────────────────────────

    #[test]
    fn config_value_to_snapshot_string_int() {
        let v = ConfigValue::Int { value: 42 };
        assert_eq!(v.to_snapshot_string(), "42");
    }

    #[test]
    fn config_value_to_snapshot_string_string() {
        let v = ConfigValue::String { value: "hello world".to_string() };
        assert_eq!(v.to_snapshot_string(), "hello world");
    }

    #[test]
    fn config_value_to_snapshot_string_event_id() {
        let v = ConfigValue::EventId {
            bytes: [0x05, 0x02, 0x01, 0x02, 0x03, 0x00, 0x00, 0x01],
            hex: "05.02.01.02.03.00.00.01".to_string(),
        };
        assert_eq!(v.to_snapshot_string(), "05.02.01.02.03.00.00.01");
    }

    #[test]
    fn config_value_to_snapshot_string_float() {
        let v = ConfigValue::Float { value: 3.14 };
        assert_eq!(v.to_snapshot_string(), "3.14");
    }

    // ── set_modified_value tests ────────────────────────────────────────

    #[test]
    fn set_modified_value_marks_leaf_dirty() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Speed</name></int>
                </segment>
            </cdi>"#,
        );
        let result = set_modified_value(
            &mut tree,
            253,
            0,
            ConfigValue::Int { value: 42 },
        );
        assert!(result, "should find the leaf");
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(
                    l.modified_value,
                    Some(ConfigValue::Int { value: 42 })
                );
                assert_eq!(l.write_state, Some(WriteState::Dirty));
                assert!(l.write_error.is_none());
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn set_modified_value_autorevert_when_same_as_committed() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Speed</name><default>10</default></int>
                </segment>
            </cdi>"#,
        );
        // First set a different value to make it dirty
        set_modified_value(&mut tree, 253, 0, ConfigValue::Int { value: 42 });
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert!(l.modified_value.is_some()),
            _ => panic!("Expected leaf"),
        }
        // Now set it back to the committed value — should auto-revert
        set_modified_value(&mut tree, 253, 0, ConfigValue::Int { value: 10 });
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert!(l.modified_value.is_none(), "should auto-revert");
                assert!(l.write_state.is_none(), "write_state cleared on auto-revert");
                assert!(l.write_error.is_none());
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn set_modified_value_returns_false_for_missing_leaf() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Speed</name></int>
                </segment>
            </cdi>"#,
        );
        // Wrong address — leaf is at 0, not 999
        let result = set_modified_value(
            &mut tree,
            253,
            999,
            ConfigValue::Int { value: 1 },
        );
        assert!(!result, "should not find leaf at wrong address");
    }

    #[test]
    fn set_modified_value_wrong_space_returns_false() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Speed</name></int>
                </segment>
            </cdi>"#,
        );
        // Right address but wrong space
        let result = set_modified_value(
            &mut tree,
            251,
            0,
            ConfigValue::Int { value: 1 },
        );
        assert!(!result, "should not find leaf in wrong space");
    }

    #[test]
    fn set_modified_value_finds_nested_leaf_in_group() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <group>
                        <name>Settings</name>
                        <int size="2"><name>Volume</name></int>
                    </group>
                </segment>
            </cdi>"#,
        );
        let result = set_modified_value(
            &mut tree,
            253,
            0,
            ConfigValue::Int { value: 100 },
        );
        assert!(result, "should find nested leaf");
        // Navigate into the group
        match &tree.segments[0].children[0] {
            ConfigNode::Group(g) => match &g.children[0] {
                ConfigNode::Leaf(l) => {
                    assert_eq!(l.modified_value, Some(ConfigValue::Int { value: 100 }));
                }
                _ => panic!("Expected leaf inside group"),
            },
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn set_modified_value_targets_correct_leaf_among_siblings() {
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>First</name></int>
                    <int size="1"><name>Second</name></int>
                    <int size="1"><name>Third</name></int>
                </segment>
            </cdi>"#,
        );
        // Modify the second leaf (address=1, since size of first is 1)
        let result = set_modified_value(
            &mut tree,
            253,
            1,
            ConfigValue::Int { value: 77 },
        );
        assert!(result);
        // First leaf should be untouched
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert!(l.modified_value.is_none(), "first leaf unchanged"),
            _ => panic!("Expected leaf"),
        }
        // Second leaf should be modified
        match &tree.segments[0].children[1] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.modified_value, Some(ConfigValue::Int { value: 77 }));
            }
            _ => panic!("Expected leaf"),
        }
        // Third leaf should be untouched
        match &tree.segments[0].children[2] {
            ConfigNode::Leaf(l) => assert!(l.modified_value.is_none(), "third leaf unchanged"),
            _ => panic!("Expected leaf"),
        }
    }

    // ── merge_snapshot_path_values tests ─────────────────────────────────

    #[test]
    fn merge_snapshot_populates_empty_tree_from_path_values() {
        use crate::layout::node_snapshot::{SnapshotLeafValue, SnapshotValueNode};

        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <string size="20"><name>Name</name></string>
                    <int size="1"><name>Mode</name></int>
                </segment>
            </cdi>"#,
        );
        // Verify tree starts without values
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert!(l.value.is_none()),
            _ => panic!("Expected leaf"),
        }
        match &tree.segments[0].children[1] {
            ConfigNode::Leaf(l) => assert!(l.value.is_none()),
            _ => panic!("Expected leaf"),
        }

        // Build snapshot config matching the tree paths
        let mut config = BTreeMap::new();
        config.insert(
            "seg:0".to_string(),
            SnapshotValueNode::Branch({
                let mut seg = BTreeMap::new();
                seg.insert(
                    "elem:0".to_string(),
                    SnapshotValueNode::Leaf(SnapshotLeafValue {
                        value: "My Node".to_string(),
                        space: Some(253),
                        offset: Some("0x00000000".to_string()),
                    }),
                );
                seg.insert(
                    "elem:1".to_string(),
                    SnapshotValueNode::Leaf(SnapshotLeafValue {
                        value: "5".to_string(),
                        space: Some(253),
                        offset: Some("0x00000014".to_string()),
                    }),
                );
                seg
            }),
        );

        merge_snapshot_path_values(&mut tree, &config);

        // Verify values are now populated
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.value, Some(ConfigValue::String { value: "My Node".to_string() }));
            }
            _ => panic!("Expected leaf"),
        }
        match &tree.segments[0].children[1] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.value, Some(ConfigValue::Int { value: 5 }));
            }
            _ => panic!("Expected leaf"),
        }
    }

    #[test]
    fn merge_snapshot_preserves_existing_values() {
        use crate::layout::node_snapshot::{SnapshotLeafValue, SnapshotValueNode};

        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Config</name>
                    <int size="1"><name>Speed</name><default>10</default></int>
                    <int size="1"><name>Mode</name></int>
                </segment>
            </cdi>"#,
        );
        // Speed has default=10 (pre-populated), Mode has no value
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert_eq!(l.value, Some(ConfigValue::Int { value: 10 })),
            _ => panic!("Expected leaf"),
        }

        // Snapshot only provides Mode (no entry for Speed)
        let mut config = BTreeMap::new();
        config.insert(
            "seg:0".to_string(),
            SnapshotValueNode::Branch({
                let mut seg = BTreeMap::new();
                seg.insert(
                    "elem:1".to_string(),
                    SnapshotValueNode::Leaf(SnapshotLeafValue {
                        value: "3".to_string(),
                        space: Some(253),
                        offset: Some("0x00000001".to_string()),
                    }),
                );
                seg
            }),
        );

        merge_snapshot_path_values(&mut tree, &config);

        // Speed should keep its default
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => assert_eq!(l.value, Some(ConfigValue::Int { value: 10 })),
            _ => panic!("Expected leaf"),
        }
        // Mode should now have the snapshot value
        match &tree.segments[0].children[1] {
            ConfigNode::Leaf(l) => assert_eq!(l.value, Some(ConfigValue::Int { value: 3 })),
            _ => panic!("Expected leaf"),
        }
    }

    /// This is the regression test for Issue 1: a CDI-built tree without
    /// snapshot values shows empty strings where the node actually has data.
    /// After merging snapshot values, set_modified_value should still work
    /// and the other leaves should retain their snapshot values.
    #[test]
    fn set_modified_on_snapshot_merged_tree_preserves_other_values() {
        use crate::layout::node_snapshot::{SnapshotLeafValue, SnapshotValueNode};

        // Build a bare CDI tree (no config values — simulates proxy fallback)
        let mut tree = tree_from_xml(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Segment A</name>
                    <string size="20"><name>Name</name></string>
                    <int size="1"><name>Speed</name></int>
                </segment>
                <segment space="253" origin="100">
                    <name>Segment B</name>
                    <int size="2"><name>Volume</name></int>
                    <string size="32"><name>Label</name></string>
                </segment>
            </cdi>"#,
        );

        // Merge layout snapshot values into the bare tree
        let mut config = BTreeMap::new();
        config.insert(
            "seg:0".to_string(),
            SnapshotValueNode::Branch({
                let mut seg = BTreeMap::new();
                seg.insert(
                    "elem:0".to_string(),
                    SnapshotValueNode::Leaf(SnapshotLeafValue {
                        value: "My Node".to_string(),
                        space: Some(253),
                        offset: Some("0x00000000".to_string()),
                    }),
                );
                seg.insert(
                    "elem:1".to_string(),
                    SnapshotValueNode::Leaf(SnapshotLeafValue {
                        value: "8".to_string(),
                        space: Some(253),
                        offset: Some("0x00000014".to_string()),
                    }),
                );
                seg
            }),
        );
        config.insert(
            "seg:1".to_string(),
            SnapshotValueNode::Branch({
                let mut seg = BTreeMap::new();
                seg.insert(
                    "elem:0".to_string(),
                    SnapshotValueNode::Leaf(SnapshotLeafValue {
                        value: "500".to_string(),
                        space: Some(253),
                        offset: Some("0x00000064".to_string()),
                    }),
                );
                seg.insert(
                    "elem:1".to_string(),
                    SnapshotValueNode::Leaf(SnapshotLeafValue {
                        value: "Main Speaker".to_string(),
                        space: Some(253),
                        offset: Some("0x00000066".to_string()),
                    }),
                );
                seg
            }),
        );

        merge_snapshot_path_values(&mut tree, &config);

        // Modify a leaf in Segment A
        let result = set_modified_value(
            &mut tree,
            253,
            0,
            ConfigValue::String { value: "New Name".to_string() },
        );
        assert!(result);

        // Segment A: Name should have modified_value, Speed should keep snapshot value
        match &tree.segments[0].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(
                    l.modified_value,
                    Some(ConfigValue::String { value: "New Name".to_string() })
                );
                // Original snapshot value still present
                assert_eq!(l.value, Some(ConfigValue::String { value: "My Node".to_string() }));
            }
            _ => panic!("Expected leaf"),
        }
        match &tree.segments[0].children[1] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.value, Some(ConfigValue::Int { value: 8 }));
                assert!(l.modified_value.is_none(), "Speed should be untouched");
            }
            _ => panic!("Expected leaf"),
        }

        // Segment B: both leaves should retain snapshot values
        match &tree.segments[1].children[0] {
            ConfigNode::Leaf(l) => {
                assert_eq!(l.value, Some(ConfigValue::Int { value: 500 }));
                assert!(l.modified_value.is_none(), "Volume should be untouched");
            }
            _ => panic!("Expected leaf"),
        }
        match &tree.segments[1].children[1] {
            ConfigNode::Leaf(l) => {
                assert_eq!(
                    l.value,
                    Some(ConfigValue::String { value: "Main Speaker".to_string() })
                );
                assert!(l.modified_value.is_none(), "Label should be untouched");
            }
            _ => panic!("Expected leaf"),
        }
    }
}
