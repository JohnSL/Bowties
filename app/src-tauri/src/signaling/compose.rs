//! `compose_block_signals` IPC boundary: gathers compiler facts from the
//! effective layout (ADR-0015), compiles, and resolves the generated edits to
//! memory locations. Read-only: no drafts, claims, configuration writes or bus
//! activity. Pure fact extraction and path resolution live in
//! `bowties_core::signaling::slots`; the compile policy in `compiler`.

use std::collections::HashMap;

use bowties_core::{
    layout::{channels::ChannelRole, facilities::ClaimedSlot, state::LayoutState},
    node_key::NodeKey,
    node_tree::{parse_event_id_hex, NodeConfigTree},
    profile::StructureProfile,
    signaling::{
        compiler::{
            compile_block_signals, BlockOccupancyEvents, CompileInput, CompileIssue, SignalWiring,
        },
        slots::{
            claimed_slots_from_facilities, node_slot_facts, pending_edits_from_drafts, resolve_edits,
            DraftEdit, LeafPathIndex, ResolvedEdit,
        },
        BlockSignalsRequest,
    },
};
use serde::{Deserialize, Serialize};

use super::{proposal_facts_from_layout, resolved_detection_events};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeBlockSignalsRequest {
    /// The facility being composed; its own claimed slots do not count as in use.
    #[serde(default)]
    pub facility_id: Option<String>,
    pub line: BlockSignalsRequest,
    pub wiring: Vec<SignalWiring>,
    /// Unsaved configuration edits, as the frontend draft store keys them.
    #[serde(default)]
    pub pending_edits: Vec<DraftEdit>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeBlockSignalsResponse {
    pub edits: Vec<ResolvedEdit>,
    pub claimed_slots: Vec<ClaimedSlot>,
    pub issues: Vec<CompileIssue>,
}

pub async fn compose(
    state: &AppState,
    request: ComposeBlockSignalsRequest,
) -> Result<ComposeBlockSignalsResponse, String> {
    let layout = state.layout_state.read().await;
    let Some(layout) = layout.as_ref() else {
        return Err("No layout is open.".to_string());
    };
    let profiles = state.profiles.read().await;
    compose_from_layout(layout, &profiles, request)
}

fn compose_from_layout(
    layout: &LayoutState,
    profiles: &HashMap<String, Option<StructureProfile>>,
    request: ComposeBlockSignalsRequest,
) -> Result<ComposeBlockSignalsResponse, String> {
    let facts = proposal_facts_from_layout(layout, profiles);
    let claimed = claimed_slots_from_facilities(layout.effective_facilities(), request.facility_id.as_deref());

    let mut occupancy = Vec::new();
    for block in &request.line.blocks {
        let events = facts
            .channels
            .iter()
            .find(|c| c.id == block.block_id && c.role == ChannelRole::BlockOccupancy)
            .map(|channel| resolved_detection_events(&facts, channel))
            .unwrap_or_default();
        let event = |state: &str| events.get(state).and_then(|hex| parse_event_id_hex(hex));
        occupancy.push(BlockOccupancyEvents {
            block_id: block.block_id.clone(),
            occupied: event("occupied"),
            clear: event("clear"),
        });
    }

    let mut trees: HashMap<String, &NodeConfigTree> = HashMap::new();
    for wiring in &request.wiring {
        let key = NodeKey::parse(&wiring.node_key).map_err(|e| format!("Invalid node key '{}': {e}", wiring.node_key))?;
        if let Some(tree) = facts.trees.get(&key) {
            trees.insert(wiring.node_key.clone(), tree);
        }
    }
    let indexes: HashMap<String, LeafPathIndex<'_>> =
        trees.iter().map(|(k, t)| (k.clone(), LeafPathIndex::new(t))).collect();

    let mut nodes = Vec::new();
    for (node_key, tree) in &trees {
        let mut node = node_slot_facts(node_key, tree, &claimed);
        node.pending_edits = pending_edits_from_drafts(node_key, &indexes[node_key], &request.pending_edits)
            .map_err(|e| e.to_string())?;
        nodes.push(node);
    }

    let compiled = compile_block_signals(&CompileInput {
        line: request.line,
        wiring: request.wiring,
        hardware: facts.hardware.clone(),
        nodes,
        occupancy,
    });
    let edits = resolve_edits(&indexes, &compiled.edits).map_err(|e| e.to_string())?;
    Ok(ComposeBlockSignalsResponse { edits, claimed_slots: compiled.claimed_slots, issues: compiled.issues })
}
