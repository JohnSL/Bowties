//! Pure compiler for the Automatic Block Signals recipe across the Signal LCCs
//! (boards) a Line's signals are wired to.
//!
//! Turns a Line (the facility's block list and policy settings), the wiring
//! chosen for each of its signals and read-only node facts into configuration edits,
//! claimed mast/Logic slots and blocking issues. No I/O and no CDI
//! addresses: each edit is addressed by a CDI path for its node and carries a
//! typed value; the backend resolves paths to space/address.
//!
//! Path syntax: '/'-separated CDI element names. `Name[k]` selects instance
//! `k` (1-based) of a replicated group; `Name#k` selects the k-th (1-based)
//! sibling with that name (the Logic group has two `Action` groups; the
//! replicated one is `Action#2`).
//!
//! Slot "in use" criteria: a mast is in use when `Mast Processing != 0`
//! (Unused); a Logic is in use when `Function != 0` (Blocked). Unsaved edits
//! on the node override the base facts for those two leaves.
//!
//! Masts never move: a saved signal keeps the mast in its recorded resources.
//! Logic placement is board-wide and owned by [`super::logic_placement`]: the
//! recipe describes each signal's Logic relative to its group's first slot and
//! the planner supplies the slot.

use serde::{Deserialize, Serialize};

use super::{
    project_signal_binding, ApproachLighting, BlockSignalsRequest, EntranceEnd, ExitEnd, Movements,
    NextSignal, ProposalIssue, SignalBindingSelection, SignalHardwareFacts, SignalOutputOwner, SignalPreview,
};
use crate::layout::channels::{ChannelBinding, ChannelRole};
use super::logic_placement::{
    plan_logic_placement, LogicGroupDemand, LogicPlacementIssue, LogicPlacementIssueCode, LogicSlotFact, PlannedLogic,
};
use crate::layout::facilities::{
    canonical_node_key, ClaimedSlot, ClaimedSlotKind, NodeLogicPlacement, PlacedLogicGroup, SignalResources,
};

/// Style the compiled recipe realises.
pub const TWO_DRIVER_RG_STYLE: &str = "two-driver-rg-signal";

/// A typed configuration value.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EditValue {
    Int { value: i64 },
    EventId { bytes: [u8; 8] },
    String { value: String },
    /// A new event ID for the caller to generate, so the leaf no longer shares
    /// an event with anything. Only `release_slots` produces it; the frontend
    /// owns the single event-ID generator and swaps it for an `EventId`.
    FreshEventId,
}

/// One configuration edit on a node, addressed by CDI path.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfigEdit {
    pub node_key: String,
    pub path: String,
    pub value: EditValue,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MastFact {
    /// 1-based mast number.
    pub index: u32,
    /// Processing is not Unused, or another facility claims the mast.
    pub in_use: bool,
    /// A facility's claimed slot (as opposed to configuration found on the
    /// board that no facility owns).
    #[serde(default)]
    pub claimed: bool,
    /// The `set aspect` event of every Rule on the mast, in rule order
    /// (the recipe uses Rules 1, 2 and 3).
    pub set_aspect_events: Vec<Option<[u8; 8]>>,
    /// The mast's `Link Address` event as read from the board; `None` when unread.
    #[serde(default)]
    pub link_address: Option<[u8; 8]>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicFact {
    /// 1-based Logic number.
    pub index: u32,
    /// Function is not Blocked, or another facility claims the Logic.
    pub in_use: bool,
    /// The raw Function value; `None` when it has not been read.
    #[serde(default)]
    pub function: Option<i64>,
    /// A facility's claimed slot (as opposed to configuration found on the
    /// board that no facility owns).
    #[serde(default)]
    pub claimed: bool,
    /// The Logic's `Action#2` rows, in order.
    #[serde(default)]
    pub actions: Vec<LogicAction>,
    /// The raw `Variable #1/Source` and `Variable #2/Source` values that were read
    /// (1..=8 name Track Circuits 1..=8; 0 is the Logic's own events).
    #[serde(default)]
    pub sources: Vec<i64>,
}

/// One Track Circuit of a board.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackCircuitFact {
    /// 1-based circuit number.
    pub index: u32,
    /// A facility claims it, or active Logic Bowties does not own uses it as a Source.
    pub in_use: bool,
    /// A facility claims the circuit.
    #[serde(default)]
    pub claimed: bool,
    /// The circuit's `Link Address` as read; `None` when unread.
    #[serde(default)]
    pub link_address: Option<[u8; 8]>,
}

/// One of a Logic's four event-sending actions.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicAction {
    /// The action's Condition; 0 means it sends nothing. `None` when unread.
    pub condition: Option<i64>,
    pub event: Option<[u8; 8]>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeSlotFacts {
    pub node_key: String,
    pub masts: Vec<MastFact>,
    pub logic: Vec<LogicFact>,
    /// 1-based Direct Lamp Control rows present on the node.
    #[serde(default)]
    pub direct_lamps: Vec<u32>,
    #[serde(default)]
    pub track_circuits: Vec<TrackCircuitFact>,
}

/// The protected block's detector event IDs.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockOccupancyEvents {
    pub block_id: String,
    pub occupied: Option<[u8; 8]>,
    pub clear: Option<[u8; 8]>,
}

/// The red/green wiring chosen for one signal.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalWiring {
    pub signal_id: String,
    pub node_key: String,
    pub red_output: u32,
    pub green_output: u32,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileInput {
    pub line: BlockSignalsRequest,
    pub wiring: Vec<SignalWiring>,
    /// Output hosts and owners, as `project_signal_binding` expects.
    pub hardware: SignalHardwareFacts,
    pub nodes: Vec<NodeSlotFacts>,
    pub occupancy: Vec<BlockOccupancyEvents>,
    /// The resources the facility's saved signals already hold. A saved
    /// signal keeps its mast (masts never move) and its pins are not reported
    /// as in use. Empty when creating (lowest free mast).
    #[serde(default)]
    pub saved_resources: Vec<PlannedSignalResources>,
    /// Every board's recorded Logic placement, of every facility. The board's
    /// planner keeps this facility's groups where they are.
    #[serde(default)]
    pub logic_placement: Vec<NodeLogicPlacement>,
}

/// The board resources one signal uses.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSignalResources {
    pub signal_id: String,
    pub resources: SignalResources,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompileIssueCode {
    WiringMissing,
    BindingInvalid,
    HardwareUnavailable,
    StyleUnsupported,
    OutputDuplicate,
    OutputInUse,
    OutputCapacity,
    NoFreeMast,
    NoFreeLogic,
    ClaimedSlotUnavailable,
    /// Logic outside Bowties has a Function value the CDI does not define.
    LogicUndefinedFunction,
    /// Logic outside Bowties has a run of `Group` entries with no `Last (Single)`.
    LogicUnterminatedGroup,
    /// A Logic's Function has not been read.
    LogicUnread,
    /// A requested release names an owner that is facility-owned, not an
    /// owner, or not on a loaded node.
    ReleaseInvalid,
    OccupancyEventsMissing,
    AspectEventsMissing,
    NoFreeTrackCircuit,
    LinkAddressMissing,
    UnsupportedMovements,
    UnsupportedEnds,
    UnsupportedAspects,
    UnsupportedApproachLighting,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompileIssue {
    pub code: CompileIssueCode,
    pub signal_id: Option<String>,
    pub node_key: Option<String>,
    pub detail: String,
    /// The underlying binding issue when it came from the shared check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_issue: Option<ProposalIssue>,
    /// The Logic numbers a planner issue names; empty for every other issue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logic: Vec<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompiledBlockSignals {
    pub edits: Vec<NodeConfigEdit>,
    /// Each signal's mast and Track Circuits.
    pub resources: Vec<PlannedSignalResources>,
    /// The whole new Logic placement of each wiring board (one entry per board; empty with issues).
    pub placement: Vec<NodeLogicPlacement>,
    pub issues: Vec<CompileIssue>,
}

/// Compile the Automatic Block Signals recipe for every signal of a Line, each
/// on the board its wiring names. Deterministic: signals, slots and edits are
/// chosen and ordered from the inputs alone, lowest free slot first, in chain
/// order. Slots are claimed per board. A signal with a
/// signal after it gets a two-rung group and a Track Circuit; the last signal
/// gets the terminal recipe. With any issue, `edits`, `resources` and
/// `placement` are empty.
pub fn compile_block_signals(input: &CompileInput) -> CompiledBlockSignals {
    let mut out = CompiledBlockSignals::default();
    let mut ctx = IssueSink { issues: &mut out.issues, signal_id: None, node_key: None };
    let mut signals = super::chain::derive_signals(&input.line, &mut Vec::new());
    ctx.signal_id = signals.first().map(|s| s.signal_id.clone());
    check_scope(input, &signals, &mut ctx);
    if signals.is_empty() {
        return out;
    }
    let facility_id = input.line.facility_id.clone();
    let wiring_of = |signal_id: &str| input.wiring.iter().find(|w| w.signal_id == signal_id);
    if !signals.iter().any(|s| wiring_of(&s.signal_id).is_some()) {
        for signal in &signals {
            ctx.signal_id = Some(signal.signal_id.clone());
            ctx.push(CompileIssueCode::WiringMissing, "No Signal LCC outputs are chosen for this signal.");
        }
        return out;
    }
    let node_of = |key: &str| input.nodes.iter().find(|n| same_node(&n.node_key, key));
    let saved_of = |signal_id: &str| {
        input.saved_resources.iter().find(|r| r.signal_id == signal_id).map(|r| &r.resources)
    };
    // Saved signals keep their mast and circuit, so new signals never take them.
    // Masts and circuits are numbered per board.
    let mut taken_masts: Vec<(String, u32)> = signals
        .iter()
        .filter_map(|s| saved_of(&s.signal_id))
        .map(|r| (canonical_node_key(&r.node_key), r.mast))
        .collect();
    let mut taken_circuits: Vec<(String, u32)> = signals
        .iter()
        .filter_map(|s| saved_of(&s.signal_id))
        .flat_map(|r| r.track_circuits.iter().map(|c| (canonical_node_key(&r.node_key), *c)))
        .collect();
    let taken_on = |taken: &[(String, u32)], board: &str| -> Vec<u32> {
        taken.iter().filter(|(b, _)| *b == canonical_node_key(board)).map(|(_, i)| *i).collect()
    };

    // Saved signals this edit no longer derives: their mast and Track Circuits
    // are released, and their Logic group is vacated by the board planner.
    let dropped: Vec<&PlannedSignalResources> = input
        .saved_resources
        .iter()
        .filter(|r| !signals.iter().any(|s| s.signal_id == r.signal_id))
        .collect();

    let mut plans: Vec<SignalPlan> = Vec::new();
    for signal in signals.iter_mut() {
        ctx.signal_id = Some(signal.signal_id.clone());
        ctx.node_key = None;
        let Some(wiring) = wiring_of(&signal.signal_id) else {
            ctx.push(CompileIssueCode::WiringMissing, "No Signal LCC outputs are chosen for this signal.");
            continue;
        };
        ctx.node_key = Some(wiring.node_key.clone());
        let node = node_of(&wiring.node_key);
        let own = saved_of(&signal.signal_id);
        let held_mast = own.filter(|r| same_node(&r.node_key, &wiring.node_key)).map(|r| r.mast);
        // Every mast this facility saved on the board counts as its own for the
        // pin check, so a pin freed by a dropped or re-wired signal is reusable.
        let own_masts: Vec<u32> = input
            .saved_resources
            .iter()
            .filter(|r| same_node(&r.resources.node_key, &wiring.node_key))
            .map(|r| r.resources.mast)
            .collect();
        check_binding(signal, wiring, &input.hardware, &own_masts, &mut ctx);

        let mast = if own.is_some() {
            let mast = held_mast.and_then(|i| node.and_then(|n| n.masts.iter().find(|m| m.index == i)));
            if mast.is_none() {
                ctx.push(
                    CompileIssueCode::ClaimedSlotUnavailable,
                    "The mast this facility already uses is not available on the chosen Signal LCC.",
                );
            }
            mast
        } else {
            let mast = node.and_then(|n| free_mast(n, &taken_on(&taken_masts, &wiring.node_key)));
            match mast {
                Some(m) => taken_masts.push((canonical_node_key(&wiring.node_key), m.index)),
                None => {
                    let outside = node.map_or(0, |n| n.masts.iter().filter(|m| m.in_use && !m.claimed).count());
                    ctx.push(CompileIssueCode::NoFreeMast, &no_free_detail("mast", outside));
                }
            }
            mast
        };
        let next_signal_id = match &signal.next {
            NextSignal::Signal { signal_id } => Some(signal_id.clone()),
            _ => None,
        };
        plans.push(SignalPlan {
            signal_id: signal.signal_id.clone(),
            name: signal.name.clone(),
            wiring: wiring.clone(),
            protected_block_ids: signal.protected_block_ids.clone(),
            mast: mast.map(|m| m.index),
            next_signal_id,
            circuit: None,
            released_circuits: Vec::new(),
        });
    }

    // Track Circuits and the next signal's Link Address, once every mast is chosen.
    // The circuit is on the signal's own board; the Link Address is read from the next signal's board.
    for i in 0..plans.len() {
        ctx.signal_id = Some(plans[i].signal_id.clone());
        ctx.node_key = Some(plans[i].wiring.node_key.clone());
        let node = node_of(&plans[i].wiring.node_key);
        let saved = saved_of(&plans[i].signal_id).map(|r| r.track_circuits.clone()).unwrap_or_default();
        let Some(next_id) = plans[i].next_signal_id.clone() else {
            plans[i].released_circuits = saved;
            continue;
        };
        let circuit = match saved.first() {
            Some(held) => {
                if !node.is_some_and(|n| n.track_circuits.iter().any(|c| c.index == *held)) {
                    ctx.push(
                        CompileIssueCode::ClaimedSlotUnavailable,
                        "The Track Circuit this facility already uses is not available on the chosen Signal LCC.",
                    );
                }
                plans[i].released_circuits = saved[1..].to_vec();
                Some(*held)
            }
            None => {
                let taken = taken_on(&taken_circuits, &plans[i].wiring.node_key);
                let free = node.and_then(|n| {
                    n.track_circuits
                        .iter()
                        .filter(|c| !c.in_use && !taken.contains(&c.index))
                        .map(|c| c.index)
                        .min()
                });
                match free {
                    Some(c) => taken_circuits.push((canonical_node_key(&plans[i].wiring.node_key), c)),
                    None => {
                        let outside =
                            node.map_or(0, |n| n.track_circuits.iter().filter(|c| c.in_use && !c.claimed).count());
                        ctx.push(CompileIssueCode::NoFreeTrackCircuit, &no_free_detail("Track Circuit", outside));
                    }
                }
                free
            }
        };
        plans[i].circuit = circuit;
        let next = plans.iter().find(|p| p.signal_id == next_id);
        if let Some((next_mast, next_board)) = next.and_then(|p| Some((p.mast?, p.wiring.node_key.as_str()))) {
            if link_address_of(node_of(next_board), next_mast).is_none() {
                let next_name = signals.iter().find(|s| s.signal_id == next_id).map_or("the next signal", |s| s.name.as_str());
                ctx.push(
                    CompileIssueCode::LinkAddressMissing,
                    &format!("The Link Address of {next_name}'s mast has not been read from board {next_board}."),
                );
            }
        }
    }

    // Occupancy and set-aspect events per signal.
    for plan in &plans {
        ctx.signal_id = Some(plan.signal_id.clone());
        ctx.node_key = Some(plan.wiring.node_key.clone());
        if plan.occupancy(input).is_none() {
            ctx.push(
                CompileIssueCode::OccupancyEventsMissing,
                "A protected block has no occupied and clear event IDs.",
            );
        }
        if let Some(mast) = plan.mast {
            if plan.aspect_events(node_of(&plan.wiring.node_key), mast).is_none() {
                let wanted = if plan.next_signal_id.is_some() { "Stop, Approach and Clear" } else { "Stop and Clear" };
                ctx.push(
                    CompileIssueCode::AspectEventsMissing,
                    &format!("The chosen mast's {wanted} set-aspect events are not available."),
                );
            }
        }
    }

    // One planner call per board, one group per signal wired to it.
    // A board held only by dropped signals is planned too, so its group is vacated.
    let mut planned_boards = board_keys(&plans);
    for saved in &dropped {
        if !planned_boards.iter().any(|k| same_node(k, &saved.resources.node_key)) {
            planned_boards.push(saved.resources.node_key.clone());
        }
    }
    let boards: Vec<BoardLogic> = planned_boards
        .into_iter()
        .map(|board_key| plan_board_logic(input, &plans, &board_key, &mut ctx))
        .collect();
    let group_of = |plan: &SignalPlan| {
        boards
            .iter()
            .find(|b| same_node(&b.key, &plan.wiring.node_key))?
            .planned
            .placement
            .iter()
            .find(|g| g.facility_id == facility_id && g.signal_id == plan.signal_id)
            .cloned()
    };
    for plan in &plans {
        let Some(group) = group_of(plan) else { continue };
        let missing = (group.first..group.first + group.length)
            .any(|i| !node_of(&plan.wiring.node_key).is_some_and(|n| n.logic.iter().any(|l| l.index == i)));
        if missing {
            ctx.signal_id = Some(plan.signal_id.clone());
            ctx.node_key = Some(plan.wiring.node_key.clone());
            ctx.push(
                CompileIssueCode::ClaimedSlotUnavailable,
                "The Logic this facility already uses is not available on the chosen Signal LCC.",
            );
        }
    }

    if !out.issues.is_empty() {
        return out;
    }
    let mut kept_logic: Vec<(String, u32)> = Vec::new();
    for plan in &plans {
        let node = node_of(&plan.wiring.node_key);
        let (Some(mast), Some(group), Some(occupancy)) = (plan.mast, group_of(plan), plan.occupancy(input))
        else {
            return CompiledBlockSignals::default();
        };
        let Some(events) = plan.aspect_events(node, mast) else { return CompiledBlockSignals::default() };
        let board = canonical_node_key(&plan.wiring.node_key);
        kept_logic.extend((group.first..group.first + group.length).map(|i| (board.clone(), i)));
        let mut writes = mast_edits(&plan.name, mast, &plan.wiring);
        match (&plan.next_signal_id, plan.circuit) {
            (Some(next_id), Some(circuit)) => {
                let next = signals.iter().find(|s| &s.signal_id == next_id);
                let link = plans
                    .iter()
                    .find(|p| &p.signal_id == next_id)
                    .and_then(|p| link_address_of(node_of(&p.wiring.node_key), p.mast?));
                let (Some(link), Some(next), Some(approach)) = (link, next, events.approach) else {
                    return CompiledBlockSignals::default();
                };
                let internal = InternalRecipe {
                    name: &plan.name,
                    next_name: &next.name,
                    first_logic: group.first,
                    circuit,
                    link,
                    occupancy: &occupancy,
                    stop: events.stop,
                    approach,
                    clear: events.clear,
                };
                writes.extend(internal_edits(&internal));
            }
            _ => writes.extend(terminal_edits(&plan.name, group.first, &occupancy, events.stop, events.clear)),
        }
        out.edits.extend(writes.into_iter().map(|(path, value)| NodeConfigEdit {
            node_key: plan.wiring.node_key.clone(),
            path,
            value,
        }));
        out.resources.push(PlannedSignalResources {
            signal_id: plan.signal_id.clone(),
            resources: SignalResources {
                node_key: plan.wiring.node_key.clone(),
                mast,
                track_circuits: plan.circuit.into_iter().collect(),
            },
        });
    }
    // Vacated Logic and released circuits are reset, on the board that held them,
    // after the new groups are written.
    for board in &boards {
        let board_id = canonical_node_key(&board.key);
        for logic in board.planned.vacated.iter().filter(|i| !kept_logic.contains(&(board_id.clone(), **i))) {
            out.edits.extend(logic_reset_edits(*logic).into_iter().map(|(path, value)| NodeConfigEdit {
                node_key: board.key.clone(),
                path,
                value,
            }));
        }
    }
    for plan in &plans {
        for circuit in &plan.released_circuits {
            out.edits.extend(track_circuit_reset_edits(*circuit).into_iter().map(|(path, value)| NodeConfigEdit {
                node_key: plan.wiring.node_key.clone(),
                path,
                value,
            }));
        }
    }
    out.edits.extend(dropped_resets(&dropped, &plans, &node_of));
    out.placement = boards
        .into_iter()
        .map(|b| NodeLogicPlacement { node_key: b.key, groups: b.planned.placement })
        .collect();
    out
}

/// Resets of the masts and Track Circuits of saved signals that are no longer
/// derived, on the board that held them. A slot a surviving signal keeps is
/// never reset.
fn dropped_resets<'a>(
    dropped: &[&PlannedSignalResources],
    plans: &[SignalPlan],
    node_of: &impl Fn(&str) -> Option<&'a NodeSlotFacts>,
) -> Vec<NodeConfigEdit> {
    let kept_mast = |board: &str, mast: u32| {
        plans.iter().any(|p| same_node(&p.wiring.node_key, board) && p.mast == Some(mast))
    };
    let kept_circuit = |board: &str, circuit: u32| {
        plans.iter().any(|p| {
            same_node(&p.wiring.node_key, board) && (p.circuit == Some(circuit) || p.released_circuits.contains(&circuit))
        })
    };
    let mut edits = Vec::new();
    for saved in dropped {
        let board = &saved.resources.node_key;
        let mut writes: Vec<(String, EditValue)> = Vec::new();
        if !kept_mast(board, saved.resources.mast) {
            let rules = node_of(board)
                .and_then(|n| n.masts.iter().find(|m| m.index == saved.resources.mast))
                .map_or(RECIPE_RULES, |m| m.set_aspect_events.len() as u32);
            writes.extend(mast_reset_edits(saved.resources.mast, rules));
        }
        for circuit in saved.resources.track_circuits.iter().filter(|c| !kept_circuit(board, **c)) {
            writes.extend(track_circuit_reset_edits(*circuit));
        }
        edits.extend(writes.into_iter().map(|(path, value)| NodeConfigEdit { node_key: board.clone(), path, value }));
    }
    edits
}

/// One wiring board's Logic placement.
struct BoardLogic {
    key: String,
    planned: PlannedLogic,
}

/// The boards the planned signals are wired to, in first-use order.
fn board_keys(plans: &[SignalPlan]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for plan in plans {
        if !keys.iter().any(|k| same_node(k, &plan.wiring.node_key)) {
            keys.push(plan.wiring.node_key.clone());
        }
    }
    keys
}

/// Plan one board's Logic: one group per signal wired to it, mapping planner issues to compile issues.
fn plan_board_logic(
    input: &CompileInput,
    plans: &[SignalPlan],
    board_key: &str,
    ctx: &mut IssueSink<'_>,
) -> BoardLogic {
    let node = input.nodes.iter().find(|n| same_node(&n.node_key, board_key));
    let on_board: Vec<&SignalPlan> = plans.iter().filter(|p| same_node(&p.wiring.node_key, board_key)).collect();
    ctx.signal_id = on_board.first().map(|p| p.signal_id.clone());
    ctx.node_key = Some(board_key.to_string());
    let recorded: Vec<PlacedLogicGroup> = input
        .logic_placement
        .iter()
        .filter(|board| same_node(&board.node_key, board_key))
        .flat_map(|board| board.groups.iter().cloned())
        .collect();
    let slot_facts: Vec<LogicSlotFact> = node
        .map(|n| n.logic.iter().map(|l| LogicSlotFact { index: l.index, function: l.function }).collect())
        .unwrap_or_default();
    let demand: Vec<LogicGroupDemand> = on_board
        .iter()
        .map(|p| LogicGroupDemand {
            signal_id: p.signal_id.clone(),
            length: p.logic_length(),
        })
        .collect();
    let planned = plan_logic_placement(&slot_facts, &recorded, &input.line.facility_id, &demand);
    let outside_logic = node.map_or(0, |n| n.logic.iter().filter(|l| l.in_use && !l.claimed).count());
    for issue in &planned.issues {
        let (code, detail) = map_placement_issue(issue, outside_logic);
        let board_wide = ctx.signal_id.clone();
        if issue.signal_id.is_some() {
            ctx.signal_id = issue.signal_id.clone();
        }
        ctx.push_logic(code, &detail, issue.logic.clone());
        ctx.signal_id = board_wide;
    }
    BoardLogic { key: board_key.to_string(), planned }
}

/// One signal's choices while a Line compiles.
struct SignalPlan {
    signal_id: String,
    name: String,
    wiring: SignalWiring,
    protected_block_ids: Vec<String>,
    mast: Option<u32>,
    next_signal_id: Option<String>,
    circuit: Option<u32>,
    /// Saved Track Circuits this signal no longer needs.
    released_circuits: Vec<u32>,
}

impl SignalPlan {
    /// The occupied and clear events of every protected block, in protected order.
    /// `None` when any protected block lacks usable events.
    fn occupancy(&self, input: &CompileInput) -> Option<Vec<([u8; 8], [u8; 8])>> {
        self.protected_block_ids
            .iter()
            .map(|id| {
                let b = input.occupancy.iter().find(|b| &b.block_id == id)?;
                Some((usable(b.occupied)?, usable(b.clear)?))
            })
            .collect()
    }

    /// Logic rungs the signal needs: one per protected block, plus the Track
    /// Circuit rung when a signal follows.
    fn logic_length(&self) -> u32 {
        self.protected_block_ids.len().max(1) as u32 + u32::from(self.next_signal_id.is_some())
    }

    /// The mast's set-aspect events: Stop and Clear always, Approach too when a signal follows.
    fn aspect_events(&self, node: Option<&NodeSlotFacts>, mast: u32) -> Option<AspectEvents> {
        let events = &node?.masts.iter().find(|m| m.index == mast)?.set_aspect_events;
        let at = |i: usize| usable(events.get(i).copied().flatten());
        let approach = at(1);
        if self.next_signal_id.is_some() && approach.is_none() {
            return None;
        }
        Some(AspectEvents { stop: at(0)?, approach, clear: at(2)? })
    }
}

/// A mast's Stop, Approach and Clear set-aspect events.
struct AspectEvents {
    stop: [u8; 8],
    approach: Option<[u8; 8]>,
    clear: [u8; 8],
}

/// The mast's `Link Address` as read from the board, when it is a real event ID.
fn link_address_of(node: Option<&NodeSlotFacts>, mast: u32) -> Option<[u8; 8]> {
    usable(node?.masts.iter().find(|m| m.index == mast)?.link_address)
}

/// Whether two node keys name the same node (dotted and undotted forms alike).
fn same_node(a: &str, b: &str) -> bool {
    canonical_node_key(a) == canonical_node_key(b)
}

/// Map a planner issue to a compile issue whose detail names the Logic.
fn map_placement_issue(issue: &LogicPlacementIssue, outside_logic: usize) -> (CompileIssueCode, String) {
    let names = |logic: &[u32]| {
        let list: Vec<String> = logic.iter().map(u32::to_string).collect();
        format!(" (Logic {})", list.join(", "))
    };
    match issue.code {
        LogicPlacementIssueCode::NoFreeLogic => {
            (CompileIssueCode::NoFreeLogic, no_free_detail("Logic conditional", outside_logic))
        }
        LogicPlacementIssueCode::UndefinedFunction => {
            (CompileIssueCode::LogicUndefinedFunction, issue.detail.clone())
        }
        LogicPlacementIssueCode::UnterminatedGroup => {
            (CompileIssueCode::LogicUnterminatedGroup, format!("{}{}", issue.detail, names(&issue.logic)))
        }
        LogicPlacementIssueCode::LogicUnread => {
            (CompileIssueCode::LogicUnread, format!("{}{}", issue.detail, names(&issue.logic)))
        }
    }
}

/// "The board has no free mast", naming how many slots are configured
/// outside Bowties when any are.
fn no_free_detail(slot: &str, outside: usize) -> String {
    match outside {
        0 => format!("The board has no free {slot}."),
        1 => format!("The board has no free {slot}; 1 is configured outside Bowties."),
        n => format!("The board has no free {slot}; {n} are configured outside Bowties."),
    }
}

struct IssueSink<'a> {
    issues: &'a mut Vec<CompileIssue>,
    signal_id: Option<String>,
    node_key: Option<String>,
}

impl IssueSink<'_> {
    fn push(&mut self, code: CompileIssueCode, detail: &str) {
        self.push_binding(code, detail, None);
    }

    fn push_binding(&mut self, code: CompileIssueCode, detail: &str, binding_issue: Option<ProposalIssue>) {
        self.issues.push(CompileIssue {
            code,
            signal_id: self.signal_id.clone(),
            node_key: self.node_key.clone(),
            detail: detail.to_string(),
            binding_issue,
            logic: Vec::new(),
        });
    }

    fn push_logic(&mut self, code: CompileIssueCode, detail: &str, logic: Vec<u32>) {
        self.push(code, detail);
        if let Some(issue) = self.issues.last_mut() {
            issue.logic = logic;
        }
    }
}

/// An event ID the node can really carry; all-zero means unset.
fn usable(event: Option<[u8; 8]>) -> Option<[u8; 8]> {
    event.filter(|bytes| bytes.iter().any(|b| *b != 0))
}

fn check_scope(input: &CompileInput, signals: &[SignalPreview], ctx: &mut IssueSink<'_>) {
    let line = &input.line;
    if line.movements != Movements::OneDirection {
        ctx.push(CompileIssueCode::UnsupportedMovements, "This release supports one direction only.");
    }
    if line.entrance_end != EntranceEnd::NoPlant || line.exit_end != ExitEnd::NothingBeyond {
        ctx.push(CompileIssueCode::UnsupportedEnds, "This release supports a Line with nothing beyond either end.");
    }
    let first = ctx.signal_id.clone();
    for signal in signals {
        ctx.signal_id = Some(signal.signal_id.clone());
        let (aspects, lighting) = line
            .signal_overrides
            .iter()
            .find(|o| o.signal_id == signal.signal_id)
            .map(|o| (o.aspects, o.approach_lighting.clone()))
            .unwrap_or((line.settings.aspects, line.settings.approach_lighting.clone()));
        if aspects != 3 {
            ctx.push(CompileIssueCode::UnsupportedAspects, "This release supports three indications only.");
        }
        if lighting != ApproachLighting::AlwaysLit {
            ctx.push(
                CompileIssueCode::UnsupportedApproachLighting,
                "This release supports signals that are always lit only.",
            );
        }
    }
    ctx.signal_id = first;
}

/// Reuse the shared binding check; map its issues to stable codes.
fn check_binding(
    signal: &mut SignalPreview,
    wiring: &SignalWiring,
    hardware: &SignalHardwareFacts,
    own_masts: &[u32],
    ctx: &mut IssueSink<'_>,
) {
    let selection = SignalBindingSelection {
        signal_id: signal.signal_id.clone(),
        role: ChannelRole::SignalIndication,
        style: TWO_DRIVER_RG_STYLE.to_string(),
        binding: ChannelBinding::SignalLccRgOutputs {
            node_key: wiring.node_key.clone(),
            green_output: wiring.green_output,
            red_output: wiring.red_output,
        },
    };
    let mut issues = Vec::new();
    project_signal_binding(signal, &selection, hardware, own_masts, &mut issues);
    for issue in issues {
        let (code, detail) = match &issue {
            ProposalIssue::InvalidSignalBinding { reason, .. } => (CompileIssueCode::BindingInvalid, reason.clone()),
            ProposalIssue::UnavailableSignalHardware { reason, .. } => {
                (CompileIssueCode::HardwareUnavailable, reason.clone())
            }
            ProposalIssue::UnsupportedSignalStyle { style, .. } => {
                (CompileIssueCode::StyleUnsupported, format!("The board does not support style {style}."))
            }
            ProposalIssue::DuplicateSignalOutputs { output, .. } => {
                (CompileIssueCode::OutputDuplicate, format!("Red and green both use output {output}."))
            }
            ProposalIssue::SignalOutputConflict { output, owner, .. } => {
                (CompileIssueCode::OutputInUse, format!("Output {output} is already used by {owner}."))
            }
            ProposalIssue::InsufficientSignalOutputCapacity { required, available, .. } => (
                CompileIssueCode::OutputCapacity,
                format!("Needs {required} free outputs; {available} available."),
            ),
            other => (CompileIssueCode::BindingInvalid, format!("{other:?}")),
        };
        ctx.push_binding(code, &detail, Some(issue));
    }
}

/// Path of a mast's group; every leaf the recipe or a release touches lies under it.
fn mast_path(mast: u32) -> String {
    format!("Rule to Aspect/Mast[{mast}]")
}

/// Path of a Logic's group; every leaf the recipe or a release touches lies under it.
fn logic_path(logic: u32) -> String {
    format!("Conditionals/Logic[{logic}]")
}

/// Path of a mast's processing leaf, for callers building pending edits.
pub fn mast_processing_path(mast: u32) -> String {
    format!("{}/Mast Processing", mast_path(mast))
}

/// Path of a Logic's function leaf, for callers building pending edits.
pub fn logic_function_path(logic: u32) -> String {
    format!("{}/Function", logic_path(logic))
}

/// Path of a mast's `Link Address` leaf, for callers reading board facts.
pub fn mast_link_address_path(mast: u32) -> String {
    format!("{}/Link Address", mast_path(mast))
}

/// Path of a Logic Variable's `Source` leaf (`variable` is 1 or 2).
pub fn logic_source_path(logic: u32, variable: u32) -> String {
    format!("{}/Variable #{variable}/Source", logic_path(logic))
}

/// Path of a Track Circuit's `Link Address` leaf.
pub(crate) fn track_circuit_link_path(circuit: u32) -> String {
    format!("{}/Link Address", track_circuit_path(circuit))
}

/// Rules the recipe writes on a mast (Stop, Approach, Clear).
const RECIPE_RULES: u32 = 3;
/// Appearances per rule, each with one `Lamp Selection`.
const RECIPE_APPEARANCES: u32 = 4;

fn rule_path(mast: u32, rule: u32) -> String {
    format!("{}/Rule[{rule}]", mast_path(mast))
}

fn appearance_lamp_path(mast: u32, rule: u32, appearance: u32) -> String {
    format!("{}/Appearance[{appearance}]/Lamp Selection", rule_path(mast, rule))
}

fn effects_lamp_path(mast: u32, rule: u32) -> String {
    format!("{}/Effects Lamp", rule_path(mast, rule))
}

fn occupied_event_path(logic: u32) -> String {
    format!("{}/Variable #1/set true", logic_path(logic))
}

fn clear_event_path(logic: u32) -> String {
    format!("{}/Variable #1/set false", logic_path(logic))
}

/// Every event-ID leaf on a Logic: Variable #1 and #2 set true/false and the
/// four actions' events. A superset of what the recipe writes.
fn logic_event_paths(logic: u32) -> Vec<String> {
    let l = logic_path(logic);
    let mut paths = vec![
        occupied_event_path(logic),
        clear_event_path(logic),
        format!("{l}/Variable #2/set true"),
        format!("{l}/Variable #2/set false"),
    ];
    paths.extend((1..=LOGIC_ACTIONS).map(|action| action_event_path(logic, action)));
    paths
}

/// Actions on a Logic (`Action#2` rows).
const LOGIC_ACTIONS: u32 = 4;

fn action_event_path(logic: u32, action: u32) -> String {
    format!("{}/Action#2[{action}]/Action Event", logic_path(logic))
}

/// Path of one Track Circuit group (1-based).
pub(crate) fn track_circuit_path(circuit: u32) -> String {
    format!("Track Circuits/Circuit[{circuit}]")
}

fn direct_lamp_path(lamp: u32) -> String {
    format!("Direct Lamp Control/Lamp[{lamp}]")
}

/// Every leaf a mast release resets: Processing and Description, then each
/// of `rules` rules' Appearance lamps and Effects Lamp.
fn mast_reset_edits(mast: u32, rules: u32) -> Vec<(String, EditValue)> {
    let mut writes = vec![
        (mast_processing_path(mast), EditValue::Int { value: 0 }),
        (format!("{}/Mast Description", mast_path(mast)), EditValue::String { value: String::new() }),
    ];
    for rule in 1..=rules {
        for appearance in 1..=RECIPE_APPEARANCES {
            writes.push((appearance_lamp_path(mast, rule, appearance), EditValue::Int { value: 0 }));
        }
        writes.push((effects_lamp_path(mast, rule), EditValue::Int { value: 0 }));
    }
    writes
}

fn logic_reset_edits(logic: u32) -> Vec<(String, EditValue)> {
    let mut writes = vec![
        (logic_function_path(logic), EditValue::Int { value: 0 }),
        (format!("{}/Description", logic_path(logic)), EditValue::String { value: String::new() }),
    ];
    writes.extend(logic_event_paths(logic).into_iter().map(|p| (p, EditValue::FreshEventId)));
    writes
}

fn direct_lamp_reset_edits(lamp: u32) -> Vec<(String, EditValue)> {
    let l = direct_lamp_path(lamp);
    vec![
        (format!("{l}/Lamp Selection"), EditValue::Int { value: 0 }),
        (format!("{l}/Lamp Description"), EditValue::String { value: String::new() }),
        (format!("{l}/Lamp On"), EditValue::FreshEventId),
        (format!("{l}/Lamp Off"), EditValue::FreshEventId),
    ]
}

fn track_circuit_reset_edits(circuit: u32) -> Vec<(String, EditValue)> {
    let c = track_circuit_path(circuit);
    vec![
        (format!("{c}/Remote Mast Description"), EditValue::String { value: String::new() }),
        (format!("{c}/Link Address"), EditValue::FreshEventId),
    ]
}

/// A slot to return to its unused state, on one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTarget {
    pub node_key: String,
    pub slot: ReleaseSlot,
}

/// What a release resets (1-based indexes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseSlot {
    Mast(u32),
    Logic(u32),
    DirectLamp(u32),
    TrackCircuit(u32),
    /// Only the listed actions' (1-based) `Action Event` fields of a Logic
    /// get fresh event IDs; the Logic otherwise stays as configured.
    LogicActions { index: u32, actions: Vec<u32> },
}

impl From<&ClaimedSlot> for ReleaseTarget {
    fn from(claim: &ClaimedSlot) -> Self {
        let slot = match claim.kind {
            ClaimedSlotKind::Mast => ReleaseSlot::Mast(claim.index),
            ClaimedSlotKind::Logic => ReleaseSlot::Logic(claim.index),
            ClaimedSlotKind::TrackCircuit => ReleaseSlot::TrackCircuit(claim.index),
        };
        Self { node_key: claim.node_key.clone(), slot }
    }
}

/// One untracked owner's release: the Logic that set its aspects.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UntrackedOwnerRelease {
    pub node_key: String,
    pub owner: SignalOutputOwner,
    /// Logic on the owner's node (ascending) that set the mast's aspects.
    pub matched_logic: Vec<u32>,
    /// The subset of `matched_logic` that also sends other events and is
    /// kept, with only its mast-matching actions given fresh event IDs.
    pub kept_logic: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UntrackedRelease {
    /// Release targets for `release_slots`, in owner order.
    pub targets: Vec<ReleaseTarget>,
    pub owners: Vec<UntrackedOwnerRelease>,
}

/// Expand untracked owners (a mast or Direct Lamp row no facility claims,
/// named by the request) into release targets. This is the one place Logic
/// is matched to a mast: a same-node Logic that is configured (not claimed,
/// Function != 0) and has any Action Event equal to one of the mast's
/// non-null `set aspect` events. A matched Logic is kept when another action
/// (Condition != 0) sends an event that is not one of those; it then only
/// gets fresh event IDs on its matching actions. Other matched Logic get the
/// full release. Everything is applied through [`release_slots`].
pub fn untracked_release_targets(
    owners: &[(String, SignalOutputOwner)],
    nodes: &[NodeSlotFacts],
) -> UntrackedRelease {
    let mut out = UntrackedRelease::default();
    for (node_key, owner) in owners {
        let mut detail = UntrackedOwnerRelease {
            node_key: node_key.clone(),
            owner: *owner,
            matched_logic: vec![],
            kept_logic: vec![],
        };
        let target = |slot| ReleaseTarget { node_key: node_key.clone(), slot };
        match *owner {
            SignalOutputOwner::DirectLamp(lamp) => push_target(&mut out.targets, target(ReleaseSlot::DirectLamp(lamp))),
            SignalOutputOwner::Mast(index) => {
                push_target(&mut out.targets, target(ReleaseSlot::Mast(index)));
                let node = nodes.iter().find(|n| &n.node_key == node_key);
                let aspect_events: Vec<[u8; 8]> = node
                    .and_then(|n| n.masts.iter().find(|m| m.index == index))
                    .map(|m| m.set_aspect_events.iter().filter_map(|e| usable(*e)).collect())
                    .unwrap_or_default();
                let logic = node.map(|n| n.logic.as_slice()).unwrap_or_default();
                for l in logic.iter().filter(|l| l.in_use && !l.claimed) {
                    let sets_aspect = |event: &Option<[u8; 8]>| event.is_some_and(|e| aspect_events.contains(&e));
                    let matching: Vec<u32> = l
                        .actions
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| sets_aspect(&a.event))
                        .map(|(n, _)| n as u32 + 1)
                        .collect();
                    if matching.is_empty() {
                        continue;
                    }
                    detail.matched_logic.push(l.index);
                    let sends_other = l.actions.iter().any(|a| a.condition != Some(0) && !sets_aspect(&a.event));
                    if sends_other {
                        detail.kept_logic.push(l.index);
                        push_target(&mut out.targets, target(ReleaseSlot::LogicActions { index: l.index, actions: matching }));
                    } else {
                        push_target(&mut out.targets, target(ReleaseSlot::Logic(l.index)));
                    }
                }
            }
        }
        out.owners.push(detail);
    }
    out
}

/// Add a target without ever naming a slot twice: a full Logic release wins
/// over a partial one, and partial releases of one Logic union their actions.
fn push_target(targets: &mut Vec<ReleaseTarget>, new: ReleaseTarget) {
    let logic_index = |slot: &ReleaseSlot| match slot {
        ReleaseSlot::Logic(i) | ReleaseSlot::LogicActions { index: i, .. } => Some(*i),
        _ => None,
    };
    let same_slot = |t: &ReleaseTarget| {
        t.node_key == new.node_key
            && match (&t.slot, &new.slot) {
                (ReleaseSlot::Mast(a), ReleaseSlot::Mast(b))
                | (ReleaseSlot::DirectLamp(a), ReleaseSlot::DirectLamp(b))
                | (ReleaseSlot::TrackCircuit(a), ReleaseSlot::TrackCircuit(b)) => a == b,
                (a, b) => logic_index(a).is_some() && logic_index(a) == logic_index(b),
            }
    };
    let Some(existing) = targets.iter_mut().find(|t| same_slot(t)) else {
        targets.push(new);
        return;
    };
    existing.slot = match (std::mem::replace(&mut existing.slot, ReleaseSlot::Mast(0)), new.slot) {
        (ReleaseSlot::LogicActions { index, mut actions }, ReleaseSlot::LogicActions { actions: more, .. }) => {
            actions.extend(more);
            actions.sort_unstable();
            actions.dedup();
            ReleaseSlot::LogicActions { index, actions }
        }
        (ReleaseSlot::LogicActions { index, .. }, ReleaseSlot::Logic(_)) => ReleaseSlot::Logic(index),
        (kept, _) => kept,
    };
}

/// Release slots to their unused state so they count as free again, using
/// one reset set for every release (a removed facility's claims and a named
/// owner found outside Bowties alike): a mast clears Processing, Description
/// and every rule's lamps; a Logic clears Function and Description and gets a
/// `FreshEventId` on every event field; a Direct Lamp row clears Lamp
/// Selection and Description and gets fresh Lamp On / Lamp Off events. Edits
/// touch only the targeted slots, ordered by `targets`. A target whose node
/// has no facts (configuration not loaded) or whose slot is missing is a
/// blocking `ClaimedSlotUnavailable` issue; then `edits` is empty.
pub fn release_slots(targets: &[ReleaseTarget], nodes: &[NodeSlotFacts]) -> CompiledBlockSignals {
    let mut out = CompiledBlockSignals::default();
    for target in targets {
        let node = nodes.iter().find(|n| n.node_key == target.node_key);
        let writes = node.and_then(|n| match &target.slot {
            ReleaseSlot::Mast(i) => {
                let mast = n.masts.iter().find(|m| m.index == *i)?;
                Some(mast_reset_edits(*i, mast.set_aspect_events.len() as u32))
            }
            ReleaseSlot::Logic(i) => n.logic.iter().any(|l| l.index == *i).then(|| logic_reset_edits(*i)),
            ReleaseSlot::DirectLamp(i) => n.direct_lamps.contains(i).then(|| direct_lamp_reset_edits(*i)),
            ReleaseSlot::TrackCircuit(i) => n.track_circuits.iter().any(|c| c.index == *i).then(|| track_circuit_reset_edits(*i)),
            ReleaseSlot::LogicActions { index, actions } => n.logic.iter().any(|l| l.index == *index).then(|| {
                actions
                    .iter()
                    .map(|a| (action_event_path(*index, *a), EditValue::FreshEventId))
                    .collect()
            }),
        });
        let Some(writes) = writes else {
            out.issues.push(CompileIssue {
                code: CompileIssueCode::ClaimedSlotUnavailable,
                signal_id: None,
                node_key: Some(target.node_key.clone()),
                detail: "A mast, Logic or lamp being released cannot be found on its Signal LCC; its configuration may not be loaded.".to_string(),
                binding_issue: None,
                logic: Vec::new(),
            });
            continue;
        };
        for (path, value) in writes {
            out.edits.push(NodeConfigEdit { node_key: target.node_key.clone(), path, value });
        }
    }
    if !out.issues.is_empty() {
        out.edits.clear();
    }
    out
}

/// Merge a request's release edits with the recipe's: a leaf both write takes
/// the recipe's value, so no leaf is edited twice. Release-only edits come
/// first, then the recipe in its own order.
pub fn merge_release_edits(release: Vec<NodeConfigEdit>, recipe: Vec<NodeConfigEdit>) -> Vec<NodeConfigEdit> {
    let recipe_leaves: std::collections::HashSet<(String, String)> =
        recipe.iter().map(|e| (canonical_node_key(&e.node_key), e.path.clone())).collect();
    let mut merged: Vec<NodeConfigEdit> = release
        .into_iter()
        .filter(|e| !recipe_leaves.contains(&(canonical_node_key(&e.node_key), e.path.clone())))
        .collect();
    merged.extend(recipe);
    merged
}

fn free_mast<'a>(node: &'a NodeSlotFacts, taken: &[u32]) -> Option<&'a MastFact> {
    node.masts.iter().filter(|m| !m.in_use && !taken.contains(&m.index)).min_by_key(|m| m.index)
}

/// Mast Processing value Normal, from the Signal-LCC CDI.
const MAST_NORMAL: i64 = 1;
/// Logic Function value Group (more Logic follow in the group).
const LOGIC_GROUP: i64 = 1;
/// Logic Function value Last (Single).
const LOGIC_LAST_SINGLE: i64 = 2;
/// Logic Action `when false` value Evaluate Next.
const WHEN_EVALUATE_NEXT: i64 = 4;
/// Logic Variable `Trigger` value On Variable Change.
const TRIGGER_ON_CHANGE: i64 = 0;
/// Logic Variable `Trigger` value On Matching Event.
const TRIGGER_ON_EVENT: i64 = 1;
/// Logic Variable `Trigger` value None.
const TRIGGER_NONE: i64 = 2;
/// Logic Variable `Source` value: the Variable's own (C) events.
const SOURCE_OWN_EVENTS: i64 = 0;
/// Logic Operation value V1 Only.
const OPERATION_V1_ONLY: i64 = 7;
/// Logic Action `Condition` value: nothing is sent.
const SEND_NEVER: i64 = 0;
/// Logic Action `Condition` value Immediate if True.
const SEND_IF_TRUE: i64 = 3;
/// Logic Action `Condition` value Immediate if False.
const SEND_IF_FALSE: i64 = 4;

type Writes = Vec<(String, EditValue)>;

/// A rung action slot that sends nothing.
const NO_ACTION: (i64, Option<[u8; 8]>) = (SEND_NEVER, None);

fn int(value: i64) -> EditValue {
    EditValue::Int { value }
}

fn event(bytes: [u8; 8]) -> EditValue {
    EditValue::EventId { bytes }
}

fn text(value: &str) -> EditValue {
    EditValue::String { value: value.to_string() }
}

/// Everything a signal's mast needs: name, three rules' lamps and Processing last.
fn mast_edits(name: &str, mast: u32, wiring: &SignalWiring) -> Writes {
    let mut edits = Writes::new();
    let m = mast_path(mast);
    edits.push((format!("{m}/Mast Description"), text(name)));
    edits.push((format!("{m}/Lamp Fade"), int(0)));
    let (red, green) = (i64::from(wiring.red_output), i64::from(wiring.green_output));
    // Rule name, Track Speed and lit lamps for Stop, Approach and Clear.
    let rules: [(i64, i64, &[i64]); 3] =
        [(0, 0, &[red]), (21, 5, &[red, green]), (29, 7, &[green])];
    debug_assert_eq!(rules.len() as u32, RECIPE_RULES);
    for (index, (rule_name, speed, lamps)) in rules.iter().enumerate() {
        let rule = index as u32 + 1;
        let r = rule_path(mast, rule);
        edits.push((format!("{r}/Rule Name"), int(*rule_name)));
        edits.push((format!("{r}/Track Speed"), int(*speed)));
        for slot in 0..RECIPE_APPEARANCES {
            edits.push((
                appearance_lamp_path(mast, rule, slot + 1),
                int(lamps.get(slot as usize).copied().unwrap_or(0)),
            ));
            edits.push((format!("{r}/Appearance[{}]/Lamp Phase (A-B) - Flash Rate", slot + 1), int(0)));
        }
        edits.push((format!("{r}/Appearance Effects"), int(0)));
        edits.push((effects_lamp_path(mast, rule), int(0)));
    }
    edits.push((format!("{m}/Mast Processing"), int(MAST_NORMAL)));
    edits
}

/// The rung head shared by occupancy rungs: description, own occupancy events.
fn occupancy_head(logic: u32, description: &str, (occupied, clear): ([u8; 8], [u8; 8])) -> Writes {
    let l = logic_path(logic);
    vec![
        (format!("{l}/Description"), text(description)),
        (format!("{l}/Variable #1/Trigger"), int(TRIGGER_ON_EVENT)),
        (format!("{l}/Variable #1/Source"), int(SOURCE_OWN_EVENTS)),
        (occupied_event_path(logic), event(occupied)),
        (clear_event_path(logic), event(clear)),
        (format!("{l}/Logic Operation"), int(OPERATION_V1_ONLY)),
        (format!("{l}/Variable #2/Trigger"), int(TRIGGER_NONE)),
    ]
}

/// What happens after a rung's variables: Action #1, the delay, the four
/// event actions (`Condition`, event) and the Function, written last.
fn rung_tail(logic: u32, when_false: i64, actions: [(i64, Option<[u8; 8]>); 4], function: i64) -> Writes {
    let l = logic_path(logic);
    let mut edits = vec![
        (format!("{l}/Action#1/when true"), int(0)),
        (format!("{l}/Action#1/when false"), int(when_false)),
        (format!("{l}/Time Delay before action/Delay Time (1-60000)"), int(0)),
        (format!("{l}/Time Delay before action/Units"), int(0)),
    ];
    for (n, (condition, action_event)) in actions.into_iter().enumerate() {
        let action = n as u32 + 1;
        edits.push((format!("{l}/Action#2[{action}]/Condition"), int(condition)));
        if let Some(bytes) = action_event {
            edits.push((action_event_path(logic, action), event(bytes)));
        }
    }
    edits.push((logic_function_path(logic), int(function)));
    edits
}

/// A `Last (Single)` rung's tail: its first action sends `if_true` when the
/// rung's condition holds, its second sends `if_false` when it does not.
fn set_aspect_rung_tail(logic: u32, if_true: [u8; 8], if_false: [u8; 8]) -> Writes {
    rung_tail(
        logic,
        0,
        [(SEND_IF_TRUE, Some(if_true)), (SEND_IF_FALSE, Some(if_false)), NO_ACTION, NO_ACTION],
        LOGIC_LAST_SINGLE,
    )
}

/// A rung's description: "{name} occupancy", numbered when several blocks are protected.
fn occupancy_description(name: &str, index: usize, blocks: usize) -> String {
    if blocks == 1 { format!("{name} occupancy") } else { format!("{name} occupancy {}", index + 1) }
}

/// One `Group` rung per protected block but the last: its occupancy sends Stop.
/// Written from the highest Logic down, so a `Group` is never on the board
/// without its terminator, which the caller has already written.
fn group_stop_edits(
    name: &str,
    first_logic: u32,
    occupancy: &[([u8; 8], [u8; 8])],
    blocks: usize,
    stop: [u8; 8],
) -> Writes {
    let mut edits = Writes::new();
    for (i, events) in occupancy.iter().enumerate().rev() {
        let logic = first_logic + i as u32;
        edits.extend(occupancy_head(logic, &occupancy_description(name, i, blocks), *events));
        edits.extend(rung_tail(
            logic,
            WHEN_EVALUATE_NEXT,
            [(SEND_IF_TRUE, Some(stop)), NO_ACTION, NO_ACTION, NO_ACTION],
            LOGIC_GROUP,
        ));
    }
    edits
}

/// The last signal's rungs: any block but the last sets Stop through a `Group`
/// rung; the last block's rung sets Stop, and its clear sets Clear. The `Last
/// (Single)` rung is written first.
fn terminal_edits(
    name: &str,
    logic: u32,
    occupancy: &[([u8; 8], [u8; 8])],
    stop: [u8; 8],
    clear: [u8; 8],
) -> Writes {
    let (last, earlier) = occupancy.split_last().expect("a signal protects at least one block");
    let last_logic = logic + earlier.len() as u32;
    let mut edits =
        occupancy_head(last_logic, &occupancy_description(name, earlier.len(), occupancy.len()), *last);
    edits.extend(set_aspect_rung_tail(last_logic, stop, clear));
    edits.extend(group_stop_edits(name, logic, earlier, occupancy.len(), stop));
    edits
}

/// What a signal with a signal after it needs beyond its mast.
struct InternalRecipe<'a> {
    name: &'a str,
    next_name: &'a str,
    first_logic: u32,
    circuit: u32,
    link: [u8; 8],
    occupancy: &'a [([u8; 8], [u8; 8])],
    stop: [u8; 8],
    approach: [u8; 8],
    clear: [u8; 8],
}

/// The Track Circuit bound to the next signal, then the group: one `Group` rung
/// per protected block and the `Last (Single)` rung after them. The `Last
/// (Single)` rung is written before the `Group` rungs, so a `Group` is never
/// on the board without its terminator.
fn internal_edits(recipe: &InternalRecipe<'_>) -> Writes {
    let c = track_circuit_path(recipe.circuit);
    let mut edits = vec![
        (format!("{c}/Remote Mast Description"), text(recipe.next_name)),
        (format!("{c}/Link Address"), event(recipe.link)),
    ];
    let next_rung = recipe.first_logic + recipe.occupancy.len() as u32;
    let l = logic_path(next_rung);
    edits.extend([
        (format!("{l}/Description"), text(&format!("{} next signal", recipe.name))),
        (format!("{l}/Variable #1/Trigger"), int(TRIGGER_ON_CHANGE)),
        (format!("{l}/Variable #1/Source"), int(i64::from(recipe.circuit))),
        (format!("{l}/Variable #1/Track Speed"), int(0)),
        (format!("{l}/Logic Operation"), int(OPERATION_V1_ONLY)),
        (format!("{l}/Variable #2/Trigger"), int(TRIGGER_NONE)),
    ]);
    edits.extend(set_aspect_rung_tail(next_rung, recipe.approach, recipe.clear));
    edits.extend(group_stop_edits(recipe.name, recipe.first_logic, recipe.occupancy, recipe.occupancy.len(), recipe.stop));
    edits
}
