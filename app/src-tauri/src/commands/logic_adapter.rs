//! Tauri commands for logic compilation (Spec 020 / S2).
//!
//! Reads from `LayoutState` effective views (drafts-over-saved) and per-node
//! CDI trees to build the compiler's pure `CompileInput`. Follows the same
//! data-gathering pattern as `compose_facility_bowties`.

use std::collections::HashMap;

use bowties_core::channel_events::{resolve_event_ids, resolve_lamp_row_path_prefix};
use bowties_core::layout::channels::{ChannelBinding, InformationChannel};
use bowties_core::logic_adapter::{
    build_conditional_line_address_map, compile_facility, get_capacity, has_conditional_lines,
    resolve_downstream_binding, resolve_field_writes, reset_facility, CompileInput, CompiledLogicPlan,
    InputChannelEvents, LogicCapacity, PinEvents,
};
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

/// Producer style event-mapping (occupied/clear leaf indices).
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

/// Consumer lamp event-mapping (on/off leaf indices within a Lamp group).
fn lamp_event_leaf_map() -> HashMap<String, u32> {
    let mut m = HashMap::new();
    m.insert("on".to_string(), 0);
    m.insert("off".to_string(), 1);
    m
}

/// Resolve On/Off event IDs for a single lamp row from the CDI tree.
fn resolve_lamp_row_events(
    tree: &NodeConfigTree,
    row_ordinal: u32,
) -> Result<PinEvents, String> {
    let path_prefix = resolve_lamp_row_path_prefix(tree, row_ordinal)
        .ok_or_else(|| format!("lamp row {} not found in CDI tree", row_ordinal))?;
    eprintln!(
        "[resolve_lamp_row] row={} path_prefix={:?}",
        row_ordinal, path_prefix
    );
    let map = lamp_event_leaf_map();
    let events = resolve_event_ids(tree, &path_prefix, EventRole::Consumer, &map);
    eprintln!(
        "[resolve_lamp_row] row={} resolved_events={:?}",
        row_ordinal, events.keys().collect::<Vec<_>>()
    );
    if events.is_empty() {
        // Diagnostic: check if leaves exist but lack role or value
        use bowties_core::node_tree::{ConfigNode, LeafType};
        let mut leaf_count = 0u32;
        let mut eventid_count = 0u32;
        let mut has_role_count = 0u32;
        let mut has_value_count = 0u32;
        fn scan_leaves(children: &[ConfigNode], prefix: &[String], leaf_count: &mut u32, eventid_count: &mut u32, has_role_count: &mut u32, has_value_count: &mut u32) {
            for child in children {
                match child {
                    ConfigNode::Leaf(leaf) => {
                        if leaf.path.starts_with(prefix) || prefix.iter().all(|p| leaf.path.contains(p)) {
                            *leaf_count += 1;
                            if leaf.element_type == LeafType::EventId {
                                *eventid_count += 1;
                                if leaf.event_role.is_some() {
                                    *has_role_count += 1;
                                }
                                if leaf.value.is_some() {
                                    *has_value_count += 1;
                                }
                                eprintln!(
                                    "[resolve_lamp_row]   eventid leaf path={:?} role={:?} has_value={}",
                                    leaf.path, leaf.event_role, leaf.value.is_some()
                                );
                            }
                        }
                    }
                    ConfigNode::Group(g) => {
                        scan_leaves(&g.children, prefix, leaf_count, eventid_count, has_role_count, has_value_count);
                    }
                }
            }
        }
        for seg in &tree.segments {
            scan_leaves(&seg.children, &path_prefix, &mut leaf_count, &mut eventid_count, &mut has_role_count, &mut has_value_count);
        }
        eprintln!(
            "[resolve_lamp_row] row={} diagnostic: total_leaves_under_prefix={} eventid_leaves={} with_role={} with_value={}",
            row_ordinal, leaf_count, eventid_count, has_role_count, has_value_count
        );
    }
    let on_hex = events
        .get("on")
        .ok_or_else(|| format!("lamp row {} On event not available — read config from the Signal LCC node first", row_ordinal))?;
    let off_hex = events
        .get("off")
        .ok_or_else(|| format!("lamp row {} Off event not available — read config from the Signal LCC node first", row_ordinal))?;
    let on_event = parse_hex_id(on_hex)
        .ok_or_else(|| format!("lamp row {} On event invalid hex: {}", row_ordinal, on_hex))?;
    let off_event = parse_hex_id(off_hex).ok_or_else(|| {
        format!(
            "lamp row {} Off event invalid hex: {}",
            row_ordinal, off_hex
        )
    })?;
    Ok(PinEvents { on_event, off_event })
}

/// Resolve pin events for a 2-LED bicolor signal-aspect channel.
///
/// Pin 0 (red) → lamp row at `base_row`, Pin 1 (green) → `base_row + 1`.
fn resolve_bicolor_pin_events(
    tree: &NodeConfigTree,
    base_row: u32,
) -> Result<Vec<PinEvents>, String> {
    Ok(vec![
        resolve_lamp_row_events(tree, base_row)?,
        resolve_lamp_row_events(tree, base_row + 1)?,
    ])
}

/// Compile the logic for a facility on a target node.
///
/// Reads the facility's template, channel bindings, and CDI trees from
/// `LayoutState` effective views. Resolves channel event IDs, builds
/// `CompileInput`, and calls the pure compiler.
#[tauri::command]
pub async fn compile_logic_for_facility(
    facility_id: String,
    target_node_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<CompiledLogicPlan, String> {
    let layout_guard = state.layout_state.read().await;
    let layout_state = layout_guard
        .as_ref()
        .ok_or_else(|| "no layout is open".to_string())?;

    // Locate the facility.
    let facility = layout_state
        .effective_facilities()
        .facilities
        .iter()
        .find(|f| f.facility_id == facility_id)
        .ok_or_else(|| format!("unknown facility '{facility_id}'"))?
        .clone();

    let template = bowties_core::behavior_templates::find_template(&facility.template_id)
        .ok_or_else(|| {
            format!(
                "facility '{}' references unknown template '{}'",
                facility_id, facility.template_id
            )
        })?;

    let channels: Vec<InformationChannel> = layout_state.effective_channels().channels.clone();

    // ── Resolve input (producer) channel event IDs ────────────────────

    let input_slot = template
        .find_slot("input")
        .ok_or_else(|| "template has no input slot".to_string())?;
    let input_channel_id = facility
        .slot_bindings
        .get(input_slot.label)
        .and_then(|v| v.first())
        .ok_or_else(|| format!("facility '{}' has no input channel bound", facility_id))?;
    let input_channel = channels
        .iter()
        .find(|c| c.id == *input_channel_id)
        .ok_or_else(|| format!("input channel '{}' not in inventory", input_channel_id))?;

    let (input_node_key_str, input_connector, input_ordinal) = match &input_channel.binding {
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
        &input_connector,
        input_ordinal,
        &producer_mapping,
    );
    let occupied_hex = input_ids_hex
        .get("occupied")
        .ok_or_else(|| "occupied event not available — read config from the block detector node first".to_string())?;
    let clear_hex = input_ids_hex
        .get("clear")
        .ok_or_else(|| "clear event not available — read config from the block detector node first".to_string())?;
    let input_events = InputChannelEvents {
        set_true_event: parse_hex_id(occupied_hex)
            .ok_or_else(|| format!("invalid occupied event hex: {occupied_hex}"))?,
        set_false_event: parse_hex_id(clear_hex)
            .ok_or_else(|| format!("invalid clear event hex: {clear_hex}"))?,
    };

    // ── Resolve output (consumer) channel pin events ─────────────────

    let output_slot = template
        .find_slot("output")
        .ok_or_else(|| "template has no output slot".to_string())?;
    let output_channel_id = facility
        .slot_bindings
        .get(output_slot.label)
        .and_then(|v| v.first())
        .ok_or_else(|| format!("facility '{}' has no output channel bound", facility_id))?;
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

    let output_pin_events = resolve_bicolor_pin_events(output_tree, base_row)?;

    // ── Resolve downstream-signal binding (Spec 020 / S4) ────────────

    let all_facilities = &layout_state.effective_facilities().facilities;
    let all_allocations = &layout_state.effective_facilities().logic_allocations;
    let downstream = resolve_downstream_binding(&facility, all_facilities, all_allocations);

    // ── Resolve tc_output (Spec 020 / S5) ────────────────────────────
    // If this facility already has a track_circuit allocated, re-use it.
    // The track_circuit is set by the orchestrator when an upstream signal
    // binds to this facility's output.
    let tc_output = facility
        .logic_allocation
        .as_ref()
        .and_then(|a| a.track_circuit);

    // ── Build CompileInput and compile ───────────────────────────────

    let compile_input = CompileInput {
        template_id: facility.template_id.clone(),
        facility_id: facility_id.clone(),
        facility_name: facility.name.clone(),
        target_node_key: target_node_key.clone(),
        existing_allocations: layout_state
            .effective_facilities()
            .logic_allocations
            .clone(),
        input_events,
        output_pin_events,
        downstream,
        tc_output,
    };

    let compiler_output = compile_facility(&compile_input).map_err(|e| e.to_string())?;

    // Resolve addresses from the target node's config tree.
    let target_parsed_key = NodeKey::parse(&target_node_key)
        .map_err(|e| format!("invalid target node key '{}': {}", target_node_key, e))?;
    let target_tree = layout_state
        .config_tree(&target_parsed_key)
        .ok_or_else(|| format!("no CDI tree for target node {}", target_node_key))?;
    let address_map = build_conditional_line_address_map(target_tree);
    let field_writes = resolve_field_writes(&compiler_output.field_writes, &address_map);

    Ok(CompiledLogicPlan {
        allocation: compiler_output.allocation,
        field_writes,
        wiring_plan: compiler_output.wiring_plan,
    })
}

/// Query the logic capacity of a target node.
///
/// Returns the total and used conditional lines for the given node.
/// If the node's CDI tree lacks conditional line fields, returns
/// `total_lines: 0` so the UI excludes it from the candidate list.
#[tauri::command]
pub async fn get_logic_capacity(
    target_node_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<LogicCapacity, String> {
    let layout_guard = state.layout_state.read().await;
    let Some(layout_state) = layout_guard.as_ref() else {
        return Ok(LogicCapacity {
            total_lines: 0,
            used_lines: 0,
            total_track_circuits: 0,
            used_track_circuits: 0,
        });
    };

    // Check the node's config tree for conditional line fields.
    let parsed_key = NodeKey::parse(&target_node_key)
        .map_err(|e| format!("invalid node key '{}': {}", target_node_key, e))?;

    let tree_opt = layout_state.config_tree(&parsed_key);
    let has_lines = tree_opt
        .map(|tree| {
            let result = has_conditional_lines(tree);
            eprintln!(
                "[get_logic_capacity] node={} tree=Some segments=[{}] has_conditional_lines={}",
                target_node_key,
                tree.segments.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", "),
                result
            );
            result
        })
        .unwrap_or_else(|| {
            // Check whether the node exists in saved layer at all
            let in_saved = layout_state.saved_node(&parsed_key).is_some();
            eprintln!(
                "[get_logic_capacity] node={} tree=None in_saved={}",
                target_node_key, in_saved
            );
            false
        });

    if !has_lines {
        return Ok(LogicCapacity {
            total_lines: 0,
            used_lines: 0,
            total_track_circuits: 0,
            used_track_circuits: 0,
        });
    }

    Ok(get_capacity(
        &target_node_key,
        &layout_state.effective_facilities().logic_allocations,
    ))
}

/// Reset the logic for a facility (inverse of compile_logic_for_facility).
///
/// Produces field writes that set all fields in the allocated conditional
/// line range back to CDI defaults (disabled state). Used when deleting a
/// facility to reclaim the allocation and clear the CDI drafts.
///
/// Returns an empty vec if the facility has no allocation (idempotent).
#[tauri::command]
pub async fn reset_logic_for_facility(
    facility_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<bowties_core::logic_adapter::CompiledFieldWrite>, String> {
    let layout_guard = state.layout_state.read().await;
    let layout_state = layout_guard
        .as_ref()
        .ok_or_else(|| "no layout is open".to_string())?;

    // Locate the facility and its current allocation.
    let facility = layout_state
        .effective_facilities()
        .facilities
        .iter()
        .find(|f| f.facility_id == facility_id)
        .ok_or_else(|| format!("unknown facility '{facility_id}'"))?;

    let Some(allocation) = facility.logic_allocation.as_ref() else {
        // No allocation — nothing to reset. Return empty vec (idempotent).
        return Ok(vec![]);
    };

    // Generate unresolved field writes for the reset.
    let unresolved_writes = reset_facility(allocation);

    // Resolve addresses from the target node's config tree.
    let target_parsed_key = bowties_core::node_key::NodeKey::parse(&allocation.target_node_key)
        .map_err(|e| format!("invalid target node key '{}': {}", allocation.target_node_key, e))?;
    let target_tree = layout_state
        .config_tree(&target_parsed_key)
        .ok_or_else(|| format!("no CDI tree for target node {}", allocation.target_node_key))?;
    let address_map = build_conditional_line_address_map(target_tree);
    let field_writes = resolve_field_writes(&unresolved_writes, &address_map);

    Ok(field_writes)
}
