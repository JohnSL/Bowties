//! `compose_block_signals` and facility-removal composition: IPC boundary: gathers compiler facts from the
//! effective layout (ADR-0015), compiles, and resolves the generated edits to
//! memory locations. Read-only: no drafts, claims, configuration writes or bus
//! activity. Pure fact extraction and path resolution live in
//! `bowties_core::signaling::slots`; the compile policy in `compiler`.

use std::collections::HashMap;

use bowties_core::{
    layout::{facilities::{ClaimedSlotKind, NodeLogicPlacement}, state::LayoutState},
    node_tree::ConfigValue,
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

use super::{index_effective_trees, occupancy_events, proposal_facts_from_layout, saved_signals_from_layout};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeBlockSignalsRequest {
    /// The facility being composed; its own claimed slots do not count as in use.
    #[serde(default)]
    pub facility_id: Option<String>,
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
    request.line.saved_signals = request
        .facility_id
        .as_deref()
        .map(|id| saved_signals_from_layout(layout, id))
        .unwrap_or_default();
    // Named releases are applied inside the facts, so the pin owners, slot
    // facts and free-slot pick below all see the released configuration.
    let facts = proposal_facts_from_layout(layout, profiles, &request.release);
    let claimed = claimed_slots_from_facilities(
        layout.effective_facilities(),
        layout.effective_channels(),
        request.facility_id.as_deref(),
    );
    // Recompiling a saved facility: each signal keeps its mast, and the board's
    // planner keeps the facility's Logic groups where they are.
    let saved_resources: Vec<PlannedSignalResources> = request
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

    let occupancy: Vec<BlockOccupancyEvents> =
        request.line.blocks.iter().map(|block| occupancy_events(&facts, &block.block_id)).collect();

    // A release's edits and a dropped signal's resets must resolve too, so
    // their nodes' trees are indexed even when no signal is wired there.
    let trees = index_effective_trees(
        &facts,
        request
            .wiring
            .iter()
            .map(|w| &w.node_key)
            .chain(request.release.iter().map(|r| &r.node_key))
            .chain(saved_resources.iter().map(|r| &r.resources.node_key)),
    )?;
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilityRemovalClaim {
    pub node_key: String,
    pub kind: &'static str,
    pub index: u32,
    pub lamp_selection: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeFacilityRemovalResponse {
    pub edits: Vec<ResolvedEdit>,
    pub logic_placement: Vec<NodeLogicPlacement>,
    pub issues: Vec<CompileIssue>,
    pub claims: Vec<FacilityRemovalClaim>,
}

pub async fn compose_removal(state: &AppState, facility_id: &str) -> Result<ComposeFacilityRemovalResponse, String> {
    let layout = state.layout_state.read().await;
    let Some(layout) = layout.as_ref() else {
        return Err("No layout is open.".to_string());
    };
    let profiles = state.profiles.read().await;
    compose_removal_from_layout(layout, &profiles, facility_id)
}

fn claim_kind_name(kind: ClaimedSlotKind) -> &'static str {
    match kind {
        ClaimedSlotKind::Mast => "mast",
        ClaimedSlotKind::Logic => "logic",
        ClaimedSlotKind::TrackCircuit => "trackCircuit",
        ClaimedSlotKind::DirectLamp => "directLamp",
    }
}

/// Facility removal, whatever its kind: the edits that return every slot the
/// facility claims (masts, placed Logic, Track Circuits, panel Direct Lamp rows)
/// to its unused state, built from the effective trees of the nodes that hold
/// them, each affected board's placement without the facility's groups, and the
/// claim list the edits were derived from (a Direct Lamp row carries its
/// effective Lamp Selection).
pub fn compose_removal_from_layout(
    layout: &LayoutState,
    profiles: &HashMap<String, Option<StructureProfile>>,
    facility_id: &str,
) -> Result<ComposeFacilityRemovalResponse, String> {
    let doc = layout.effective_facilities();
    if !doc.facilities.iter().any(|f| f.facility_id == facility_id) {
        return Err(format!("No facility '{facility_id}' is saved."));
    }
    let claimed = facility_claimed_slots(doc, layout.effective_channels(), facility_id);
    let facts = proposal_facts_from_layout(layout, profiles, &[]);

    let trees = index_effective_trees(&facts, claimed.iter().map(|c| &c.node_key))?;
    let indexes: HashMap<String, LeafPathIndex<'_>> =
        trees.iter().map(|(k, t)| (k.clone(), LeafPathIndex::new(t))).collect();
    let nodes: Vec<_> = trees.iter().map(|(node_key, tree)| node_slot_facts(node_key, tree, &[])).collect();

    let targets: Vec<_> = claimed.iter().map(ReleaseTarget::from).collect();
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
    let claims = claimed
        .iter()
        .map(|claim| {
            let lamp_selection = (claim.kind == ClaimedSlotKind::DirectLamp)
                .then(|| indexes.get(&claim.node_key))
                .flatten()
                .and_then(|index| index.leaf(&format!("Direct Lamp Control/Lamp[{}]/Lamp Selection", claim.index)))
                .and_then(|leaf| match &leaf.value {
                    Some(ConfigValue::Int { value }) if *value != 0 => Some(*value),
                    _ => None,
                });
            FacilityRemovalClaim {
                node_key: claim.node_key.clone(),
                kind: claim_kind_name(claim.kind),
                index: claim.index,
                lamp_selection,
            }
        })
        .collect();
    Ok(ComposeFacilityRemovalResponse { edits, logic_placement, issues: released.issues, claims })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::fixtures::*;
    use bowties_core::signaling::SignalOutputOwner;

    fn lamp_path(row: u32) -> String {
        format!("Direct Lamp Control/Lamp[{row}]/")
    }

    #[test]
    fn removing_a_panel_resets_exactly_its_claimed_lamp_rows_and_lists_them() {
        let mut tree = signal_lcc_tree();
        set_int(&mut tree, "Direct Lamp Control/Lamp[2]/Lamp Selection", 3);
        set_int(&mut tree, "Direct Lamp Control/Lamp[3]/Lamp Selection", 0);
        set_int(&mut tree, "Direct Lamp Control/Lamp[4]/Lamp Selection", 5);
        set_int(&mut tree, &mast_processing(1), 1);
        let layout = layout(
            tree,
            vec![lamp_channel("l2", 2), lamp_channel("l3", 3), lamp_channel("l4", 4)],
            vec![
                panel_facility("pnl-1", "Yard", &["l2", "l3"]),
                panel_facility("pnl-2", "Depot", &["l4"]),
                abs_facility("abs-1", "Main", 1),
            ],
        );

        let out = compose_removal_from_layout(&layout, &profiles(), "pnl-1").unwrap();
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        assert!(out.logic_placement.is_empty());
        assert!(out.edits.iter().any(|e| e.path.starts_with(&lamp_path(2))), "{:?}", out.edits);
        assert!(
            out.edits.iter().all(|e| e.path.starts_with(&lamp_path(2)) || e.path.starts_with(&lamp_path(3))),
            "{:?}",
            out.edits
        );
        let claims: Vec<_> = out.claims.iter().map(|c| (c.kind, c.index, c.lamp_selection)).collect();
        assert_eq!(claims, vec![("directLamp", 2, Some(3)), ("directLamp", 3, None)]);
    }

    #[test]
    fn removing_an_abs_facility_resets_its_mast_like_the_old_remove_mode() {
        let mut tree = signal_lcc_tree();
        set_int(&mut tree, &mast_processing(1), 1);
        set_int(&mut tree, &mast_lamp(1), 4);
        let layout = layout(tree, vec![], vec![abs_facility("abs-1", "Main", 1)]);

        let out = compose_removal_from_layout(&layout, &profiles(), "abs-1").unwrap();
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        assert!(out.edits.iter().any(|e| e.path == mast_processing(1)), "{:?}", out.edits);
        assert!(out.edits.iter().any(|e| e.path == mast_lamp(1)), "{:?}", out.edits);
        let claims: Vec<_> = out.claims.iter().map(|c| (c.kind, c.index, c.lamp_selection)).collect();
        assert_eq!(claims, vec![("mast", 1, None)]);
    }

    #[test]
    fn removing_an_unknown_facility_is_an_error() {
        let layout = layout(signal_lcc_tree(), vec![], vec![]);
        let err = compose_removal_from_layout(&layout, &profiles(), "nope").unwrap_err();
        assert!(err.contains("nope"), "{err}");
    }

    #[test]
    fn a_direct_lamp_row_held_by_a_panel_is_facility_owned_and_cannot_be_released_by_abs() {
        let mut tree = signal_lcc_tree();
        set_int(&mut tree, "Direct Lamp Control/Lamp[2]/Lamp Selection", 3);
        let layout = layout(tree, vec![lamp_channel("lamp-2", 2)], vec![panel_facility("pnl-1", "Yard Panel", &["lamp-2"])]);
        let release = vec![ReleaseRequest { node_key: BOARD.into(), owner: SignalOutputOwner::DirectLamp(2) }];

        let facts = proposal_facts_from_layout(&layout, &profiles(), &release);
        let host = &facts.hardware.hosts[0];
        assert_eq!(host.outputs[2].facility.as_ref().map(|f| f.facility_id.as_str()), Some("pnl-1"));

        let out = compose_from_layout(
            &layout,
            &profiles(),
            ComposeBlockSignalsRequest {
                facility_id: Some("abs-new".into()),
                line: Default::default(),
                wiring: vec![],
                release,
            },
        )
        .unwrap();
        assert_eq!(out.issues.len(), 1, "{:?}", out.issues);
        assert!(out.issues[0].detail.contains("owned by a facility and cannot be released here"));
        assert!(out.edits.is_empty());
    }
}
