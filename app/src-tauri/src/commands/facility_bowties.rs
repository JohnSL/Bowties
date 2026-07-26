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
    compose_bowtie_ops, compose_compiled_bowtie_ops, CompositionOp, ConsumerLeafIndex,
    FacilityCompositionError, ProducerEventIds,
};
use bowties_core::layout::channels::{ChannelBinding, ChannelRole, InformationChannel};
use bowties_core::layout::facilities::Facility;
use bowties_core::layout::state::LayoutState;
use bowties_core::logic_adapter::{self, CompileInput, InputChannelEvents, PinEvents};
use bowties_core::behavior_templates::BehaviorTemplate;
use bowties_core::node_key::NodeKey;
use bowties_core::node_tree::NodeConfigTree;
use lcc_rs::cdi::EventRole;

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

    // Spec 020 / S2 — compiled templates (e.g. ABS 3-Aspect Signal) are
    // composed via the compiler's `WiringPlan`, not the producer/consumer
    // slot path below. The composer is the sole event-wiring owner for
    // both template kinds (Single Event-Wiring Owner); this branch
    // rebuilds the same `CompileInput` the compile IPC used and recomputes
    // the plan (D2:A-alt — pure re-derivation, no cache).
    if template.compilation_target
        == bowties_core::behavior_templates::CompilationTarget::Compiled
    {
        return compose_compiled_template(&facility, template, layout_state);
    }

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

/// Compose bowtie ops for a Wired **compiled** facility (Spec 020 / S2).
///
/// Rebuilds the same `CompileInput` [`compile_logic_for_facility`] used
/// (same channel-resolution helpers, same downstream/tc_output rules) and
/// hands it to `compose_compiled_bowtie_ops`, which recomputes the
/// `WiringPlan` and fills its event-ID slots by adopting event IDs already
/// resolved onto `CompileInput` — never minting fresh ones (D6).
///
/// [`compile_logic_for_facility`]: crate::commands::logic_adapter::compile_logic_for_facility
fn compose_compiled_template(
    facility: &Facility,
    template: &BehaviorTemplate,
    layout_state: &LayoutState,
) -> Result<Vec<CompositionOp>, String> {
    let allocation = facility.logic_allocation.as_ref().ok_or_else(|| {
        format!(
            "facility '{}' has not been compiled to a logic target yet",
            facility.facility_id
        )
    })?;
    let target_node_key = allocation.target_node_key.clone();

    let channels: Vec<InformationChannel> = layout_state.effective_channels().channels.clone();

    // ── Resolve input (block-occupancy) channel event IDs ──────────────

    let input_slot = template
        .find_slot("input")
        .ok_or_else(|| "template has no input slot".to_string())?;
    let input_channel_id = facility
        .slot_bindings
        .get(input_slot.label)
        .and_then(|v| v.first())
        .ok_or_else(|| format!("facility '{}' has no input channel bound", facility.facility_id))?;
    let input_channel = channels
        .iter()
        .find(|c| c.id == *input_channel_id)
        .ok_or_else(|| format!("input channel '{}' not in inventory", input_channel_id))?;
    let (input_node_key_str, connector, input_ordinal) = match &input_channel.binding {
        ChannelBinding::ConnectorInput {
            node_key,
            connector,
            input,
        } => (node_key.clone(), connector.clone(), *input),
        _ => return Err("input channel has non-ConnectorInput binding".to_string()),
    };
    let input_parsed_key = NodeKey::parse(&input_node_key_str)
        .map_err(|e| format!("invalid input node key '{}': {}", input_node_key_str, e))?;
    let input_tree = layout_state
        .config_tree(&input_parsed_key)
        .ok_or_else(|| format!("no CDI tree for input node {}", input_node_key_str))?;
    let producer_mapping = producer_leaf_index_for_style(&input_channel.style)
        .ok_or_else(|| format!("unknown producer style '{}'", input_channel.style))?;
    let input_ids_hex = bowties_core::channel_events::resolve_channel_event_ids(
        input_tree,
        &connector,
        input_ordinal,
        &producer_mapping,
    );
    let occupied_hex = input_ids_hex.get("occupied").ok_or_else(|| {
        "occupied event not available — read config from the block detector node first".to_string()
    })?;
    let clear_hex = input_ids_hex.get("clear").ok_or_else(|| {
        "clear event not available — read config from the block detector node first".to_string()
    })?;
    let input_events = InputChannelEvents {
        set_true_event: parse_hex_id(occupied_hex)
            .ok_or_else(|| format!("invalid occupied event hex: {occupied_hex}"))?,
        set_false_event: parse_hex_id(clear_hex)
            .ok_or_else(|| format!("invalid clear event hex: {clear_hex}"))?,
    };

    // ── Resolve output (signal-aspect) channel pin events ────────────

    let output_slot = template
        .find_slot("output")
        .ok_or_else(|| "template has no output slot".to_string())?;
    let output_channel_id = facility
        .slot_bindings
        .get(output_slot.label)
        .and_then(|v| v.first())
        .ok_or_else(|| format!("facility '{}' has no output channel bound", facility.facility_id))?;
    let output_channel = channels
        .iter()
        .find(|c| c.id == *output_channel_id)
        .ok_or_else(|| format!("output channel '{}' not in inventory", output_channel_id))?;
    let (output_node_key_str, base_row) = match &output_channel.binding {
        ChannelBinding::LampRow {
            node_key,
            row_ordinal,
        } => (node_key.clone(), *row_ordinal),
        _ => return Err("output channel has non-LampRow binding".to_string()),
    };
    let output_parsed_key = NodeKey::parse(&output_node_key_str)
        .map_err(|e| format!("invalid output node key '{}': {}", output_node_key_str, e))?;
    let output_tree = layout_state
        .config_tree(&output_parsed_key)
        .ok_or_else(|| format!("no CDI tree for output node {}", output_node_key_str))?;

    let mut pin_leaf_map = HashMap::new();
    pin_leaf_map.insert("red_on".to_string(), 0u32);
    pin_leaf_map.insert("red_off".to_string(), 1u32);
    pin_leaf_map.insert("green_on".to_string(), 2u32);
    pin_leaf_map.insert("green_off".to_string(), 3u32);
    let pin_ids_hex = bowties_core::channel_events::resolve_lamp_row_range_event_ids(
        output_tree,
        base_row,
        2,
        EventRole::Consumer,
        &pin_leaf_map,
    );
    let pin_event = |key: &str| -> Result<[u8; 8], String> {
        let hex = pin_ids_hex
            .get(key)
            .ok_or_else(|| format!("{key} event not available — read config from the Signal LCC node first"))?;
        parse_hex_id(hex).ok_or_else(|| format!("invalid {key} event hex: {hex}"))
    };
    let output_pin_events = vec![
        PinEvents {
            on_event: pin_event("red_on")?,
            off_event: pin_event("red_off")?,
        },
        PinEvents {
            on_event: pin_event("green_on")?,
            off_event: pin_event("green_off")?,
        },
    ];

    // ── Resolve downstream + tc_output exactly as the compiler does ──────

    let all_facilities = &layout_state.effective_facilities().facilities;
    let all_allocations = &layout_state.effective_facilities().logic_allocations;
    let downstream =
        logic_adapter::resolve_downstream_binding(facility, all_facilities, all_allocations);
    let tc_output = allocation.track_circuit;

    // `existing_allocations` excludes this facility's own allocation —
    // `plan_facility_wiring` must reproduce the exact line-index base the
    // compiler used, which was computed BEFORE this facility's own
    // allocation existed.
    let existing_allocations: Vec<_> = all_allocations
        .iter()
        .filter(|a| a.facility_id != facility.facility_id)
        .cloned()
        .collect();

    let compile_input = CompileInput {
        template_id: facility.template_id.clone(),
        facility_id: facility.facility_id.clone(),
        facility_name: facility.name.clone(),
        target_node_key: target_node_key.clone(),
        existing_allocations,
        input_events,
        output_pin_events,
        downstream,
        tc_output,
    };

    let target_parsed_key = NodeKey::parse(&target_node_key)
        .map_err(|e| format!("invalid target node key '{}': {}", target_node_key, e))?;
    let target_tree = layout_state
        .config_tree(&target_parsed_key)
        .ok_or_else(|| format!("no CDI tree for target node {}", target_node_key))?;

    compose_compiled_bowtie_ops(&compile_input, target_tree, &input_channel.name, &output_channel.name)
        .map_err(|e: FacilityCompositionError| e.to_string())
}
