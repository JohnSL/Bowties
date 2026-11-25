//! `compose_block_signals` IPC boundary: gathers compiler facts from the
//! effective layout (ADR-0015), compiles, and resolves the generated edits to
//! memory locations. Read-only: no drafts, claims, configuration writes or bus
//! activity. Pure fact extraction and path resolution live in
//! `bowties_core::signaling::slots`; the compile policy in `compiler`.

use std::collections::HashMap;

use bowties_core::{
    layout::{channels::ChannelRole, facilities::NodeLogicPlacement, state::LayoutState},
    node_key::NodeKey,
    node_tree::{parse_event_id_hex, NodeConfigTree},
    profile::StructureProfile,
    signaling::{
        compiler::{
            compile_block_signals, merge_release_edits, release_slots, BlockOccupancyEvents, CompileInput,
            CompileIssue, PlannedSignalResources, ReleaseTarget, SignalWiring,
        },
        slots::{
            claimed_slots_from_facilities, facility_claimed_slots, node_slot_facts, resolve_edits, LeafPathIndex,
            ResolvedEdit,
        },
        BlockSignalsRequest, ReleaseRequest,
    },
};
use serde::{Deserialize, Serialize};

use super::{proposal_facts_from_layout, resolved_detection_events, saved_signals_from_layout};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeBlockSignalsRequest {
    /// The facility being composed; its own claimed slots do not count as in use.
    #[serde(default)]
    pub facility_id: Option<String>,
    /// Removal mode: release the facility's claimed slots instead of compiling
    /// a recipe. Needs `facility_id`; `line` and `wiring` are ignored.
    #[serde(default)]
    pub remove: bool,
    #[serde(default)]
    pub line: BlockSignalsRequest,
    #[serde(default)]
    pub wiring: Vec<SignalWiring>,
    /// Untracked owners (a mast or Direct Lamp row no facility claims) the
    /// modeler released. Their reset edits are merged with the recipe's and
    /// their pins and slots count as free for the recipe.
    #[serde(default)]
    pub release: Vec<ReleaseRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeBlockSignalsResponse {
    pub edits: Vec<ResolvedEdit>,
    /// Each signal's mast and Track Circuits (empty on removal and on issues).
    pub signal_resources: Vec<PlannedSignalResources>,
    /// The whole new Logic placement of each affected board (empty on issues).
    pub logic_placement: Vec<NodeLogicPlacement>,
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
    mut request: ComposeBlockSignalsRequest,
) -> Result<ComposeBlockSignalsResponse, String> {
    if request.remove {
        return release_from_layout(layout, profiles, request.facility_id.as_deref());
    }
    request.line.saved_signals = request
        .facility_id
        .as_deref()
        .map(|id| saved_signals_from_layout(layout, id))
        .unwrap_or_default();
    // Named releases are applied inside the facts, so the pin owners, slot
    // facts and free-slot pick below all see the released configuration.
    let facts = proposal_facts_from_layout(layout, profiles, &request.release);
    let claimed = claimed_slots_from_facilities(layout.effective_facilities(), request.facility_id.as_deref());
    // Recompiling a saved facility: each signal keeps its mast, and the board's
    // planner keeps the facility's Logic groups where they are.
    let saved_resources = request
        .facility_id
        .as_deref()
        .and_then(|id| layout.effective_facilities().facilities.iter().find(|f| f.facility_id == id))
        .and_then(|f| f.block_signals.as_ref())
        .map(|record| {
            record
                .signals
                .iter()
                .map(|s| PlannedSignalResources { signal_id: s.signal_id.clone(), resources: s.resources.clone() })
                .collect()
        })
        .unwrap_or_default();

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
    // A release's edits must resolve too, so its node's tree is indexed even
    // when no signal is wired there.
    let node_keys = request.wiring.iter().map(|w| &w.node_key).chain(request.release.iter().map(|r| &r.node_key));
    for node_key in node_keys {
        let key = NodeKey::parse(node_key).map_err(|e| format!("Invalid node key '{node_key}': {e}"))?;
        if let Some(tree) = facts.trees.get(&key) {
            trees.insert(node_key.clone(), tree);
        }
    }
    let indexes: HashMap<String, LeafPathIndex<'_>> =
        trees.iter().map(|(k, t)| (k.clone(), LeafPathIndex::new(t))).collect();

    // The trees are the effective configuration (with any named release
    // applied), so slot facts already see unsaved drafts and pending offline
    // changes.
    let nodes = trees
        .iter()
        .map(|(node_key, tree)| node_slot_facts(node_key, tree, &claimed))
        .collect();

    let compiled = compile_block_signals(&CompileInput {
        line: request.line,
        wiring: request.wiring,
        hardware: facts.hardware.clone(),
        nodes,
        occupancy,
        saved_resources,
        logic_placement: layout.effective_facilities().logic_placement.clone(),
    });
    let mut issues = facts.release_issues.clone();
    issues.extend(compiled.issues);
    let (merged, signal_resources, logic_placement) = if issues.is_empty() {
        (
            merge_release_edits(facts.release_edits.clone(), compiled.edits),
            compiled.resources,
            compiled.placement,
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    let edits = resolve_edits(&indexes, &merged).map_err(|e| e.to_string())?;
    Ok(ComposeBlockSignalsResponse { edits, signal_resources, logic_placement, issues })
}

/// Removal mode: the edits that release the saved facility's masts and placed
/// Logic, built from the effective trees of the nodes that hold them, and each
/// affected board's placement without the facility's groups.
fn release_from_layout(
    layout: &LayoutState,
    profiles: &HashMap<String, Option<StructureProfile>>,
    facility_id: Option<&str>,
) -> Result<ComposeBlockSignalsResponse, String> {
    let facility_id = facility_id.ok_or("Removal needs a facility id.")?;
    let doc = layout.effective_facilities();
    doc.facilities
        .iter()
        .find(|f| f.facility_id == facility_id)
        .and_then(|f| f.block_signals.as_ref())
        .ok_or_else(|| format!("No Automatic Block Signals facility '{facility_id}' is saved."))?;
    let claims = facility_claimed_slots(doc, facility_id);
    let facts = proposal_facts_from_layout(layout, profiles, &[]);

    let mut trees: HashMap<String, &NodeConfigTree> = HashMap::new();
    for claim in &claims {
        let key = NodeKey::parse(&claim.node_key).map_err(|e| format!("Invalid node key '{}': {e}", claim.node_key))?;
        if let Some(tree) = facts.trees.get(&key) {
            trees.insert(claim.node_key.clone(), tree);
        }
    }
    let indexes: HashMap<String, LeafPathIndex<'_>> =
        trees.iter().map(|(k, t)| (k.clone(), LeafPathIndex::new(t))).collect();
    let nodes: Vec<_> = trees.iter().map(|(node_key, tree)| node_slot_facts(node_key, tree, &[])).collect();

    let targets: Vec<_> = claims.iter().map(ReleaseTarget::from).collect();
    let released = release_slots(&targets, &nodes);
    let edits = resolve_edits(&indexes, &released.edits).map_err(|e| e.to_string())?;
    let logic_placement = if released.issues.is_empty() {
        doc.logic_placement
            .iter()
            .filter(|board| board.groups.iter().any(|g| g.facility_id == facility_id))
            .map(|board| NodeLogicPlacement {
                node_key: board.node_key.clone(),
                groups: board.groups.iter().filter(|g| g.facility_id != facility_id).cloned().collect(),
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(ComposeBlockSignalsResponse {
        edits,
        signal_resources: Vec::new(),
        logic_placement,
        issues: released.issues,
    })
}
