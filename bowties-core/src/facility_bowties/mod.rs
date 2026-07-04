//! Facility bowtie composition (Spec 018 / S6 — D2, D6).
//!
//! Given a Wired facility (all slots at their `min_channels`), this module
//! computes the set of [`CompositionOp`]s the frontend must dispatch to
//! bring the facility's behaviour to life on the bus. Each `CompositionOp`
//! is a single "write this event ID onto that consumer leaf + register a
//! bowtie" pair.
//!
//! D6 — LCC producer-identifies / consumer-subscribes: the composed bowties
//! adopt the producer channel's existing event IDs, and only the consumer
//! channel's CDI leaves are re-written. The producer's leaves are left
//! alone; nothing writes fresh event IDs during composition.
//!
//! D2 — this module owns the mapping so the "two bowties per Block
//! Indicator, event IDs adopted from the producer, name derived from the
//! state mapping" contract is unit-testable at the deepest layer. The
//! frontend orchestrator dispatches the ops via the existing
//! `configEditor.applyEdit` + `bowtieMetadataStore.createBowtie` seams.
//!
//! T13 ownership resolution: `BowtieMetadata::created_by_facility` is the
//! only back-reference persisted per bowtie. Teardown does NOT persist a
//! separate leaf back-reference; it re-invokes this composer on the
//! still-Wired facility shape and reuses the returned ops' leaves to
//! locate the consumer fields it must overwrite with fresh event IDs.
//! Trades one extra IPC round-trip for schema locality (option (ii) of
//! the S6 T13 alternatives — see the slice card).

use std::collections::HashMap;

use crate::behavior_templates::{BehaviorTemplate, SlotKind};
use crate::channel_events::resolve_lamp_row_path_prefix;
use crate::layout::channels::{ChannelBinding, ChannelRole, InformationChannel};
use crate::layout::facilities::Facility;
use crate::node_tree::{ConfigNode, LeafNode, LeafType, NodeConfigTree};
use lcc_rs::cdi::EventRole;

/// One composed edit + bowtie registration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionOp {
    /// `NodeKey` of the consumer channel that owns the target leaf.
    pub consumer_node_key: String,
    /// CDI path of the consumer leaf to overwrite with the producer's event ID.
    pub consumer_leaf_path: Vec<String>,
    /// Memory-space number of the consumer leaf (mirrors `LeafNode.space`).
    pub consumer_leaf_space: u8,
    /// Absolute address of the consumer leaf (mirrors `LeafNode.address`).
    pub consumer_leaf_address: u32,
    /// Producer's event-ID bytes — adopted verbatim (D6).
    pub event_id_bytes: [u8; 8],
    /// Bowtie name to register (`"{facility.name} — {consumer_command}"`).
    pub bowtie_name: String,
    /// Back-reference echoed onto every `BowtieMetadata` this op creates.
    pub created_by_facility: String,
}

/// Errors [`compose_bowtie_ops`] may return.
#[derive(Debug, Clone, PartialEq)]
pub enum FacilityCompositionError {
    /// A slot has fewer than its `min_channels` bindings — facility is not Wired.
    NotWired { slot_label: String },
    /// The producer channel's `state → event_id` map does not contain the
    /// state name declared by the template's mapping.
    MissingProducerEventId {
        channel_id: String,
        state_name: String,
    },
    /// The consumer channel's CDI does not surface a leaf for the requested
    /// consumer command (e.g. `lit` / `unlit`).
    MissingConsumerLeaf {
        channel_id: String,
        command_name: String,
    },
    /// A slot references a channel id that is not in the channel inventory.
    UnknownChannel {
        slot_label: String,
        channel_id: String,
    },
    /// A slot's bound channel carries the wrong role for its declared slot kind.
    RoleMismatch {
        slot_label: String,
        expected_role: &'static str,
        actual_role: ChannelRole,
    },
    /// The facility's template declares no producer or no consumer slot.
    /// Block Indicator has one of each; guard is defence-in-depth.
    NoProducerSlot,
    NoConsumerSlot,
}

impl std::fmt::Display for FacilityCompositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotWired { slot_label } => {
                write!(f, "facility slot '{slot_label}' is not at min_channels")
            }
            Self::MissingProducerEventId {
                channel_id,
                state_name,
            } => write!(
                f,
                "producer channel '{channel_id}' has no event ID for state '{state_name}'",
            ),
            Self::MissingConsumerLeaf {
                channel_id,
                command_name,
            } => write!(
                f,
                "consumer channel '{channel_id}' has no leaf for command '{command_name}'",
            ),
            Self::UnknownChannel {
                slot_label,
                channel_id,
            } => write!(
                f,
                "slot '{slot_label}' references unknown channel '{channel_id}'",
            ),
            Self::RoleMismatch {
                slot_label,
                expected_role,
                actual_role,
            } => write!(
                f,
                "slot '{slot_label}' expects role '{expected_role}' but channel has {actual_role:?}",
            ),
            Self::NoProducerSlot => write!(f, "template declares no producer slot"),
            Self::NoConsumerSlot => write!(f, "template declares no consumer slot"),
        }
    }
}

impl std::error::Error for FacilityCompositionError {}

/// Consumer-side event-leaf mapping supplied by the caller. Keyed by
/// consumer command name (e.g. `"lit"`, `"unlit"`); the value is the
/// 0-based ordinal within the consumer-role leaves under the lamp row's
/// CDI path prefix.
pub type ConsumerLeafIndex = HashMap<String, u32>;

/// Producer-side event-id map supplied by the caller. Keyed by producer
/// state name (e.g. `"occupied"`, `"clear"`); the value is the 8-byte
/// event ID currently written on the producer's CDI leaf.
pub type ProducerEventIds = HashMap<String, [u8; 8]>;

/// Compose the [`CompositionOp`]s for a Wired facility.
///
/// # Inputs
///
/// * `facility` — the facility whose slots are all at `min_channels`.
/// * `template` — the facility's behaviour template (state mappings +
///   slot roles).
/// * `channels` — channel inventory (must contain every channel id in
///   the facility's slot bindings).
/// * `producer_event_ids` — one map per producer channel id → producer
///   state → 8-byte event id (already resolved from the producer's CDI
///   by the caller).
/// * `per_node_cdi` — per-node CDI trees, keyed by `NodeKey`. Used to
///   locate consumer leaves via [`resolve_lamp_row_path_prefix`].
/// * `consumer_leaf_index` — the consumer style's `command → leaf ordinal`
///   map (e.g. `single-led-direct-lamp` maps `lit → 0, unlit → 1`).
pub fn compose_bowtie_ops(
    facility: &Facility,
    template: &BehaviorTemplate,
    channels: &[InformationChannel],
    producer_event_ids: &HashMap<String, ProducerEventIds>,
    per_node_cdi: &HashMap<String, NodeConfigTree>,
    consumer_leaf_index: &ConsumerLeafIndex,
) -> Result<Vec<CompositionOp>, FacilityCompositionError> {
    // ── Wired guard: every slot must be at its min_channels ──────────
    for slot in template.slots {
        let bound = facility
            .slot_bindings
            .get(slot.label)
            .map(|v| v.len())
            .unwrap_or(0);
        if bound < slot.min_channels as usize {
            return Err(FacilityCompositionError::NotWired {
                slot_label: slot.label.to_string(),
            });
        }
    }

    let producer_slot = template
        .slots
        .iter()
        .find(|s| s.kind == SlotKind::Producer)
        .ok_or(FacilityCompositionError::NoProducerSlot)?;
    let consumer_slot = template
        .slots
        .iter()
        .find(|s| s.kind == SlotKind::Consumer)
        .ok_or(FacilityCompositionError::NoConsumerSlot)?;

    let producer_channel_id = facility
        .slot_bindings
        .get(producer_slot.label)
        .and_then(|v| v.first())
        .ok_or(FacilityCompositionError::NotWired {
            slot_label: producer_slot.label.to_string(),
        })?;
    let consumer_channel_id = facility
        .slot_bindings
        .get(consumer_slot.label)
        .and_then(|v| v.first())
        .ok_or(FacilityCompositionError::NotWired {
            slot_label: consumer_slot.label.to_string(),
        })?;

    let producer_channel = find_channel(channels, producer_channel_id).ok_or_else(|| {
        FacilityCompositionError::UnknownChannel {
            slot_label: producer_slot.label.to_string(),
            channel_id: producer_channel_id.clone(),
        }
    })?;
    let consumer_channel = find_channel(channels, consumer_channel_id).ok_or_else(|| {
        FacilityCompositionError::UnknownChannel {
            slot_label: consumer_slot.label.to_string(),
            channel_id: consumer_channel_id.clone(),
        }
    })?;

    if producer_channel.role != ChannelRole::BlockOccupancy {
        return Err(FacilityCompositionError::RoleMismatch {
            slot_label: producer_slot.label.to_string(),
            expected_role: producer_slot.required_role,
            actual_role: producer_channel.role.clone(),
        });
    }
    if consumer_channel.role != ChannelRole::LampIndicator {
        return Err(FacilityCompositionError::RoleMismatch {
            slot_label: consumer_slot.label.to_string(),
            expected_role: consumer_slot.required_role,
            actual_role: consumer_channel.role.clone(),
        });
    }

    // ── Consumer leaf lookup: today's only supported binding is lampRow ─
    let (consumer_node_key, consumer_row) = match &consumer_channel.binding {
        ChannelBinding::LampRow {
            node_key,
            row_ordinal,
        } => (node_key.clone(), *row_ordinal),
        ChannelBinding::ConnectorInput { .. } => {
            return Err(FacilityCompositionError::RoleMismatch {
                slot_label: consumer_slot.label.to_string(),
                expected_role: "lamp-indicator",
                actual_role: ChannelRole::BlockOccupancy,
            });
        }
    };

    let consumer_tree = per_node_cdi.get(&consumer_node_key).ok_or_else(|| {
        FacilityCompositionError::MissingConsumerLeaf {
            channel_id: consumer_channel_id.clone(),
            command_name: "(cdi tree unavailable)".to_string(),
        }
    })?;

    let consumer_prefix = resolve_lamp_row_path_prefix(consumer_tree, consumer_row).ok_or_else(
        || FacilityCompositionError::MissingConsumerLeaf {
            channel_id: consumer_channel_id.clone(),
            command_name: format!("(row {consumer_row} not found in CDI)"),
        },
    )?;

    let consumer_leaves = collect_leaves_under_prefix(consumer_tree, &consumer_prefix, EventRole::Consumer);

    let producer_ids =
        producer_event_ids
            .get(producer_channel_id)
            .ok_or_else(|| FacilityCompositionError::MissingProducerEventId {
                channel_id: producer_channel_id.clone(),
                state_name: "(none resolved)".to_string(),
            })?;

    let mut ops = Vec::with_capacity(template.mapping.len());
    for mapping in template.mapping {
        // Producer side: pull the event ID for `producer_state`.
        let producer_bytes = producer_ids.get(mapping.producer_state).ok_or_else(|| {
            FacilityCompositionError::MissingProducerEventId {
                channel_id: producer_channel_id.clone(),
                state_name: mapping.producer_state.to_string(),
            }
        })?;

        // Consumer side: locate the leaf at `consumer_leaf_index[command]`.
        let leaf_ordinal = consumer_leaf_index.get(mapping.consumer_command).ok_or_else(
            || FacilityCompositionError::MissingConsumerLeaf {
                channel_id: consumer_channel_id.clone(),
                command_name: mapping.consumer_command.to_string(),
            },
        )?;
        let leaf = consumer_leaves.get(*leaf_ordinal as usize).ok_or_else(|| {
            FacilityCompositionError::MissingConsumerLeaf {
                channel_id: consumer_channel_id.clone(),
                command_name: mapping.consumer_command.to_string(),
            }
        })?;

        ops.push(CompositionOp {
            consumer_node_key: consumer_node_key.clone(),
            consumer_leaf_path: leaf.path.clone(),
            consumer_leaf_space: leaf.space,
            consumer_leaf_address: leaf.address,
            event_id_bytes: *producer_bytes,
            bowtie_name: format!("{} — {}", facility.name, mapping.consumer_command),
            created_by_facility: facility.facility_id.clone(),
        });
    }

    Ok(ops)
}

fn find_channel<'a>(
    channels: &'a [InformationChannel],
    id: &str,
) -> Option<&'a InformationChannel> {
    channels.iter().find(|c| c.id == id)
}

fn collect_leaves_under_prefix<'a>(
    tree: &'a NodeConfigTree,
    prefix: &[String],
    role: EventRole,
) -> Vec<&'a LeafNode> {
    let mut out = Vec::new();
    for seg in &tree.segments {
        collect(&seg.children, prefix, role, &mut out);
    }
    out
}

fn collect<'a>(
    children: &'a [ConfigNode],
    prefix: &[String],
    role: EventRole,
    out: &mut Vec<&'a LeafNode>,
) {
    for child in children {
        match child {
            ConfigNode::Leaf(leaf) => {
                if leaf.element_type == LeafType::EventId
                    && role_matches(leaf.event_role, role)
                    && path_starts_with(&leaf.path, prefix)
                {
                    out.push(leaf);
                }
            }
            ConfigNode::Group(g) => collect(&g.children, prefix, role, out),
        }
    }
}

/// Permissive role match — mirrors the frontend's `_roleMatches` rule in
/// `effectiveLayoutStore.svelte.ts`. Unannotated (`None`) and
/// `Ambiguous` leaves match any expected role.
///
/// **Why permissive here but strict in `channel_events`?** The compose
/// path scopes its search to a **single-role subtree by profile design**
/// (a lamp row under `Direct Lamp Control/Lamp #N` contains only
/// consumer EventId leaves; a track-circuit row contains only
/// consumers; etc.). Under such a prefix, no ambiguity exists between
/// producer and consumer leaves — the profile YAML has already
/// declared the whole group as one role.
///
/// The `channel_events::collect_from_children` path, by contrast,
/// scopes to a **mixed-role subtree** (a Tower-LCC connector line
/// group contains BOTH Actions/Producer and Commands/Consumer leaves).
/// Permissive matching there would silently return leaves of the wrong
/// role and corrupt `producerLeafIndex` ordinal resolution, so it
/// stays strict.
///
/// The tolerance here is defence-in-depth against profile-annotation
/// timing races on live proxy trees — see the note in
/// `app/src-tauri/src/commands/cdi.rs` around
/// `build_node_config_tree` in the config-read completion path, which
/// stores a freshly-built tree onto the proxy *before* the
/// profile-annotation pass runs across all handles.
fn role_matches(actual: Option<EventRole>, expected: EventRole) -> bool {
    match actual {
        None => true,
        Some(EventRole::Ambiguous) => true,
        Some(r) => r == expected,
    }
}

fn path_starts_with(path: &[String], prefix: &[String]) -> bool {
    if path.len() < prefix.len() {
        return false;
    }
    path[..prefix.len()] == *prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior_templates::BLOCK_INDICATOR;
    use crate::layout::channels::{ChannelBinding, ChannelOwnership, ChannelRole};
    use crate::node_tree::{
        ConfigNode, ConfigValue, GroupNode, LeafNode, LeafType, NodeConfigTree, SegmentNode,
    };
    use std::collections::BTreeMap;

    const PRODUCER_NODE: &str = "05010101FF000001";
    const CONSUMER_NODE: &str = "05010101FF000002";
    const OCCUPIED_HEX: &str = "02010101FF010001";
    const CLEAR_HEX: &str = "02010101FF010101";

    fn producer_channel() -> InformationChannel {
        InformationChannel {
            id: "ch-bod-1".to_string(),
            name: "BOD A1".to_string(),
            role: ChannelRole::BlockOccupancy,
            style: "bod-block-detector-input".to_string(),
            ownership: ChannelOwnership::HardwareOwned,
            binding: ChannelBinding::ConnectorInput {
                node_key: PRODUCER_NODE.to_string(),
                connector: "connector-a".to_string(),
                input: 1,
            },
        }
    }

    fn consumer_channel() -> InformationChannel {
        InformationChannel {
            id: "ch-lamp-2".to_string(),
            name: "Block 5 output".to_string(),
            role: ChannelRole::LampIndicator,
            style: "single-led-direct-lamp".to_string(),
            ownership: ChannelOwnership::UserOwned,
            binding: ChannelBinding::LampRow {
                node_key: CONSUMER_NODE.to_string(),
                row_ordinal: 2,
            },
        }
    }

    fn wired_block_5() -> Facility {
        let mut sb: BTreeMap<String, Vec<String>> = BTreeMap::new();
        sb.insert("input".to_string(), vec!["ch-bod-1".to_string()]);
        sb.insert("output".to_string(), vec!["ch-lamp-2".to_string()]);
        Facility {
            facility_id: "f-block-5".to_string(),
            template_id: "block-indicator".to_string(),
            name: "Block 5".to_string(),
            slot_bindings: sb,
        }
    }

    fn hex_bytes(hex: &str) -> [u8; 8] {
        let mut out = [0u8; 8];
        for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
            out[i] = u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap();
        }
        out
    }

    fn make_consumer_tree() -> NodeConfigTree {
        // Direct Lamp Control segment with two Lamp instances; each Lamp
        // group has two consumer EventId leaves (Lamp On / Lamp Off).
        let make_lamp_group = |ordinal: u32, base_addr: u32| ConfigNode::Group(GroupNode {
            name: format!("Lamp #{ordinal}"),
            has_name: true,
            description: None,
            instance: ordinal,
            instance_label: format!("Lamp #{ordinal}"),
            replication_of: "Lamp".to_string(),
            replication_count: 4,
            path: vec!["Direct Lamp Control".to_string(), format!("Lamp #{ordinal}")],
            display_name: Some(format!("Lamp #{ordinal}")),
            hideable: false,
            hidden_by_default: false,
            read_only: false,
            children: vec![
                ConfigNode::Leaf(LeafNode {
                    name: "Lamp On".to_string(),
                    description: None,
                    element_type: LeafType::EventId,
                    address: base_addr,
                    size: 8,
                    space: 253,
                    path: vec![
                        "Direct Lamp Control".to_string(),
                        format!("Lamp #{ordinal}"),
                        "Lamp On".to_string(),
                    ],
                    value: Some(ConfigValue::EventId {
                        bytes: [0; 8],
                        hex: "0000000000000000".to_string(),
                    }),
                    event_role: Some(EventRole::Consumer),
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
                }),
                ConfigNode::Leaf(LeafNode {
                    name: "Lamp Off".to_string(),
                    description: None,
                    element_type: LeafType::EventId,
                    address: base_addr + 8,
                    size: 8,
                    space: 253,
                    path: vec![
                        "Direct Lamp Control".to_string(),
                        format!("Lamp #{ordinal}"),
                        "Lamp Off".to_string(),
                    ],
                    value: Some(ConfigValue::EventId {
                        bytes: [0; 8],
                        hex: "0000000000000000".to_string(),
                    }),
                    event_role: Some(EventRole::Consumer),
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
                }),
            ],
        });

        NodeConfigTree {
            node_id: CONSUMER_NODE.to_string(),
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
                children: vec![make_lamp_group(1, 100), make_lamp_group(2, 116)],
            }],
        }
    }

    fn consumer_leaf_index() -> ConsumerLeafIndex {
        let mut m = HashMap::new();
        m.insert("lit".to_string(), 0);
        m.insert("unlit".to_string(), 1);
        m
    }

    fn producer_event_ids() -> HashMap<String, ProducerEventIds> {
        let mut per_channel = HashMap::new();
        let mut ids = HashMap::new();
        ids.insert("occupied".to_string(), hex_bytes(OCCUPIED_HEX));
        ids.insert("clear".to_string(), hex_bytes(CLEAR_HEX));
        per_channel.insert("ch-bod-1".to_string(), ids);
        per_channel
    }

    #[test]
    fn wired_block_indicator_produces_two_ops_adopting_producer_event_ids() {
        let facility = wired_block_5();
        let channels = vec![producer_channel(), consumer_channel()];
        let mut trees = HashMap::new();
        trees.insert(CONSUMER_NODE.to_string(), make_consumer_tree());

        let ops = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &producer_event_ids(),
            &trees,
            &consumer_leaf_index(),
        )
        .unwrap();

        assert_eq!(ops.len(), 2);

        // op[0] — occupied → lit, Lamp On leaf, occupied bytes.
        assert_eq!(ops[0].event_id_bytes, hex_bytes(OCCUPIED_HEX));
        assert_eq!(
            ops[0].consumer_leaf_path,
            vec![
                "Direct Lamp Control".to_string(),
                "Lamp #2".to_string(),
                "Lamp On".to_string(),
            ],
        );
        assert_eq!(ops[0].consumer_node_key, CONSUMER_NODE);
        assert_eq!(ops[0].bowtie_name, "Block 5 — lit");
        assert_eq!(ops[0].created_by_facility, "f-block-5");

        // op[1] — clear → unlit, Lamp Off leaf, clear bytes.
        assert_eq!(ops[1].event_id_bytes, hex_bytes(CLEAR_HEX));
        assert_eq!(
            ops[1].consumer_leaf_path,
            vec![
                "Direct Lamp Control".to_string(),
                "Lamp #2".to_string(),
                "Lamp Off".to_string(),
            ],
        );
        assert_eq!(ops[1].bowtie_name, "Block 5 — unlit");
    }

    #[test]
    fn not_wired_on_empty_input_slot() {
        let mut facility = wired_block_5();
        facility.slot_bindings.insert("input".to_string(), vec![]);
        let channels = vec![producer_channel(), consumer_channel()];
        let mut trees = HashMap::new();
        trees.insert(CONSUMER_NODE.to_string(), make_consumer_tree());

        let err = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &producer_event_ids(),
            &trees,
            &consumer_leaf_index(),
        )
        .unwrap_err();
        match err {
            FacilityCompositionError::NotWired { slot_label } => {
                assert_eq!(slot_label, "input");
            }
            e => panic!("expected NotWired, got {e:?}"),
        }
    }

    #[test]
    fn not_wired_on_empty_output_slot() {
        let mut facility = wired_block_5();
        facility.slot_bindings.insert("output".to_string(), vec![]);
        let channels = vec![producer_channel(), consumer_channel()];
        let mut trees = HashMap::new();
        trees.insert(CONSUMER_NODE.to_string(), make_consumer_tree());

        let err = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &producer_event_ids(),
            &trees,
            &consumer_leaf_index(),
        )
        .unwrap_err();
        assert!(matches!(err, FacilityCompositionError::NotWired { slot_label } if slot_label == "output"));
    }

    #[test]
    fn not_wired_on_empty_both_slots() {
        let mut facility = wired_block_5();
        facility.slot_bindings.insert("input".to_string(), vec![]);
        facility.slot_bindings.insert("output".to_string(), vec![]);
        let channels = vec![producer_channel(), consumer_channel()];
        let mut trees = HashMap::new();
        trees.insert(CONSUMER_NODE.to_string(), make_consumer_tree());

        let err = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &producer_event_ids(),
            &trees,
            &consumer_leaf_index(),
        )
        .unwrap_err();
        // First slot in declaration order (input) is checked first.
        assert!(matches!(err, FacilityCompositionError::NotWired { .. }));
    }

    #[test]
    fn missing_producer_event_id_surfaces() {
        let facility = wired_block_5();
        let channels = vec![producer_channel(), consumer_channel()];
        let mut trees = HashMap::new();
        trees.insert(CONSUMER_NODE.to_string(), make_consumer_tree());

        // Producer id map missing the "clear" state.
        let mut per_channel = HashMap::new();
        let mut ids = HashMap::new();
        ids.insert("occupied".to_string(), hex_bytes(OCCUPIED_HEX));
        per_channel.insert("ch-bod-1".to_string(), ids);

        let err = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &per_channel,
            &trees,
            &consumer_leaf_index(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FacilityCompositionError::MissingProducerEventId { state_name, .. } if state_name == "clear"
        ));
    }

    #[test]
    fn unknown_producer_channel_surfaces() {
        let facility = wired_block_5();
        // Only consumer channel present; producer id lookup fails.
        let channels = vec![consumer_channel()];
        let mut trees = HashMap::new();
        trees.insert(CONSUMER_NODE.to_string(), make_consumer_tree());

        let err = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &producer_event_ids(),
            &trees,
            &consumer_leaf_index(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FacilityCompositionError::UnknownChannel { slot_label, .. } if slot_label == "input"
        ));
    }

    #[test]
    fn missing_consumer_cdi_surfaces() {
        let facility = wired_block_5();
        let channels = vec![producer_channel(), consumer_channel()];
        // Empty CDI map → consumer node's tree is missing.
        let trees = HashMap::new();

        let err = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &producer_event_ids(),
            &trees,
            &consumer_leaf_index(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FacilityCompositionError::MissingConsumerLeaf { .. }
        ));
    }

    /// Regression: profile-annotation timing race. A live proxy's config
    /// tree can transiently carry `event_role: None` on its EventId
    /// leaves — [app/src-tauri/src/commands/cdi.rs] rebuilds a tree via
    /// `build_node_config_tree` after a config-value read and stores it
    /// on the proxy *before* the profile-annotation pass runs. The
    /// compose IPC that fires from a Wired transition can hit this
    /// unannotated tree and, under the previous strict
    /// `event_role == Some(role)` filter, would return zero consumer
    /// leaves — surfacing as `MissingConsumerLeaf { command: 'lit' }`.
    ///
    /// The permissive `role_matches` rule (None / Ambiguous match any
    /// expected role) mirrors the frontend's `_roleMatches` convention
    /// in `effectiveLayoutStore` and restores composition against
    /// transiently-unannotated trees.
    #[test]
    fn composes_against_unannotated_consumer_tree_regression() {
        let facility = wired_block_5();
        let channels = vec![producer_channel(), consumer_channel()];

        // Build a consumer tree exactly like `make_consumer_tree` but
        // with `event_role: None` on every EventId leaf — mirroring the
        // pre-profile-annotation state of a freshly-rebuilt live tree.
        let unannotated = {
            let mut tree = make_consumer_tree();
            fn strip(children: &mut Vec<ConfigNode>) {
                for c in children.iter_mut() {
                    match c {
                        ConfigNode::Leaf(l) if l.element_type == LeafType::EventId => {
                            l.event_role = None;
                        }
                        ConfigNode::Group(g) => strip(&mut g.children),
                        _ => {}
                    }
                }
            }
            for seg in tree.segments.iter_mut() {
                strip(&mut seg.children);
            }
            tree
        };

        let mut trees = HashMap::new();
        trees.insert(CONSUMER_NODE.to_string(), unannotated);

        let ops = compose_bowtie_ops(
            &facility,
            &BLOCK_INDICATOR,
            &channels,
            &producer_event_ids(),
            &trees,
            &consumer_leaf_index(),
        )
        .expect("composition must succeed against unannotated tree");

        // The two consumer ops resolve to the correct leaves in tree order
        // (Lamp On = index 0 = "lit"; Lamp Off = index 1 = "unlit").
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].consumer_leaf_path.last().unwrap(), "Lamp On");
        assert_eq!(ops[1].consumer_leaf_path.last().unwrap(), "Lamp Off");
    }

    /// Producer roles must NOT be silently reinterpreted as Consumer
    /// (or vice versa). The permissive fallback only applies when the
    /// leaf is unannotated (`None`) or `Ambiguous`; leaves that carry
    /// an explicit wrong role are excluded.
    #[test]
    fn explicit_wrong_role_still_excluded_by_role_matches() {
        use lcc_rs::cdi::EventRole;
        assert!(role_matches(None, EventRole::Consumer));
        assert!(role_matches(Some(EventRole::Ambiguous), EventRole::Consumer));
        assert!(role_matches(Some(EventRole::Consumer), EventRole::Consumer));
        assert!(!role_matches(Some(EventRole::Producer), EventRole::Consumer));
    }
}
