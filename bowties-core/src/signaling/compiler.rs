//! Pure compiler for the one-block Automatic Block Signals terminal recipe.
//!
//! Turns a Line (the facility's block list and policy settings), the wiring
//! chosen for its signal and read-only node facts into configuration edits,
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

use serde::{Deserialize, Serialize};

use super::{
    project_signal_binding, ApproachLighting, BlockSignalsRequest, EntranceEnd, ExitEnd, Movements,
    ProposalIssue, SignalBindingSelection, SignalHardwareFacts, SignalPreview,
};
use crate::layout::channels::{ChannelBinding, ChannelRole};
use crate::layout::facilities::{ClaimedSlot, ClaimedSlotKind};

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
    /// The mast's generated `set aspect` events for Rules 1, 2 and 3.
    pub set_aspect_events: [Option<[u8; 8]>; 3],
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicFact {
    /// 1-based Logic number.
    pub index: u32,
    /// Function is not Blocked, or another facility claims the Logic.
    pub in_use: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeSlotFacts {
    pub node_key: String,
    pub masts: Vec<MastFact>,
    pub logic: Vec<LogicFact>,
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
    /// The facility's own claimed slots when recompiling a saved facility. The
    /// recipe is placed in exactly these slots and their pins and values are
    /// not reported as in use. Empty when creating (lowest free slots).
    #[serde(default)]
    pub own_claims: Vec<ClaimedSlot>,
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
    OccupancyEventsMissing,
    AspectEventsMissing,
    UnsupportedBlockCount,
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
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompiledBlockSignals {
    pub edits: Vec<NodeConfigEdit>,
    pub claimed_slots: Vec<ClaimedSlot>,
    pub issues: Vec<CompileIssue>,
}

/// Compile the one-block terminal recipe. Deterministic: signals, slots and
/// edits are chosen and ordered from the inputs alone, lowest free slot first.
/// With any issue, `edits` and `claimed_slots` are empty.
pub fn compile_block_signals(input: &CompileInput) -> CompiledBlockSignals {
    let mut out = CompiledBlockSignals::default();
    let mut ctx = IssueSink { issues: &mut out.issues, signal_id: None, node_key: None };
    let signals = super::chain::derive_signals(&input.line, &mut Vec::new());
    ctx.signal_id = signals.first().map(|s| s.signal_id.clone());
    check_scope(input, &signals, &mut ctx);
    let Some(mut signal) = signals.into_iter().next() else {
        return out;
    };
    let Some(wiring) = input.wiring.iter().find(|w| w.signal_id == signal.signal_id) else {
        ctx.push(CompileIssueCode::WiringMissing, "No Signal LCC outputs are chosen for this signal.");
        return out;
    };
    ctx.node_key = Some(wiring.node_key.clone());

    let own_masts: Vec<u32> = input
        .own_claims
        .iter()
        .filter(|c| c.kind == ClaimedSlotKind::Mast && c.node_key == wiring.node_key)
        .map(|c| c.index)
        .collect();
    check_binding(&mut signal, wiring, &input.hardware, &own_masts, &mut ctx);

    let node = input.nodes.iter().find(|n| n.node_key == wiring.node_key);
    let (mast, logic) = if input.own_claims.is_empty() {
        let mast = node.and_then(free_mast);
        if mast.is_none() {
            ctx.push(CompileIssueCode::NoFreeMast, "The board has no free mast.");
        }
        let logic = node.and_then(free_logic);
        if logic.is_none() {
            ctx.push(CompileIssueCode::NoFreeLogic, "The board has no free Logic conditional.");
        }
        (mast, logic)
    } else {
        claimed_slots(&input.own_claims, wiring, node, &mut ctx)
    };
    let occupancy = input
        .occupancy
        .iter()
        .find(|b| signal.protected_block_ids.contains(&b.block_id))
        .and_then(|b| Some((usable(b.occupied)?, usable(b.clear)?)));
    if occupancy.is_none() {
        ctx.push(
            CompileIssueCode::OccupancyEventsMissing,
            "The protected block has no occupied and clear event IDs.",
        );
    }
    let aspect_events =
        mast.and_then(|m| Some((usable(m.set_aspect_events[0])?, usable(m.set_aspect_events[2])?)));
    if mast.is_some() && aspect_events.is_none() {
        ctx.push(
            CompileIssueCode::AspectEventsMissing,
            "The chosen mast's Stop and Clear set-aspect events are not available.",
        );
    }

    if !out.issues.is_empty() {
        return out;
    }
    let (Some(mast), Some(logic), Some(occupancy), Some(aspect_events)) =
        (mast, logic, occupancy, aspect_events)
    else {
        return out;
    };
    out.edits = recipe_edits(&signal.name, mast.index, logic.index, wiring, occupancy, aspect_events);
    out.claimed_slots = vec![
        ClaimedSlot { node_key: wiring.node_key.clone(), kind: ClaimedSlotKind::Mast, index: mast.index },
        ClaimedSlot { node_key: wiring.node_key.clone(), kind: ClaimedSlotKind::Logic, index: logic.index },
    ];
    out
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
        });
    }
}

/// An event ID the node can really carry; all-zero means unset.
fn usable(event: Option<[u8; 8]>) -> Option<[u8; 8]> {
    event.filter(|bytes| bytes.iter().any(|b| *b != 0))
}

fn check_scope(input: &CompileInput, signals: &[SignalPreview], ctx: &mut IssueSink<'_>) {
    let line = &input.line;
    if line.blocks.len() != 1 {
        ctx.push(CompileIssueCode::UnsupportedBlockCount, "This release supports one block with one signal.");
    }
    if line.movements != Movements::OneDirection {
        ctx.push(CompileIssueCode::UnsupportedMovements, "This release supports one direction only.");
    }
    if line.entrance_end != EntranceEnd::NoPlant || line.exit_end != ExitEnd::NothingBeyond {
        ctx.push(CompileIssueCode::UnsupportedEnds, "This release supports a Line with nothing beyond either end.");
    }
    let signal_id = signals.first().map(|s| s.signal_id.as_str());
    let (aspects, lighting) = line
        .signal_overrides
        .iter()
        .find(|o| Some(o.signal_id.as_str()) == signal_id)
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

/// The facility's own mast and Logic, which must exist on the wiring node.
/// Never re-picks: a claim that cannot be honoured is a blocking issue.
fn claimed_slots<'a>(
    claims: &[ClaimedSlot],
    wiring: &SignalWiring,
    node: Option<&'a NodeSlotFacts>,
    ctx: &mut IssueSink<'_>,
) -> (Option<&'a MastFact>, Option<&'a LogicFact>) {
    let on_node = |kind: ClaimedSlotKind| {
        claims
            .iter()
            .find(|c| c.kind == kind && c.node_key == wiring.node_key)
            .map(|c| c.index)
    };
    let mast = on_node(ClaimedSlotKind::Mast)
        .and_then(|i| node.and_then(|n| n.masts.iter().find(|m| m.index == i)));
    let logic = on_node(ClaimedSlotKind::Logic)
        .and_then(|i| node.and_then(|n| n.logic.iter().find(|l| l.index == i)));
    let off_node = claims.iter().any(|c| c.node_key != wiring.node_key);
    if mast.is_none() || logic.is_none() || off_node {
        ctx.push(
            CompileIssueCode::ClaimedSlotUnavailable,
            "The mast and Logic this facility already uses are not available on the chosen Signal LCC.",
        );
    }
    (mast, logic)
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

/// Every lamp leaf the recipe sets on a mast: the lit lamps of each
/// appearance and each rule's effects lamp.
fn recipe_lamp_paths(mast: u32) -> Vec<String> {
    let mut paths = Vec::new();
    for rule in 1..=RECIPE_RULES {
        for appearance in 1..=RECIPE_APPEARANCES {
            paths.push(appearance_lamp_path(mast, rule, appearance));
        }
        paths.push(effects_lamp_path(mast, rule));
    }
    paths
}

fn occupied_event_path(logic: u32) -> String {
    format!("{}/Variable #1/set true", logic_path(logic))
}

fn clear_event_path(logic: u32) -> String {
    format!("{}/Variable #1/set false", logic_path(logic))
}

fn stop_action_event_path(logic: u32) -> String {
    format!("{}/Action#2[1]/Action Event", logic_path(logic))
}

fn clear_action_event_path(logic: u32) -> String {
    format!("{}/Action#2[2]/Action Event", logic_path(logic))
}

/// Every event-ID leaf the recipe writes on a Logic: the block's occupied and
/// clear triggers and the two set-aspect actions.
fn recipe_logic_event_paths(logic: u32) -> [String; 4] {
    [
        occupied_event_path(logic),
        clear_event_path(logic),
        stop_action_event_path(logic),
        clear_action_event_path(logic),
    ]
}

/// Release the slots a removed facility claimed: return each claimed mast and
/// Logic to its unused state (Processing / Function 0, empty description) so
/// it counts as free again. A mast's lamps are cleared, and every event the
/// recipe wired into the Logic is marked `FreshEventId`, breaking its
/// connections. Edits touch only the claimed slots, ordered by
/// `claims`. A claim whose node has no facts (configuration not loaded) or
/// whose slot is missing is a blocking `ClaimedSlotUnavailable` issue; then
/// `edits` is empty.
pub fn release_slots(claims: &[ClaimedSlot], nodes: &[NodeSlotFacts]) -> CompiledBlockSignals {
    let mut out = CompiledBlockSignals::default();
    let int = |value: i64| EditValue::Int { value };
    let empty = || EditValue::String { value: String::new() };
    for claim in claims {
        let node = nodes.iter().find(|n| n.node_key == claim.node_key);
        let present = node.is_some_and(|n| match claim.kind {
            ClaimedSlotKind::Mast => n.masts.iter().any(|m| m.index == claim.index),
            ClaimedSlotKind::Logic => n.logic.iter().any(|l| l.index == claim.index),
        });
        if !present {
            out.issues.push(CompileIssue {
                code: CompileIssueCode::ClaimedSlotUnavailable,
                signal_id: None,
                node_key: Some(claim.node_key.clone()),
                detail: "A mast or Logic this facility uses cannot be found on its Signal LCC; its configuration may not be loaded.".to_string(),
                binding_issue: None,
            });
            continue;
        }
        let (processing, description) = match claim.kind {
            ClaimedSlotKind::Mast => {
                (mast_processing_path(claim.index), format!("{}/Mast Description", mast_path(claim.index)))
            }
            ClaimedSlotKind::Logic => {
                (logic_function_path(claim.index), format!("{}/Description", logic_path(claim.index)))
            }
        };
        let mut writes = vec![(processing, int(0)), (description, empty())];
        match claim.kind {
            ClaimedSlotKind::Mast => {
                writes.extend(recipe_lamp_paths(claim.index).into_iter().map(|p| (p, int(0))));
            }
            ClaimedSlotKind::Logic => {
                writes.extend(
                    recipe_logic_event_paths(claim.index).into_iter().map(|p| (p, EditValue::FreshEventId)),
                );
            }
        }
        for (path, value) in writes {
            out.edits.push(NodeConfigEdit { node_key: claim.node_key.clone(), path, value });
        }
    }
    if !out.issues.is_empty() {
        out.edits.clear();
    }
    out
}

fn free_mast(node: &NodeSlotFacts) -> Option<&MastFact> {
    node.masts.iter().filter(|m| !m.in_use).min_by_key(|m| m.index)
}

fn free_logic(node: &NodeSlotFacts) -> Option<&LogicFact> {
    node.logic.iter().filter(|l| !l.in_use).min_by_key(|l| l.index)
}

/// Mast Processing value Normal, from the Signal-LCC CDI.
const MAST_NORMAL: i64 = 1;
/// Logic Function value Last (Single).
const LOGIC_LAST_SINGLE: i64 = 2;

fn recipe_edits(
    name: &str,
    mast: u32,
    logic: u32,
    wiring: &SignalWiring,
    (occupied, clear): ([u8; 8], [u8; 8]),
    (stop_event, clear_event): ([u8; 8], [u8; 8]),
) -> Vec<NodeConfigEdit> {
    let mut edits = Vec::new();
    let node_key = &wiring.node_key;
    let mut put = |path: String, value: EditValue| {
        edits.push(NodeConfigEdit { node_key: node_key.clone(), path, value });
    };
    let int = |value: i64| EditValue::Int { value };
    let m = mast_path(mast);
    put(format!("{m}/Mast Description"), EditValue::String { value: name.to_string() });
    put(format!("{m}/Lamp Fade"), int(0));
    let (red, green) = (i64::from(wiring.red_output), i64::from(wiring.green_output));
    // Rule name, Track Speed and lit lamps for Stop, Approach and Clear.
    let rules: [(i64, i64, &[i64]); 3] =
        [(0, 0, &[red]), (21, 5, &[red, green]), (29, 7, &[green])];
    debug_assert_eq!(rules.len() as u32, RECIPE_RULES);
    for (index, (rule_name, speed, lamps)) in rules.iter().enumerate() {
        let rule = index as u32 + 1;
        let r = rule_path(mast, rule);
        put(format!("{r}/Rule Name"), int(*rule_name));
        put(format!("{r}/Track Speed"), int(*speed));
        for slot in 0..RECIPE_APPEARANCES {
            put(
                appearance_lamp_path(mast, rule, slot + 1),
                int(lamps.get(slot as usize).copied().unwrap_or(0)),
            );
            put(format!("{r}/Appearance[{}]/Lamp Phase (A-B) - Flash Rate", slot + 1), int(0));
        }
        put(format!("{r}/Appearance Effects"), int(0));
        put(effects_lamp_path(mast, rule), int(0));
    }
    put(format!("{m}/Mast Processing"), int(MAST_NORMAL));

    let l = logic_path(logic);
    put(format!("{l}/Description"), EditValue::String { value: format!("{name} occupancy") });
    put(format!("{l}/Variable #1/Trigger"), int(1));
    put(format!("{l}/Variable #1/Source"), int(0));
    put(occupied_event_path(logic), EditValue::EventId { bytes: occupied });
    put(clear_event_path(logic), EditValue::EventId { bytes: clear });
    put(format!("{l}/Logic Operation"), int(7));
    put(format!("{l}/Variable #2/Trigger"), int(2));
    put(format!("{l}/Action#1/when true"), int(0));
    put(format!("{l}/Action#1/when false"), int(0));
    put(format!("{l}/Time Delay before action/Delay Time (1-60000)"), int(0));
    put(format!("{l}/Time Delay before action/Units"), int(0));
    put(format!("{l}/Action#2[1]/Condition"), int(3));
    put(stop_action_event_path(logic), EditValue::EventId { bytes: stop_event });
    put(format!("{l}/Action#2[2]/Condition"), int(4));
    put(clear_action_event_path(logic), EditValue::EventId { bytes: clear_event });
    put(format!("{l}/Action#2[3]/Condition"), int(0));
    put(format!("{l}/Action#2[4]/Condition"), int(0));
    put(format!("{l}/Function"), int(LOGIC_LAST_SINGLE));
    edits
}
