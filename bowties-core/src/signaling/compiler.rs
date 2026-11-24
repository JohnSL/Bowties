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
//!
//! Masts never move: a saved signal keeps the mast in its recorded resources.
//! Logic placement is board-wide and owned by [`super::logic_placement`]: the
//! recipe describes its Logic relative to the group's first slot and the
//! planner supplies the slot.

use serde::{Deserialize, Serialize};

use super::{
    project_signal_binding, ApproachLighting, BlockSignalsRequest, EntranceEnd, ExitEnd, Movements,
    ProposalIssue, SignalBindingSelection, SignalHardwareFacts, SignalOutputOwner, SignalPreview,
};
use crate::layout::channels::{ChannelBinding, ChannelRole};
use super::logic_placement::{
    plan_logic_placement, LogicGroupDemand, LogicPlacementIssue, LogicPlacementIssueCode, LogicSlotFact,
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
    /// A saved group would need a different length (not supported yet).
    LogicResizeUnsupported,
    /// A requested release names an owner that is facility-owned, not an
    /// owner, or not on a loaded node.
    ReleaseInvalid,
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
    /// The wiring board's whole new Logic placement (one entry).
    pub placement: Vec<NodeLogicPlacement>,
    pub issues: Vec<CompileIssue>,
}

/// Compile the one-block terminal recipe. Deterministic: signals, slots and
/// edits are chosen and ordered from the inputs alone, lowest free slot first.
/// With any issue, `edits`, `resources` and `placement` are empty.
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

    let facility_id = input.line.facility_id.clone();
    let own = input
        .saved_resources
        .iter()
        .find(|r| r.signal_id == signal.signal_id)
        .map(|r| &r.resources);
    let own_masts: Vec<u32> = own
        .filter(|r| same_node(&r.node_key, &wiring.node_key))
        .map(|r| r.mast)
        .into_iter()
        .collect();
    check_binding(&mut signal, wiring, &input.hardware, &own_masts, &mut ctx);

    let node = input.nodes.iter().find(|n| same_node(&n.node_key, &wiring.node_key));
    let mast = if own.is_some() {
        let mast = own_masts
            .first()
            .and_then(|i| node.and_then(|n| n.masts.iter().find(|m| m.index == *i)));
        if mast.is_none() {
            ctx.push(
                CompileIssueCode::ClaimedSlotUnavailable,
                "The mast this facility already uses is not available on the chosen Signal LCC.",
            );
        }
        mast
    } else {
        let mast = node.and_then(free_mast);
        if mast.is_none() {
            let outside = node.map_or(0, |n| n.masts.iter().filter(|m| m.in_use && !m.claimed).count());
            ctx.push(CompileIssueCode::NoFreeMast, &no_free_detail("mast", outside));
        }
        mast
    };

    let recorded: Vec<PlacedLogicGroup> = input
        .logic_placement
        .iter()
        .filter(|board| same_node(&board.node_key, &wiring.node_key))
        .flat_map(|board| board.groups.iter().cloned())
        .collect();
    let slot_facts: Vec<LogicSlotFact> = node
        .map(|n| n.logic.iter().map(|l| LogicSlotFact { index: l.index, function: l.function }).collect())
        .unwrap_or_default();
    // One group of one Logic per signal in this release.
    let demand = [LogicGroupDemand { signal_id: signal.signal_id.clone(), length: 1 }];
    let planned = plan_logic_placement(&slot_facts, &recorded, &facility_id, &demand);
    let outside_logic = node.map_or(0, |n| n.logic.iter().filter(|l| l.in_use && !l.claimed).count());
    for issue in &planned.issues {
        let (code, detail) = map_placement_issue(issue, outside_logic);
        ctx.push_logic(code, &detail, issue.logic.clone());
    }
    let group = planned
        .placement
        .iter()
        .find(|g| g.facility_id == facility_id && g.signal_id == signal.signal_id)
        .cloned();
    if let Some(group) = &group {
        let missing = (group.first..group.first + group.length)
            .any(|i| !node.is_some_and(|n| n.logic.iter().any(|l| l.index == i)));
        if missing {
            ctx.push(
                CompileIssueCode::ClaimedSlotUnavailable,
                "The Logic this facility already uses is not available on the chosen Signal LCC.",
            );
        }
    }
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
        mast.and_then(|m| Some((usable(*m.set_aspect_events.first()?)?, usable(*m.set_aspect_events.get(2)?)?)));
    if mast.is_some() && aspect_events.is_none() {
        ctx.push(
            CompileIssueCode::AspectEventsMissing,
            "The chosen mast's Stop and Clear set-aspect events are not available.",
        );
    }

    if !out.issues.is_empty() {
        return out;
    }
    let (Some(mast), Some(group), Some(occupancy), Some(aspect_events)) =
        (mast, group, occupancy, aspect_events)
    else {
        return out;
    };
    out.edits = recipe_edits(&signal.name, mast.index, group.first, wiring, occupancy, aspect_events);
    out.resources = vec![PlannedSignalResources {
        signal_id: signal.signal_id.clone(),
        resources: SignalResources {
            node_key: wiring.node_key.clone(),
            mast: mast.index,
            track_circuits: own.map(|r| r.track_circuits.clone()).unwrap_or_default(),
        },
    }];
    out.placement =
        vec![NodeLogicPlacement { node_key: wiring.node_key.clone(), groups: planned.placement }];
    out
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
        LogicPlacementIssueCode::ResizeUnsupported => {
            (CompileIssueCode::LogicResizeUnsupported, format!("{}{}", issue.detail, names(&issue.logic)))
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

fn occupied_event_path(logic: u32) -> String {
    format!("{}/Variable #1/set true", logic_path(logic))
}

fn clear_event_path(logic: u32) -> String {
    format!("{}/Variable #1/set false", logic_path(logic))
}

fn stop_action_event_path(logic: u32) -> String {
    action_event_path(logic, 1)
}

fn clear_action_event_path(logic: u32) -> String {
    action_event_path(logic, 2)
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
    /// Only the listed actions' (1-based) `Action Event` fields of a Logic
    /// get fresh event IDs; the Logic otherwise stays as configured.
    LogicActions { index: u32, actions: Vec<u32> },
}

impl From<&ClaimedSlot> for ReleaseTarget {
    fn from(claim: &ClaimedSlot) -> Self {
        let slot = match claim.kind {
            ClaimedSlotKind::Mast => ReleaseSlot::Mast(claim.index),
            ClaimedSlotKind::Logic => ReleaseSlot::Logic(claim.index),
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
                (ReleaseSlot::Mast(a), ReleaseSlot::Mast(b)) | (ReleaseSlot::DirectLamp(a), ReleaseSlot::DirectLamp(b)) => a == b,
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

fn free_mast(node: &NodeSlotFacts) -> Option<&MastFact> {
    node.masts.iter().filter(|m| !m.in_use).min_by_key(|m| m.index)
}

/// Mast Processing value Normal, from the Signal-LCC CDI.
const MAST_NORMAL: i64 = 1;
/// Logic Function value Last (Single).
const LOGIC_LAST_SINGLE: i64 = 2;
/// Offset from the group's first Logic of the rung holding the signal's own occupancy.
const OCCUPANCY_RUNG: u32 = 0;

fn recipe_edits(
    name: &str,
    mast: u32,
    group_first: u32,
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

    // The one rung of this release's group: the signal's own occupancy.
    let logic = group_first + OCCUPANCY_RUNG;
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
