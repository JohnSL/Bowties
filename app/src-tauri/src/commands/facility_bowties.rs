//! Tauri command for facility bowtie composition (Spec 018 / S6 — D2).
//!
//! Bridges `bowties_core::facility_bowties::compose_bowtie_ops` to the
//! frontend. Consults the live authoritative state:
//!
//! * facilities + channels — from `LayoutState`'s effective
//!   (drafts-over-saved) view; the frontend calls `sync_layout_drafts`
//!   before this IPC so pending facility/channel edits are visible
//! * behaviour-template registry — from `bowties_core::behavior_templates`
//! * per-node CDI trees — from the live `NodeRegistry` (falling back to
//!   the persisted trees inside `LayoutState`)
//! * producer event IDs — resolved from the producer channel's CDI leaves
//!   using `bowties_core::channel_events::resolve_channel_event_ids`
//!
//! The consumer-side leaf-index map (`lit → 0`, `unlit → 1`) is hardcoded
//! here for `single-led-direct-lamp`; when the frontend style catalog moves
//! to backend YAML in a later slice, the mapping resolves from the profile.

use std::collections::HashMap;

use bowties_core::facility_bowties::{
    compose_bowtie_ops, CompositionOp, ConsumerLeafIndex, FacilityCompositionError,
    ProducerEventIds,
};
use bowties_core::layout::channels::{ChannelBinding, ChannelRole, InformationChannel};
use bowties_core::node_key::NodeKey;
use bowties_core::node_tree::NodeConfigTree;

use crate::state::AppState;

fn parse_hex_id(hex: &str) -> Option<[u8; 8]> {
    let cleaned: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if cleaned.len() != 16 {
        return None;
    }
    let mut out = [0u8; 8];
    for (i, chunk) in cleaned.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
}

/// Consumer style event-mapping (mirrors the frontend `channelStyles.ts`
/// registry). Today only `single-led-direct-lamp` is composable.
fn consumer_leaf_index_for_style(style: &str) -> Option<ConsumerLeafIndex> {
    match style {
        "single-led-direct-lamp" => {
            let mut m = HashMap::new();
            m.insert("lit".to_string(), 0);
            m.insert("unlit".to_string(), 1);
            Some(m)
        }
        _ => None,
    }
}

/// Producer style event-mapping. Today only `bod-block-detector-input`.
fn producer_leaf_index_for_style(style: &str) -> Option<HashMap<String, u32>> {
    match style {
        "bod-block-detector-input" => {
            let mut m = HashMap::new();
            m.insert("occupied".to_string(), 0);
            m.insert("clear".to_string(), 1);
            Some(m)
        }
        _ => None,
    }
}

/// Compose the [`CompositionOp`]s for a Wired facility.
///
/// Returns an error string when the facility is unknown, its slots are not
/// at their `min_channels`, or a bound channel's CDI does not resolve.
#[tauri::command]
pub async fn compose_facility_bowties(
    facility_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CompositionOp>, String> {
    let layout_guard = state.layout_state.read().await;
    let layout_state = layout_guard
        .as_ref()
        .ok_or_else(|| "no layout is open".to_string())?;

    // Spec 018 / S6 bugfix — read facilities + channels through the
    // effective (drafts-over-saved) view so composition sees the
    // frontend's pending facility / channel edits. The frontend calls
    // `sync_layout_drafts` right before this IPC to populate them.
    // Locate the facility.
    let facility = layout_state
        .effective_facilities()
        .facilities
        .iter()
        .find(|f| f.facility_id == facility_id)
        .ok_or_else(|| format!("unknown facility '{}'", facility_id))?
        .clone();

    // Resolve the facility's template.
    let template = bowties_core::behavior_templates::find_template(&facility.template_id)
        .ok_or_else(|| {
            format!(
                "facility '{}' references unknown template '{}'",
                facility_id, facility.template_id
            )
        })?;

    // Snapshot channels.
    let channels: Vec<InformationChannel> = layout_state.effective_channels().channels.clone();

    // Gather CDI trees for every node referenced by a bound channel, preferring
    // the live proxy's tree when connected and falling back to the persisted
    // tree in `LayoutState`.
    let mut per_node_cdi: HashMap<String, NodeConfigTree> = HashMap::new();
    for bindings in facility.slot_bindings.values() {
        for channel_id in bindings {
            let Some(channel) = channels.iter().find(|c| c.id == *channel_id) else {
                continue;
            };
            let node_key_str = match &channel.binding {
                ChannelBinding::ConnectorInput { node_key, .. } => node_key.clone(),
                ChannelBinding::LampRow { node_key, .. } => node_key.clone(),
            };
            if per_node_cdi.contains_key(&node_key_str) {
                continue;
            }
            let parsed_key = NodeKey::parse(&node_key_str)
                .map_err(|e| format!("invalid node key '{}': {}", node_key_str, e))?;
            // Prefer LayoutState tree (captured-over-saved precedence).
            let tree = layout_state.config_tree(&parsed_key).cloned();
            if let Some(tree) = tree {
                per_node_cdi.insert(node_key_str, tree);
            } else {
                // Genuine fault signal — no tree via live proxy or LayoutState.
                // Compose will fail downstream with `MissingConsumerLeaf` or
                // `MissingProducerEventId`; this log makes the root cause
                // visible without waiting for the frontend error toast.
                eprintln!(
                    "[facility_bowties] no tree for {} (neither live proxy nor LayoutState)",
                    node_key_str
                );
            }
        }
    }

    // Resolve producer event IDs by walking the producer channel's CDI.
    let mut producer_event_ids: HashMap<String, ProducerEventIds> = HashMap::new();
    for channel in channels.iter().filter(|c| c.role == ChannelRole::BlockOccupancy) {
        let (node_key_str, connector, input) = match &channel.binding {
            ChannelBinding::ConnectorInput {
                node_key,
                connector,
                input,
            } => (node_key.clone(), connector.clone(), *input),
            _ => continue,
        };
        let Some(tree) = per_node_cdi.get(&node_key_str) else {
            continue;
        };
        let Some(mapping) = producer_leaf_index_for_style(&channel.style) else {
            continue;
        };
        let ids_hex = bowties_core::channel_events::resolve_channel_event_ids(
            tree, &connector, input, &mapping,
        );
        if ids_hex.is_empty() {
            continue;
        }
        let mut ids_bytes: ProducerEventIds = HashMap::new();
        for (state_name, hex) in ids_hex {
            if let Some(bytes) = parse_hex_id(&hex) {
                ids_bytes.insert(state_name, bytes);
            }
        }
        producer_event_ids.insert(channel.id.clone(), ids_bytes);
    }

    // Consumer-side leaf-index map derives from the consumer channel's style.
    let consumer_slot = template
        .slots
        .iter()
        .find(|s| s.kind == bowties_core::behavior_templates::SlotKind::Consumer)
        .ok_or_else(|| "template declares no consumer slot".to_string())?;
    let consumer_channel_id = facility
        .slot_bindings
        .get(consumer_slot.label)
        .and_then(|v| v.first())
        .ok_or_else(|| format!("facility '{}' has no consumer channel", facility_id))?
        .clone();
    let consumer_channel = channels
        .iter()
        .find(|c| c.id == consumer_channel_id)
        .ok_or_else(|| {
            format!(
                "consumer channel '{}' is missing from inventory",
                consumer_channel_id
            )
        })?;
    let consumer_leaf_index = consumer_leaf_index_for_style(&consumer_channel.style)
        .ok_or_else(|| {
            format!(
                "consumer channel '{}' style '{}' has no composable event mapping",
                consumer_channel_id, consumer_channel.style
            )
        })?;

    compose_bowtie_ops(
        &facility,
        template,
        &channels,
        &producer_event_ids,
        &per_node_cdi,
        &consumer_leaf_index,
    )
    .map_err(|e: FacilityCompositionError| e.to_string())
}
