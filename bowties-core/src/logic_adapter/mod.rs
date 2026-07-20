//! Logic adapter — compiles behavior templates into CDI field writes.
//!
//! This module owns the compilation of `Compiled` behavior templates into
//! concrete CDI configuration values (conditional line settings on Tower LCC
//! nodes). The compiler expands condition-action rules from behavior templates
//! into Tower LCC conditional line configurations with correct mast group
//! structure, variable inputs, and aspect-driven action events.
//!
//! Function-level module seam (YAGNI: no trait/dynamic dispatch until a
//! second compilation target arrives — per D2:A).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::node_tree::{ConfigNode, GroupNode, NodeConfigTree, replication_instances};

// ── Allocation types ──────────────────────────────────────────────────────

/// A contiguous range of conditional lines allocated on a target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalLineRange {
    /// 0-based start index of the first conditional line in this range.
    pub start: u32,
    /// Number of contiguous conditional lines in this range.
    pub count: u32,
}

/// A logic allocation record for one facility on one target node.
///
/// Persisted as part of the facility layer so that save + reopen
/// preserves the allocation. The compiler is the single authority
/// on allocation rules (D3:A — per-facility field, no new store).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicAllocation {
    /// Facility that owns this allocation.
    pub facility_id: String,
    /// Node key of the logic target (Tower LCC node).
    pub target_node_key: String,
    /// Conditional line range(s) allocated on the target node.
    pub conditional_lines: ConditionalLineRange,
}

// ── Compiled plan ─────────────────────────────────────────────────────────

/// One CDI field write produced by the compiler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompiledFieldWrite {
    /// CDI leaf path on the target node (e.g. "cdi/segment/group/int").
    pub leaf_path: String,
    /// Memory space of the leaf.
    pub space: u8,
    /// Byte address of the leaf within the memory space.
    pub address: u64,
    /// Value to write (as raw bytes, up to 8 bytes for numeric fields).
    pub value: Vec<u8>,
    /// Element type hint for the frontend staging logic.
    pub element_type: String,
}

/// The output of the logic compiler for one facility.
///
/// Contains the allocation record and the CDI field writes that
/// configure the allocated conditional lines on the target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompiledLogicPlan {
    /// The allocation record (persisted with the facility).
    pub allocation: LogicAllocation,
    /// CDI field writes to stage as drafts on the target node.
    pub field_writes: Vec<CompiledFieldWrite>,
}

// ── Capacity ──────────────────────────────────────────────────────────────

/// Capacity information for a logic target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicCapacity {
    /// Total conditional lines available on the node.
    pub total_lines: u32,
    /// Conditional lines currently allocated.
    pub used_lines: u32,
}

impl LogicCapacity {
    pub fn available(&self) -> u32 {
        self.total_lines.saturating_sub(self.used_lines)
    }
}

// ── Errors ────────────────────────────────────────────────────────────────

/// Errors the logic compiler may return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// The target node does not have enough conditional lines to
    /// satisfy the template's requirements.
    InsufficientCapacity {
        required: u32,
        available: u32,
    },
    /// The referenced template is not a `Compiled` template.
    NotCompiled {
        template_id: String,
    },
    /// The referenced template was not found in the registry.
    UnknownTemplate {
        template_id: String,
    },
    /// An aspect in the template exceeds the maximum action events
    /// per conditional line (Tower LCC limit: 4).
    TooManyActions {
        aspect: String,
        action_count: usize,
        max_actions: usize,
    },
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientCapacity { required, available } => {
                write!(
                    f,
                    "insufficient capacity: need {} conditional lines but only {} available",
                    required, available
                )
            }
            Self::NotCompiled { template_id } => {
                write!(f, "template '{}' is not a compiled template", template_id)
            }
            Self::UnknownTemplate { template_id } => {
                write!(f, "unknown template '{}'", template_id)
            }
            Self::TooManyActions { aspect, action_count, max_actions } => {
                write!(
                    f,
                    "aspect '{}' requires {} action events but maximum is {}",
                    aspect, action_count, max_actions
                )
            }
        }
    }
}

impl std::error::Error for CompileError {}

// ── Tower LCC Capacity Constants ──────────────────────────────────────────

/// Maximum conditional lines per Tower LCC node.
pub const MAX_CONDITIONAL_LINES: u32 = 32;
/// Maximum action events per conditional line.
pub const MAX_ACTIONS_PER_LINE: usize = 4;
/// Maximum length of a conditional line description string.
const DESCRIPTION_MAX_LEN: usize = 32;

// ── Unresolved field writes ───────────────────────────────────────────────

/// Identifies a specific field within a Tower LCC conditional line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConditionalLineField {
    Description,
    Function,
    V1Trigger,
    V1Source,
    V1TrackSpeed,
    V1SetTrueEvent,
    V1SetFalseEvent,
    LogicOperation,
    V2Trigger,
    ActionWhenTrue,
    ActionWhenFalse,
    /// Per-action-event condition (slot 0–3).
    ActionCondition(u8),
    /// Per-action-event destination (slot 0–3).
    ActionDestination(u8),
    /// Per-action-event track speed (slot 0–3).
    ActionTrackSpeed(u8),
    /// Per-action-event event ID (slot 0–3).
    ActionEventId(u8),
}

impl ConditionalLineField {
    /// Returns the CDI element type for this field.
    pub fn element_type_hint(&self) -> &'static str {
        match self {
            Self::Description => "string",
            Self::V1SetTrueEvent | Self::V1SetFalseEvent | Self::ActionEventId(_) => "eventId",
            _ => "int",
        }
    }
}

/// A field write with logical identity, not yet resolved to a memory address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedFieldWrite {
    /// Which field within the conditional line.
    pub field: ConditionalLineField,
    /// 0-based conditional line index.
    pub line_index: u32,
    /// Raw value bytes.
    pub value: Vec<u8>,
}

/// A resolved memory address for a CDI leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAddress {
    /// Memory space number.
    pub space: u8,
    /// Byte address within the memory space.
    pub address: u32,
}

/// Internal output of the compiler before address resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledLogicOutput {
    /// The allocation record.
    pub allocation: LogicAllocation,
    /// Field writes with logical identity (no addresses yet).
    pub unresolved_writes: Vec<UnresolvedFieldWrite>,
}

// ── CDI Enum Types ────────────────────────────────────────────────────────

/// Tower LCC conditional line function (mast group structure).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalFunction {
    Blocked = 0,
    Group = 1,
    Last = 3,
}

/// How a variable is triggered.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableTrigger {
    OnVariableChange = 0,
    OnMatchingEvent = 1,
    None = 2,
}

/// Variable data source.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableSource {
    Events = 0,
    TrackCircuit1 = 1,
    TrackCircuit2 = 2,
    TrackCircuit3 = 3,
    TrackCircuit4 = 4,
    TrackCircuit5 = 5,
    TrackCircuit6 = 6,
    TrackCircuit7 = 7,
    TrackCircuit8 = 8,
}

impl VariableSource {
    fn from_track_circuit(n: u8) -> Self {
        match n {
            1 => Self::TrackCircuit1,
            2 => Self::TrackCircuit2,
            3 => Self::TrackCircuit3,
            4 => Self::TrackCircuit4,
            5 => Self::TrackCircuit5,
            6 => Self::TrackCircuit6,
            7 => Self::TrackCircuit7,
            8 => Self::TrackCircuit8,
            _ => panic!("invalid track circuit number: {n} (must be 1–8)"),
        }
    }
}

/// Track speed values for track circuit variables.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackSpeed {
    Stop = 0,
    RestrictedSpeed = 1,
    Slow = 2,
    Medium = 3,
    LimitedSpeed = 4,
    Approach = 5,
    AdvancedApproach = 6,
    Clear = 7,
}

/// Logic operation combining V1 and V2.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicOperation {
    And = 0,
    Or = 1,
    NullTrue = 6,
    V1Only = 7,
    V2Only = 8,
}

/// Exit behavior after condition evaluation.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionBehavior {
    SendThenExit = 0,
    SendThenEvalNext = 2,
    ExitGroup = 3,
    EvalNext = 4,
}

/// Condition controlling when an action event fires.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCondition {
    None = 0,
    Immediately = 1,
    ImmediateIfTrue = 3,
    ImmediateIfFalse = 4,
}

/// Destination for an action event.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionDestination {
    Event = 0,
}

// ── Compiler Input Types ──────────────────────────────────────────────────

/// Block-occupancy channel event IDs resolved from the CDI tree.
#[derive(Debug, Clone)]
pub struct InputChannelEvents {
    /// Event ID for the "occupied" state (V1 set-true trigger).
    pub set_true_event: [u8; 8],
    /// Event ID for the "clear" state (V1 set-false trigger).
    pub set_false_event: [u8; 8],
}

/// On/Off event IDs for one output pin (one lamp row).
#[derive(Debug, Clone)]
pub struct PinEvents {
    /// Lamp On event ID for this pin's lamp row.
    pub on_event: [u8; 8],
    /// Lamp Off event ID for this pin's lamp row.
    pub off_event: [u8; 8],
}

/// Reference to a downstream signal's track circuit input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownstreamBinding {
    /// Track circuit input number (1–8) on the target node.
    pub track_circuit: u8,
    /// Speed value to match (e.g. Stop for Approach detection).
    pub speed: TrackSpeed,
}

/// All inputs needed to compile a facility's behavior template.
///
/// Built by the IPC command from LayoutState effective views; the compiler
/// itself is pure and does not read from any external state.
#[derive(Debug, Clone)]
pub struct CompileInput {
    pub template_id: String,
    pub facility_id: String,
    /// Human-readable facility name for description generation.
    pub facility_name: String,
    pub target_node_key: String,
    pub existing_allocations: Vec<LogicAllocation>,
    /// Resolved event IDs from the input (block-occupancy) channel.
    pub input_events: InputChannelEvents,
    /// Resolved On/Off event IDs per output pin (lamp row).
    pub output_pin_events: Vec<PinEvents>,
    /// Downstream signal binding for Approach rule. `None` = end-of-line.
    pub downstream: Option<DownstreamBinding>,
}

// ── Aspect-to-Pin-Action Map (D2:A — compiler-owned) ──────────────────────

/// One pin's target state for an aspect.
struct PinAction {
    pin_index: usize,
    on: bool,
}

/// Maps an aspect name to its pin actions.
struct AspectPinMapping {
    aspect: &'static str,
    actions: &'static [PinAction],
}

/// 2-LED bicolor aspect-to-pin-action map.
///
/// Pin 0 = red LED, Pin 1 = green LED.
/// - stop:     red ON,  green OFF
/// - approach: red ON,  green ON  (both = yellow)
/// - clear:    red OFF, green ON
const BICOLOR_2LED_ASPECT_MAP: &[AspectPinMapping] = &[
    AspectPinMapping {
        aspect: "stop",
        actions: &[
            PinAction { pin_index: 0, on: true },
            PinAction { pin_index: 1, on: false },
        ],
    },
    AspectPinMapping {
        aspect: "approach",
        actions: &[
            PinAction { pin_index: 0, on: true },
            PinAction { pin_index: 1, on: true },
        ],
    },
    AspectPinMapping {
        aspect: "clear",
        actions: &[
            PinAction { pin_index: 0, on: false },
            PinAction { pin_index: 1, on: true },
        ],
    },
];

fn find_aspect_pin_actions(aspect: &str) -> Option<&'static [PinAction]> {
    BICOLOR_2LED_ASPECT_MAP
        .iter()
        .find(|m| m.aspect == aspect)
        .map(|m| m.actions)
}

// ── Compiler ──────────────────────────────────────────────────────────────

/// Compile a facility's behavior template into a `CompiledLogicPlan`.
///
/// Expands condition-action rules into Tower LCC conditional line
/// configurations. Rules are sorted by priority (most restrictive first),
/// filtered (Approach omitted when no downstream), and each rule produces
/// one conditional line with correct mast group flags, variable inputs,
/// logic operation, and aspect-driven action events.
pub fn compile_facility(
    input: &CompileInput,
) -> Result<CompiledLogicOutput, CompileError> {
    let template = crate::behavior_templates::find_template(&input.template_id)
        .ok_or_else(|| CompileError::UnknownTemplate {
            template_id: input.template_id.clone(),
        })?;

    if template.compilation_target != crate::behavior_templates::CompilationTarget::Compiled {
        return Err(CompileError::NotCompiled {
            template_id: input.template_id.clone(),
        });
    }

    // Filter rules: omit Approach when no downstream binding.
    let mut rules: Vec<&crate::behavior_templates::ConditionActionRule> = template
        .rules
        .iter()
        .filter(|r| !(r.aspect == "approach" && input.downstream.is_none()))
        .collect();
    rules.sort_by_key(|r| r.priority);

    // Validate action counts against the per-line limit.
    for rule in &rules {
        if let Some(pin_actions) = find_aspect_pin_actions(rule.aspect) {
            if pin_actions.len() > MAX_ACTIONS_PER_LINE {
                return Err(CompileError::TooManyActions {
                    aspect: rule.aspect.to_string(),
                    action_count: pin_actions.len(),
                    max_actions: MAX_ACTIONS_PER_LINE,
                });
            }
        }
        // Aspects without a pin mapping produce zero action events (valid).
    }

    // Check capacity.
    let required = rules.len() as u32;
    let used = used_lines_on_node(&input.target_node_key, &input.existing_allocations);
    let available = MAX_CONDITIONAL_LINES.saturating_sub(used);
    if required > available {
        return Err(CompileError::InsufficientCapacity { required, available });
    }

    let start = used;
    let allocation = LogicAllocation {
        facility_id: input.facility_id.clone(),
        target_node_key: input.target_node_key.clone(),
        conditional_lines: ConditionalLineRange {
            start,
            count: required,
        },
    };

    let mut field_writes = Vec::new();
    for (i, rule) in rules.iter().enumerate() {
        let line_index = start + i as u32;
        let is_last = i == rules.len() - 1;
        compile_rule_to_field_writes(
            rule,
            input,
            line_index,
            is_last,
            &mut field_writes,
        );
    }

    Ok(CompiledLogicOutput {
        allocation,
        unresolved_writes: field_writes,
    })
}

// ── Per-rule field-write generation ───────────────────────────────────────

/// Emit unresolved field writes for one conditional line from a rule.
fn compile_rule_to_field_writes(
    rule: &crate::behavior_templates::ConditionActionRule,
    input: &CompileInput,
    line_index: u32,
    is_last: bool,
    writes: &mut Vec<UnresolvedFieldWrite>,
) {
    // Description
    let desc = format!("{} - {}", input.facility_name, rule.label);
    let mut desc_bytes = desc.into_bytes();
    desc_bytes.truncate(DESCRIPTION_MAX_LEN);
    if desc_bytes.len() < DESCRIPTION_MAX_LEN {
        desc_bytes.push(0);
    }
    writes.push(ufw(ConditionalLineField::Description, line_index, desc_bytes));

    // Function (mast group structure)
    let function = if is_last {
        ConditionalFunction::Last
    } else {
        ConditionalFunction::Group
    };
    writes.push(ufw(ConditionalLineField::Function, line_index, vec![function as u8]));

    // Variable 1 setup + logic operation
    if is_last {
        // Default/fallback rule: NullTrue — always evaluates true.
        writes.push(ufw(ConditionalLineField::V1Trigger, line_index, vec![VariableTrigger::None as u8]));
        writes.push(ufw(ConditionalLineField::LogicOperation, line_index, vec![LogicOperation::NullTrue as u8]));
    } else if rule.aspect == "approach" {
        // Approach rule: V1 reads downstream signal via Track Circuit.
        let ds = input.downstream.as_ref().expect("downstream filtered above");
        writes.push(ufw(ConditionalLineField::V1Trigger, line_index, vec![VariableTrigger::OnVariableChange as u8]));
        writes.push(ufw(ConditionalLineField::V1Source, line_index, vec![VariableSource::from_track_circuit(ds.track_circuit) as u8]));
        writes.push(ufw(ConditionalLineField::V1TrackSpeed, line_index, vec![ds.speed as u8]));
        writes.push(ufw(ConditionalLineField::LogicOperation, line_index, vec![LogicOperation::V1Only as u8]));
    } else {
        // Input-condition rule (e.g. Stop): V1 from block-occupancy events.
        writes.push(ufw(ConditionalLineField::V1Trigger, line_index, vec![VariableTrigger::OnMatchingEvent as u8]));
        writes.push(ufw(ConditionalLineField::V1Source, line_index, vec![VariableSource::Events as u8]));
        writes.push(ufw(ConditionalLineField::V1SetTrueEvent, line_index, input.input_events.set_true_event.to_vec()));
        writes.push(ufw(ConditionalLineField::V1SetFalseEvent, line_index, input.input_events.set_false_event.to_vec()));
        writes.push(ufw(ConditionalLineField::LogicOperation, line_index, vec![LogicOperation::V1Only as u8]));
    }

    // V2 unused — explicitly set to None to clear any stale data.
    writes.push(ufw(ConditionalLineField::V2Trigger, line_index, vec![VariableTrigger::None as u8]));

    // Exit behavior
    writes.push(ufw(ConditionalLineField::ActionWhenTrue, line_index, vec![ActionBehavior::SendThenExit as u8]));
    let when_false = if is_last {
        ActionBehavior::SendThenExit
    } else {
        ActionBehavior::EvalNext
    };
    writes.push(ufw(ConditionalLineField::ActionWhenFalse, line_index, vec![when_false as u8]));

    // Action events from the aspect-to-pin map (one write per sub-field).
    let pin_actions = find_aspect_pin_actions(rule.aspect).unwrap_or(&[]);
    let action_cond = if is_last {
        ActionCondition::Immediately
    } else {
        ActionCondition::ImmediateIfTrue
    };

    for slot in 0..MAX_ACTIONS_PER_LINE {
        let s = slot as u8;
        if slot < pin_actions.len() {
            let pa = &pin_actions[slot];
            let event_id = if pa.on {
                &input.output_pin_events[pa.pin_index].on_event
            } else {
                &input.output_pin_events[pa.pin_index].off_event
            };
            writes.push(ufw(ConditionalLineField::ActionCondition(s), line_index, vec![action_cond as u8]));
            writes.push(ufw(ConditionalLineField::ActionDestination(s), line_index, vec![ActionDestination::Event as u8]));
            writes.push(ufw(ConditionalLineField::ActionTrackSpeed(s), line_index, vec![0u8]));
            writes.push(ufw(ConditionalLineField::ActionEventId(s), line_index, event_id.to_vec()));
        } else {
            // Unused action event slot — zeroed out.
            writes.push(ufw(ConditionalLineField::ActionCondition(s), line_index, vec![0u8]));
            writes.push(ufw(ConditionalLineField::ActionDestination(s), line_index, vec![0u8]));
            writes.push(ufw(ConditionalLineField::ActionTrackSpeed(s), line_index, vec![0u8]));
            writes.push(ufw(ConditionalLineField::ActionEventId(s), line_index, vec![0u8; 8]));
        }
    }
}

// ── Field-write builder ───────────────────────────────────────────────────

fn ufw(field: ConditionalLineField, line_index: u32, value: Vec<u8>) -> UnresolvedFieldWrite {
    UnresolvedFieldWrite {
        field,
        line_index,
        value,
    }
}

/// Join unresolved field writes with resolved addresses to produce
/// concrete CDI field writes.
pub fn resolve_field_writes(
    unresolved: &[UnresolvedFieldWrite],
    address_map: &HashMap<(ConditionalLineField, u32), ResolvedAddress>,
) -> Vec<CompiledFieldWrite> {
    unresolved
        .iter()
        .filter_map(|u| {
            let key = (u.field, u.line_index);
            address_map.get(&key).map(|addr| CompiledFieldWrite {
                leaf_path: format!("{:?}[{}]", u.field, u.line_index),
                space: addr.space,
                address: addr.address as u64,
                value: u.value.clone(),
                element_type: u.field.element_type_hint().to_string(),
            })
        })
        .collect()
}

/// Walk a node's config tree to build the field → address map for
/// conditional lines.  Returns an empty map if the tree lacks a
/// "Conditionals" segment.
pub fn build_conditional_line_address_map(
    tree: &NodeConfigTree,
) -> HashMap<(ConditionalLineField, u32), ResolvedAddress> {
    let mut map = HashMap::new();

    // Find the "Conditionals" segment.
    let segment = match tree.segments.iter().find(|s| s.name == "Conditionals") {
        Some(s) => s,
        None => return map,
    };

    // Find all Logic group instances using the replication_instances helper
    // (ADR-0013: replicated groups use a wrapper with instance==0; real
    // instances live one level deeper with instance > 0).
    let logic_instances = replication_instances(&segment.children, "Logic");
    for logic_group in logic_instances {
        let line_index = logic_group.instance.saturating_sub(1); // 1-based → 0-based
        map_logic_group_fields(logic_group, line_index, &mut map);
    }

    map
}

/// Map fields within a single Logic group instance to resolved addresses.
fn map_logic_group_fields(
    group: &GroupNode,
    line_index: u32,
    map: &mut HashMap<(ConditionalLineField, u32), ResolvedAddress>,
) {
    for child in &group.children {
        match child {
            ConfigNode::Leaf(leaf) => {
                let field = match leaf.name.as_str() {
                    "Description" => Some(ConditionalLineField::Description),
                    "Function" => Some(ConditionalLineField::Function),
                    "Logic Operation" => Some(ConditionalLineField::LogicOperation),
                    _ => None,
                };
                if let Some(f) = field {
                    map.insert(
                        (f, line_index),
                        ResolvedAddress { space: leaf.space, address: leaf.address },
                    );
                }
            }
            ConfigNode::Group(sub) => {
                match sub.replication_of.as_str() {
                    "Variable #1" => map_variable1_fields(sub, line_index, map),
                    "Variable #2" => map_variable2_fields(sub, line_index, map),
                    "Action" => {
                        if sub.instance == 0 {
                            // ADR-0013: wrapper for replicated Action groups —
                            // real instances live inside with instance > 0.
                            for wrapper_child in &sub.children {
                                if let ConfigNode::Group(action_inst) = wrapper_child {
                                    if action_inst.replication_of == "Action" && action_inst.instance > 0 {
                                        let slot = action_inst.instance.saturating_sub(1) as u8;
                                        map_action_event_fields(action_inst, line_index, slot, map);
                                    }
                                }
                            }
                        } else if sub.replication_count == 1 {
                            // Non-replicated: exit behavior (when true / when false)
                            map_exit_action_fields(sub, line_index, map);
                        } else {
                            // Direct instance (sibling shape — test fixtures)
                            let slot = sub.instance.saturating_sub(1) as u8;
                            map_action_event_fields(sub, line_index, slot, map);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn map_variable1_fields(
    group: &GroupNode,
    line_index: u32,
    map: &mut HashMap<(ConditionalLineField, u32), ResolvedAddress>,
) {
    for child in &group.children {
        if let ConfigNode::Leaf(leaf) = child {
            let field = match leaf.name.as_str() {
                "Trigger" => Some(ConditionalLineField::V1Trigger),
                "Source" => Some(ConditionalLineField::V1Source),
                "Track Speed" => Some(ConditionalLineField::V1TrackSpeed),
                "set true" => Some(ConditionalLineField::V1SetTrueEvent),
                "set false" => Some(ConditionalLineField::V1SetFalseEvent),
                _ => None,
            };
            if let Some(f) = field {
                map.insert(
                    (f, line_index),
                    ResolvedAddress { space: leaf.space, address: leaf.address },
                );
            }
        }
    }
}

fn map_variable2_fields(
    group: &GroupNode,
    line_index: u32,
    map: &mut HashMap<(ConditionalLineField, u32), ResolvedAddress>,
) {
    for child in &group.children {
        if let ConfigNode::Leaf(leaf) = child {
            if leaf.name == "Trigger" {
                map.insert(
                    (ConditionalLineField::V2Trigger, line_index),
                    ResolvedAddress { space: leaf.space, address: leaf.address },
                );
            }
        }
    }
}

fn map_exit_action_fields(
    group: &GroupNode,
    line_index: u32,
    map: &mut HashMap<(ConditionalLineField, u32), ResolvedAddress>,
) {
    for child in &group.children {
        if let ConfigNode::Leaf(leaf) = child {
            let field = match leaf.name.as_str() {
                "when true" => Some(ConditionalLineField::ActionWhenTrue),
                "when false" => Some(ConditionalLineField::ActionWhenFalse),
                _ => None,
            };
            if let Some(f) = field {
                map.insert(
                    (f, line_index),
                    ResolvedAddress { space: leaf.space, address: leaf.address },
                );
            }
        }
    }
}

fn map_action_event_fields(
    group: &GroupNode,
    line_index: u32,
    slot: u8,
    map: &mut HashMap<(ConditionalLineField, u32), ResolvedAddress>,
) {
    for child in &group.children {
        if let ConfigNode::Leaf(leaf) = child {
            let field = match leaf.name.as_str() {
                "Condition" => Some(ConditionalLineField::ActionCondition(slot)),
                "Destination" => Some(ConditionalLineField::ActionDestination(slot)),
                "Track Speed" => Some(ConditionalLineField::ActionTrackSpeed(slot)),
                "Action Event" => Some(ConditionalLineField::ActionEventId(slot)),
                _ => None,
            };
            if let Some(f) = field {
                map.insert(
                    (f, line_index),
                    ResolvedAddress { space: leaf.space, address: leaf.address },
                );
            }
        }
    }
}

/// Count how many conditional lines are already allocated on a given node.
fn used_lines_on_node(node_key: &str, allocations: &[LogicAllocation]) -> u32 {
    allocations
        .iter()
        .filter(|a| a.target_node_key == node_key)
        .map(|a| a.conditional_lines.count)
        .sum()
}

/// Query the capacity of a logic target node given existing allocations.
pub fn get_capacity(
    target_node_key: &str,
    existing_allocations: &[LogicAllocation],
) -> LogicCapacity {
    let used = used_lines_on_node(target_node_key, existing_allocations);
    LogicCapacity {
        total_lines: MAX_CONDITIONAL_LINES,
        used_lines: used,
    }
}

/// Returns `true` if the node's config tree contains a "Conditionals"
/// segment with at least one Logic group (i.e. the node has conditional
/// line CDI fields). Used by the IPC layer to filter the candidate list
/// so only Tower-LCC-class nodes appear in the logic target picker.
pub fn has_conditional_lines(tree: &NodeConfigTree) -> bool {
    !build_conditional_line_address_map(tree).is_empty()
}

// ── Downstream resolution (Spec 020 / S4) ────────────────────────────────

use crate::layout::facilities::Facility;

/// Resolve the downstream signal binding for a facility.
///
/// Looks up the `downstream-signal` slot binding on the compiling facility,
/// finds which other facility owns that channel as its output, and checks
/// whether that downstream facility already has a logic allocation. If so,
/// produces a `DownstreamBinding` with a hardcoded Track Circuit 1 (S5 will
/// allocate dynamically). Returns `None` if:
/// - The slot is unbound (end-of-line signal)
/// - The bound channel is not the output of any other facility
/// - The downstream facility has no allocation yet (deferred compilation)
pub fn resolve_downstream_binding(
    facility: &Facility,
    all_facilities: &[Facility],
    allocations: &[LogicAllocation],
) -> Option<DownstreamBinding> {
    // 1. Get the downstream-signal slot binding.
    let downstream_channel_id = facility
        .slot_bindings
        .get("downstream-signal")
        .and_then(|v| v.first())?;

    // 2. Find which facility has this channel as its "output" slot binding.
    let downstream_facility = all_facilities.iter().find(|f| {
        f.facility_id != facility.facility_id
            && f.slot_bindings
                .get("output")
                .map_or(false, |v| v.contains(downstream_channel_id))
    })?;

    // 3. Check if the downstream facility has a logic allocation.
    let _downstream_allocation = allocations
        .iter()
        .find(|a| a.facility_id == downstream_facility.facility_id)?;

    // 4. Produce DownstreamBinding (S4: hardcoded TC 1; S5 will allocate).
    Some(DownstreamBinding {
        track_circuit: 1,
        speed: TrackSpeed::Stop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Distinguishable test event IDs.
    const OCCUPIED_EVENT: [u8; 8] = [0, 0, 0, 0, 0, 0, 1, 1];
    const CLEAR_EVENT: [u8; 8] = [0, 0, 0, 0, 0, 0, 1, 2];
    const RED_ON_EVENT: [u8; 8] = [0, 0, 0, 0, 0, 0, 2, 1];
    const RED_OFF_EVENT: [u8; 8] = [0, 0, 0, 0, 0, 0, 2, 2];
    const GREEN_ON_EVENT: [u8; 8] = [0, 0, 0, 0, 0, 0, 2, 3];
    const GREEN_OFF_EVENT: [u8; 8] = [0, 0, 0, 0, 0, 0, 2, 4];

    /// Build a CompileInput for a standalone 3-aspect signal with downstream.
    fn make_standalone_input() -> CompileInput {
        CompileInput {
            template_id: "abs-3-aspect-signal".to_string(),
            facility_id: "facility-1".to_string(),
            facility_name: "Signal B1".to_string(),
            target_node_key: "050201020300".to_string(),
            existing_allocations: vec![],
            input_events: InputChannelEvents {
                set_true_event: OCCUPIED_EVENT,
                set_false_event: CLEAR_EVENT,
            },
            output_pin_events: vec![
                PinEvents {
                    on_event: RED_ON_EVENT,
                    off_event: RED_OFF_EVENT,
                },
                PinEvents {
                    on_event: GREEN_ON_EVENT,
                    off_event: GREEN_OFF_EVENT,
                },
            ],
            downstream: Some(DownstreamBinding {
                track_circuit: 1,
                speed: TrackSpeed::Stop,
            }),
        }
    }

    /// Build a CompileInput for an end-of-line signal (no downstream).
    fn make_end_of_line_input() -> CompileInput {
        let mut input = make_standalone_input();
        input.downstream = None;
        input
    }

    /// Find an unresolved field write by (field, line_index).
    fn find_unresolved(
        writes: &[UnresolvedFieldWrite],
        field: ConditionalLineField,
        line_index: u32,
    ) -> &UnresolvedFieldWrite {
        writes
            .iter()
            .find(|w| w.field == field && w.line_index == line_index)
            .unwrap_or_else(|| {
                panic!("no unresolved write for {:?} on line {}", field, line_index)
            })
    }

    // ── Standalone 3-aspect signal (with downstream) ─────────────────

    #[test]
    fn compiles_3_lines_with_correct_group_structure() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        assert_eq!(output.allocation.facility_id, "facility-1");
        assert_eq!(output.allocation.target_node_key, "050201020300");
        assert_eq!(output.allocation.conditional_lines.start, 0);
        assert_eq!(output.allocation.conditional_lines.count, 3);

        // Function flags: Group, Group, Last
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::Function, 0).value,
            [ConditionalFunction::Group as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::Function, 1).value,
            [ConditionalFunction::Group as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::Function, 2).value,
            [ConditionalFunction::Last as u8]
        );
    }

    #[test]
    fn evaluation_order_is_stop_approach_clear() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        let desc0 = &find_unresolved(&output.unresolved_writes, ConditionalLineField::Description, 0).value;
        assert!(String::from_utf8_lossy(desc0).contains("Stop"));

        let desc1 = &find_unresolved(&output.unresolved_writes, ConditionalLineField::Description, 1).value;
        assert!(String::from_utf8_lossy(desc1).contains("Approach"));

        let desc2 = &find_unresolved(&output.unresolved_writes, ConditionalLineField::Description, 2).value;
        assert!(String::from_utf8_lossy(desc2).contains("Clear"));
    }

    #[test]
    fn stop_line_uses_event_variables_with_block_occupancy() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1Trigger, 0).value,
            [VariableTrigger::OnMatchingEvent as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1Source, 0).value,
            [VariableSource::Events as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1SetTrueEvent, 0).value,
            OCCUPIED_EVENT
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1SetFalseEvent, 0).value,
            CLEAR_EVENT
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::LogicOperation, 0).value,
            [LogicOperation::V1Only as u8]
        );
    }

    #[test]
    fn approach_line_uses_track_circuit_from_downstream() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1Trigger, 1).value,
            [VariableTrigger::OnVariableChange as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1Source, 1).value,
            [VariableSource::TrackCircuit1 as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1TrackSpeed, 1).value,
            [TrackSpeed::Stop as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::LogicOperation, 1).value,
            [LogicOperation::V1Only as u8]
        );
    }

    #[test]
    fn clear_line_uses_null_true_logic() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::V1Trigger, 2).value,
            [VariableTrigger::None as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::LogicOperation, 2).value,
            [LogicOperation::NullTrue as u8]
        );
    }

    #[test]
    fn stop_line_actions_drive_red_on_green_off_when_true() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionWhenTrue, 0).value,
            [ActionBehavior::SendThenExit as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionWhenFalse, 0).value,
            [ActionBehavior::EvalNext as u8]
        );

        // Action 0: ImmediateIfTrue, Event, red ON
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(0), 0).value,
            [ActionCondition::ImmediateIfTrue as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionDestination(0), 0).value,
            [ActionDestination::Event as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(0), 0).value,
            RED_ON_EVENT
        );

        // Action 1: ImmediateIfTrue, Event, green OFF
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(1), 0).value,
            [ActionCondition::ImmediateIfTrue as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(1), 0).value,
            GREEN_OFF_EVENT
        );
    }

    #[test]
    fn approach_line_actions_drive_red_on_green_on_when_true() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(0), 1).value,
            [ActionCondition::ImmediateIfTrue as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(0), 1).value,
            RED_ON_EVENT
        );

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(1), 1).value,
            [ActionCondition::ImmediateIfTrue as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(1), 1).value,
            GREEN_ON_EVENT
        );
    }

    #[test]
    fn clear_line_actions_drive_red_off_green_on_immediately() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionWhenTrue, 2).value,
            [ActionBehavior::SendThenExit as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionWhenFalse, 2).value,
            [ActionBehavior::SendThenExit as u8]
        );

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(0), 2).value,
            [ActionCondition::Immediately as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(0), 2).value,
            RED_OFF_EVENT
        );

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(1), 2).value,
            [ActionCondition::Immediately as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(1), 2).value,
            GREEN_ON_EVENT
        );
    }

    #[test]
    fn unresolved_writes_carry_no_addresses() {
        let output = compile_facility(&make_standalone_input()).unwrap();
        // CompiledLogicOutput contains UnresolvedFieldWrite which has no
        // space or address fields — this is a compile-time guarantee.
        // Just verify we produced writes.
        assert!(!output.unresolved_writes.is_empty());
    }

    #[test]
    fn unused_action_slots_are_zeroed() {
        let output = compile_facility(&make_standalone_input()).unwrap();

        // Action slots 2 and 3 are unused (2-LED bicolor only uses 2 actions).
        for slot in 2..4u8 {
            assert_eq!(
                find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(slot), 0).value,
                [0u8],
                "ActionCondition({slot}) not zeroed on line 0"
            );
            assert_eq!(
                find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionDestination(slot), 0).value,
                [0u8],
                "ActionDestination({slot}) not zeroed on line 0"
            );
            assert_eq!(
                find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionTrackSpeed(slot), 0).value,
                [0u8],
                "ActionTrackSpeed({slot}) not zeroed on line 0"
            );
            assert_eq!(
                find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(slot), 0).value,
                [0u8; 8],
                "ActionEventId({slot}) not zeroed on line 0"
            );
        }
    }

    #[test]
    fn v2_trigger_set_to_none_on_all_lines() {
        let output = compile_facility(&make_standalone_input()).unwrap();
        for i in 0..3u32 {
            assert_eq!(
                find_unresolved(&output.unresolved_writes, ConditionalLineField::V2Trigger, i).value,
                [VariableTrigger::None as u8],
                "V2 trigger not None on line {i}"
            );
        }
    }

    // ── End-of-line signal (no downstream) ───────────────────────────

    #[test]
    fn end_of_line_produces_2_lines_stop_and_clear() {
        let output = compile_facility(&make_end_of_line_input()).unwrap();

        assert_eq!(output.allocation.conditional_lines.count, 2);

        // Group, Last
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::Function, 0).value,
            [ConditionalFunction::Group as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::Function, 1).value,
            [ConditionalFunction::Last as u8]
        );

        // Line 0 = Stop, Line 1 = Clear (Approach omitted)
        let desc0 = &find_unresolved(&output.unresolved_writes, ConditionalLineField::Description, 0).value;
        assert!(String::from_utf8_lossy(desc0).contains("Stop"));
        let desc1 = &find_unresolved(&output.unresolved_writes, ConditionalLineField::Description, 1).value;
        assert!(String::from_utf8_lossy(desc1).contains("Clear"));
    }

    #[test]
    fn end_of_line_clear_line_is_null_true_with_correct_actions() {
        let output = compile_facility(&make_end_of_line_input()).unwrap();

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::LogicOperation, 1).value,
            [LogicOperation::NullTrue as u8]
        );

        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionCondition(0), 1).value,
            [ActionCondition::Immediately as u8]
        );
        assert_eq!(
            find_unresolved(&output.unresolved_writes, ConditionalLineField::ActionEventId(0), 1).value,
            RED_OFF_EVENT
        );
    }

    // ── Allocation + error handling ──────────────────────────────────

    #[test]
    fn allocates_after_existing_usage() {
        let mut input = make_standalone_input();
        input.existing_allocations = vec![LogicAllocation {
            facility_id: "other".to_string(),
            target_node_key: "050201020300".to_string(),
            conditional_lines: ConditionalLineRange { start: 0, count: 5 },
        }];

        let output = compile_facility(&input).unwrap();
        assert_eq!(output.allocation.conditional_lines.start, 5);
        assert_eq!(output.allocation.conditional_lines.count, 3);

        // Field writes reference line index 5.
        assert!(output
            .unresolved_writes
            .iter()
            .any(|w| w.field == ConditionalLineField::Function && w.line_index == 5));
    }

    #[test]
    fn rejects_when_capacity_exceeded() {
        let mut input = make_standalone_input();
        input.existing_allocations = vec![LogicAllocation {
            facility_id: "other".to_string(),
            target_node_key: "050201020300".to_string(),
            conditional_lines: ConditionalLineRange { start: 0, count: 31 },
        }];

        let err = compile_facility(&input).unwrap_err();
        assert_eq!(
            err,
            CompileError::InsufficientCapacity {
                required: 3,
                available: 1
            }
        );
    }

    #[test]
    fn rejects_composed_template() {
        let mut input = make_standalone_input();
        input.template_id = "block-indicator".to_string();

        let err = compile_facility(&input).unwrap_err();
        assert_eq!(
            err,
            CompileError::NotCompiled {
                template_id: "block-indicator".to_string()
            }
        );
    }

    #[test]
    fn rejects_unknown_template() {
        let mut input = make_standalone_input();
        input.template_id = "nonexistent".to_string();

        let err = compile_facility(&input).unwrap_err();
        assert_eq!(
            err,
            CompileError::UnknownTemplate {
                template_id: "nonexistent".to_string()
            }
        );
    }

    // ── Capacity queries (unchanged) ─────────────────────────────────

    #[test]
    fn capacity_query_reflects_existing_allocations() {
        let allocs = vec![
            LogicAllocation {
                facility_id: "a".to_string(),
                target_node_key: "NODE1".to_string(),
                conditional_lines: ConditionalLineRange { start: 0, count: 10 },
            },
            LogicAllocation {
                facility_id: "b".to_string(),
                target_node_key: "NODE1".to_string(),
                conditional_lines: ConditionalLineRange { start: 10, count: 5 },
            },
        ];

        let cap = get_capacity("NODE1", &allocs);
        assert_eq!(cap.total_lines, 32);
        assert_eq!(cap.used_lines, 15);
        assert_eq!(cap.available(), 17);
    }

    #[test]
    fn capacity_query_ignores_other_nodes() {
        let allocs = vec![LogicAllocation {
            facility_id: "a".to_string(),
            target_node_key: "OTHER".to_string(),
            conditional_lines: ConditionalLineRange { start: 0, count: 20 },
        }];

        let cap = get_capacity("NODE1", &allocs);
        assert_eq!(cap.used_lines, 0);
        assert_eq!(cap.available(), 32);
    }

    // ── JSON round-trip (unchanged) ──────────────────────────────────

    #[test]
    fn allocation_round_trips_json() {
        let alloc = LogicAllocation {
            facility_id: "f1".to_string(),
            target_node_key: "050201020300".to_string(),
            conditional_lines: ConditionalLineRange { start: 5, count: 3 },
        };
        let json = serde_json::to_value(&alloc).unwrap();
        assert_eq!(json["facilityId"], "f1");
        assert_eq!(json["targetNodeKey"], "050201020300");
        assert_eq!(json["conditionalLines"]["start"], 5);
        assert_eq!(json["conditionalLines"]["count"], 3);

        let parsed: LogicAllocation = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, alloc);
    }

    #[test]
    fn compiled_plan_round_trips_json() {
        let plan = CompiledLogicPlan {
            allocation: LogicAllocation {
                facility_id: "f1".to_string(),
                target_node_key: "NODE1".to_string(),
                conditional_lines: ConditionalLineRange { start: 0, count: 1 },
            },
            field_writes: vec![CompiledFieldWrite {
                leaf_path: "conditionalLine[0]/description".to_string(),
                space: 253,
                address: 2528,
                value: vec![83, 116, 111, 112], // "Stop"
                element_type: "string".to_string(),
            }],
        };
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: CompiledLogicPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, plan);
    }

    // ── resolve_field_writes ─────────────────────────────────────────

    #[test]
    fn resolve_field_writes_joins_with_address_map() {
        let unresolved = vec![
            UnresolvedFieldWrite {
                field: ConditionalLineField::Function,
                line_index: 0,
                value: vec![1],
            },
            UnresolvedFieldWrite {
                field: ConditionalLineField::V1Trigger,
                line_index: 0,
                value: vec![2],
            },
        ];
        let mut address_map = std::collections::HashMap::new();
        address_map.insert(
            (ConditionalLineField::Function, 0),
            ResolvedAddress { space: 253, address: 5000 },
        );
        address_map.insert(
            (ConditionalLineField::V1Trigger, 0),
            ResolvedAddress { space: 253, address: 5001 },
        );

        let resolved = resolve_field_writes(&unresolved, &address_map);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].space, 253);
        assert_eq!(resolved[0].address, 5000);
        assert_eq!(resolved[0].value, [1]);
        assert_eq!(resolved[0].element_type, "int");
        assert_eq!(resolved[1].address, 5001);
        assert_eq!(resolved[1].value, [2]);
        assert_eq!(resolved[1].element_type, "int");
    }

    #[test]
    fn resolve_field_writes_skips_unmapped_fields() {
        let unresolved = vec![
            UnresolvedFieldWrite {
                field: ConditionalLineField::Function,
                line_index: 0,
                value: vec![1],
            },
        ];
        let address_map = HashMap::new(); // empty

        let resolved = resolve_field_writes(&unresolved, &address_map);
        assert!(resolved.is_empty());
    }

    // ── build_conditional_line_address_map ────────────────────────────

    use crate::node_tree::{LeafNode, LeafType, SegmentNode};

    fn make_leaf(name: &str, element_type: LeafType, address: u32, space: u8, size: u32) -> ConfigNode {
        ConfigNode::Leaf(LeafNode {
            name: name.to_string(),
            description: None,
            element_type,
            address,
            size,
            space,
            path: vec![],
            value: None,
            event_role: None,
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
        })
    }

    fn make_group(name: &str, instance: u32, replication_of: &str, replication_count: u32, children: Vec<ConfigNode>) -> ConfigNode {
        ConfigNode::Group(GroupNode {
            name: name.to_string(),
            has_name: true,
            description: None,
            instance,
            instance_label: format!("{} {}", name, instance),
            replication_of: replication_of.to_string(),
            replication_count,
            path: vec![],
            children,
            display_name: None,
            hideable: false,
            hidden_by_default: false,
            read_only: false,
        })
    }

    /// Build a single "Logic" group instance with realistic test addresses.
    fn make_logic_line(instance: u32, base_addr: u32) -> ConfigNode {
        make_group("Logic", instance, "Logic", 32, vec![
            make_leaf("Description", LeafType::String, base_addr, 253, 32),
            make_leaf("Function", LeafType::Int, base_addr + 100, 253, 1),
            make_group("Variable #1", 1, "Variable #1", 1, vec![
                make_leaf("Trigger", LeafType::Int, base_addr + 200, 253, 1),
                make_leaf("Source", LeafType::Int, base_addr + 201, 253, 1),
                make_leaf("Track Speed", LeafType::Int, base_addr + 202, 253, 1),
                make_leaf("set true", LeafType::EventId, base_addr + 203, 253, 8),
                make_leaf("set false", LeafType::EventId, base_addr + 211, 253, 8),
            ]),
            make_leaf("Logic Operation", LeafType::Int, base_addr + 300, 253, 1),
            make_group("Variable #2", 1, "Variable #2", 1, vec![
                make_leaf("Trigger", LeafType::Int, base_addr + 400, 253, 1),
            ]),
            // Non-replicated Action group (exit behavior)
            make_group("Action", 1, "Action", 1, vec![
                make_leaf("when true", LeafType::Int, base_addr + 500, 253, 1),
                make_leaf("when false", LeafType::Int, base_addr + 501, 253, 1),
            ]),
            // Replicated Action group (4 action event slots)
            make_group("Action", 1, "Action", 4, vec![
                make_leaf("Condition", LeafType::Int, base_addr + 600, 253, 1),
                make_leaf("Destination", LeafType::Int, base_addr + 601, 253, 1),
                make_leaf("Track Speed", LeafType::Int, base_addr + 602, 253, 1),
                make_leaf("Action Event", LeafType::EventId, base_addr + 603, 253, 8),
            ]),
            make_group("Action", 2, "Action", 4, vec![
                make_leaf("Condition", LeafType::Int, base_addr + 700, 253, 1),
                make_leaf("Destination", LeafType::Int, base_addr + 701, 253, 1),
                make_leaf("Track Speed", LeafType::Int, base_addr + 702, 253, 1),
                make_leaf("Action Event", LeafType::EventId, base_addr + 703, 253, 8),
            ]),
            make_group("Action", 3, "Action", 4, vec![
                make_leaf("Condition", LeafType::Int, base_addr + 800, 253, 1),
                make_leaf("Destination", LeafType::Int, base_addr + 801, 253, 1),
                make_leaf("Track Speed", LeafType::Int, base_addr + 802, 253, 1),
                make_leaf("Action Event", LeafType::EventId, base_addr + 803, 253, 8),
            ]),
            make_group("Action", 4, "Action", 4, vec![
                make_leaf("Condition", LeafType::Int, base_addr + 900, 253, 1),
                make_leaf("Destination", LeafType::Int, base_addr + 901, 253, 1),
                make_leaf("Track Speed", LeafType::Int, base_addr + 902, 253, 1),
                make_leaf("Action Event", LeafType::EventId, base_addr + 903, 253, 8),
            ]),
        ])
    }

    fn make_test_tree_with_conditionals() -> NodeConfigTree {
        // Line 0 at base 2528, Line 1 at base 10000 (dual-pool jump).
        let line0 = make_logic_line(1, 2528);
        let line1 = make_logic_line(2, 10000);

        NodeConfigTree {
            node_id: "05.02.01.02.03.00".to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: false,
            segments: vec![
                SegmentNode {
                    name: "Conditionals".to_string(),
                    description: None,
                    origin: 0,
                    space: 253,
                    children: vec![line0, line1],
                },
            ],
        }
    }

    #[test]
    fn tree_walker_maps_fields_to_addresses_for_two_lines() {
        let tree = make_test_tree_with_conditionals();
        let map = build_conditional_line_address_map(&tree);

        // Line 0 (instance 1, base 2528)
        let addr = |f: ConditionalLineField, li: u32| -> &ResolvedAddress {
            map.get(&(f, li)).unwrap_or_else(|| panic!("missing {:?} line {}", f, li))
        };

        assert_eq!(addr(ConditionalLineField::Description, 0).address, 2528);
        assert_eq!(addr(ConditionalLineField::Function, 0).address, 2628);
        assert_eq!(addr(ConditionalLineField::V1Trigger, 0).address, 2728);
        assert_eq!(addr(ConditionalLineField::V1Source, 0).address, 2729);
        assert_eq!(addr(ConditionalLineField::V1TrackSpeed, 0).address, 2730);
        assert_eq!(addr(ConditionalLineField::V1SetTrueEvent, 0).address, 2731);
        assert_eq!(addr(ConditionalLineField::V1SetFalseEvent, 0).address, 2739);
        assert_eq!(addr(ConditionalLineField::LogicOperation, 0).address, 2828);
        assert_eq!(addr(ConditionalLineField::V2Trigger, 0).address, 2928);
        assert_eq!(addr(ConditionalLineField::ActionWhenTrue, 0).address, 3028);
        assert_eq!(addr(ConditionalLineField::ActionWhenFalse, 0).address, 3029);
        assert_eq!(addr(ConditionalLineField::ActionCondition(0), 0).address, 3128);
        assert_eq!(addr(ConditionalLineField::ActionDestination(0), 0).address, 3129);
        assert_eq!(addr(ConditionalLineField::ActionTrackSpeed(0), 0).address, 3130);
        assert_eq!(addr(ConditionalLineField::ActionEventId(0), 0).address, 3131);
        assert_eq!(addr(ConditionalLineField::ActionCondition(1), 0).address, 3228);
        assert_eq!(addr(ConditionalLineField::ActionEventId(3), 0).address, 3431);

        // All addresses in space 253.
        for (_, resolved) in &map {
            assert_eq!(resolved.space, 253);
        }

        // Line 1 (instance 2, base 10000) — confirm dual-pool jump.
        assert_eq!(addr(ConditionalLineField::Description, 1).address, 10000);
        assert_eq!(addr(ConditionalLineField::Function, 1).address, 10100);
        assert_eq!(addr(ConditionalLineField::V1Trigger, 1).address, 10200);
    }

    #[test]
    fn tree_walker_returns_empty_map_when_no_conditionals_segment() {
        let tree = NodeConfigTree {
            node_id: "01.02.03.04.05.06".to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: false,
            segments: vec![
                SegmentNode {
                    name: "User Info".to_string(),
                    description: None,
                    origin: 0,
                    space: 253,
                    children: vec![],
                },
            ],
        };
        let map = build_conditional_line_address_map(&tree);
        assert!(map.is_empty());
    }

    #[test]
    fn has_conditional_lines_returns_false_for_tree_without_conditionals() {
        let tree = NodeConfigTree {
            node_id: "01.02.03.04.05.06".to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: false,
            segments: vec![
                SegmentNode {
                    name: "Configuration".to_string(),
                    description: None,
                    origin: 0,
                    space: 253,
                    children: vec![
                        make_leaf("Some Field", LeafType::Int, 100, 253, 1),
                    ],
                },
            ],
        };
        assert!(!has_conditional_lines(&tree));
    }

    #[test]
    fn has_conditional_lines_returns_true_for_tree_with_conditionals() {
        let tree = make_test_tree_with_conditionals();
        assert!(has_conditional_lines(&tree));
    }

    /// ADR-0013 regression: real CDI trees use wrapper groups (instance=0)
    /// containing replicated instances. This test verifies that
    /// `build_conditional_line_address_map` correctly traverses the wrapper
    /// structure (as produced by `build_node_config_tree`) rather than
    /// treating the wrapper itself as a leaf-bearing group.
    #[test]
    fn tree_walker_handles_wrapper_structure_from_real_cdi() {
        // Build a tree with the wrapper structure that build_node_config_tree produces:
        // Segment "Conditionals"
        //   └── Logic wrapper (instance=0, replication_count=32)
        //        └── Logic (instance=1) ← real instance with leaves
        let logic_inst_1 = make_group("Logic", 1, "Logic", 32, vec![
            make_leaf("Description", LeafType::String, 2528, 253, 32),
            make_leaf("Function", LeafType::Int, 2628, 253, 1),
            make_group("Variable #1", 1, "Variable #1", 1, vec![
                make_leaf("Trigger", LeafType::Int, 2728, 253, 1),
                make_leaf("Source", LeafType::Int, 2729, 253, 1),
                make_leaf("Track Speed", LeafType::Int, 2730, 253, 1),
                make_leaf("set true", LeafType::EventId, 2731, 253, 8),
                make_leaf("set false", LeafType::EventId, 2739, 253, 8),
            ]),
            make_leaf("Logic Operation", LeafType::Int, 2828, 253, 1),
            make_group("Variable #2", 1, "Variable #2", 1, vec![
                make_leaf("Trigger", LeafType::Int, 2928, 253, 1),
            ]),
            // Non-replicated Action (exit behavior)
            make_group("Action", 1, "Action", 1, vec![
                make_leaf("when true", LeafType::Int, 3028, 253, 1),
                make_leaf("when false", LeafType::Int, 3029, 253, 1),
            ]),
            // Replicated Action WRAPPER (instance=0) containing instances
            make_group("Action", 0, "Action", 4, vec![
                ConfigNode::Group(GroupNode {
                    name: "Action".to_string(),
                    has_name: true,
                    description: None,
                    instance: 1,
                    instance_label: "Action 1".to_string(),
                    replication_of: "Action".to_string(),
                    replication_count: 4,
                    path: vec![],
                    children: vec![
                        make_leaf("Condition", LeafType::Int, 3128, 253, 1),
                        make_leaf("Destination", LeafType::Int, 3129, 253, 1),
                        make_leaf("Track Speed", LeafType::Int, 3130, 253, 1),
                        make_leaf("Action Event", LeafType::EventId, 3131, 253, 8),
                    ],
                    display_name: None,
                    hideable: false,
                    hidden_by_default: false,
                    read_only: false,
                }),
                ConfigNode::Group(GroupNode {
                    name: "Action".to_string(),
                    has_name: true,
                    description: None,
                    instance: 2,
                    instance_label: "Action 2".to_string(),
                    replication_of: "Action".to_string(),
                    replication_count: 4,
                    path: vec![],
                    children: vec![
                        make_leaf("Condition", LeafType::Int, 3228, 253, 1),
                        make_leaf("Destination", LeafType::Int, 3229, 253, 1),
                        make_leaf("Track Speed", LeafType::Int, 3230, 253, 1),
                        make_leaf("Action Event", LeafType::EventId, 3231, 253, 8),
                    ],
                    display_name: None,
                    hideable: false,
                    hidden_by_default: false,
                    read_only: false,
                }),
            ]),
        ]);

        // Wrapper: instance=0 holding the Logic instances
        let logic_wrapper = make_group("Logic", 0, "Logic", 32, vec![logic_inst_1]);

        let tree = NodeConfigTree {
            node_id: "05.02.01.02.03.00".to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: false,
            segments: vec![
                SegmentNode {
                    name: "Conditionals".to_string(),
                    description: None,
                    origin: 0,
                    space: 253,
                    children: vec![logic_wrapper],
                },
            ],
        };

        assert!(has_conditional_lines(&tree));

        let map = build_conditional_line_address_map(&tree);

        // Verify line 0 (instance 1) fields are mapped
        assert_eq!(map.get(&(ConditionalLineField::Description, 0)).unwrap().address, 2528);
        assert_eq!(map.get(&(ConditionalLineField::Function, 0)).unwrap().address, 2628);
        assert_eq!(map.get(&(ConditionalLineField::V1Trigger, 0)).unwrap().address, 2728);
        assert_eq!(map.get(&(ConditionalLineField::LogicOperation, 0)).unwrap().address, 2828);
        assert_eq!(map.get(&(ConditionalLineField::V2Trigger, 0)).unwrap().address, 2928);
        assert_eq!(map.get(&(ConditionalLineField::ActionWhenTrue, 0)).unwrap().address, 3028);
        assert_eq!(map.get(&(ConditionalLineField::ActionWhenFalse, 0)).unwrap().address, 3029);
        // Replicated action slots (via wrapper)
        assert_eq!(map.get(&(ConditionalLineField::ActionCondition(0), 0)).unwrap().address, 3128);
        assert_eq!(map.get(&(ConditionalLineField::ActionEventId(0), 0)).unwrap().address, 3131);
        assert_eq!(map.get(&(ConditionalLineField::ActionCondition(1), 0)).unwrap().address, 3228);
        assert_eq!(map.get(&(ConditionalLineField::ActionEventId(1), 0)).unwrap().address, 3231);
    }

    // ── resolve_downstream_binding tests (S4) ─────────────────────────

    use crate::layout::facilities::Facility;
    use std::collections::BTreeMap;

    fn make_facility(id: &str, template: &str, bindings: Vec<(&str, Vec<&str>)>) -> Facility {
        let slot_bindings: BTreeMap<String, Vec<String>> = bindings
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into_iter().map(|s| s.to_string()).collect()))
            .collect();
        Facility {
            facility_id: id.to_string(),
            template_id: template.to_string(),
            name: format!("Facility {}", id),
            slot_bindings,
            logic_allocation: None,
        }
    }

    #[test]
    fn resolve_downstream_returns_none_when_slot_unbound() {
        let upstream = make_facility("f1", "abs-3-aspect-signal", vec![
            ("input", vec!["ch-block"]),
            ("output", vec!["ch-signal"]),
            ("downstream-signal", vec![]),
        ]);
        let all = vec![upstream.clone()];
        let allocs = vec![];

        assert_eq!(resolve_downstream_binding(&upstream, &all, &allocs), None);
    }

    #[test]
    fn resolve_downstream_returns_none_when_no_owning_facility() {
        let upstream = make_facility("f1", "abs-3-aspect-signal", vec![
            ("input", vec!["ch-block"]),
            ("output", vec!["ch-signal-1"]),
            ("downstream-signal", vec!["ch-orphan"]),
        ]);
        let all = vec![upstream.clone()];
        let allocs = vec![];

        assert_eq!(resolve_downstream_binding(&upstream, &all, &allocs), None);
    }

    #[test]
    fn resolve_downstream_returns_none_when_downstream_has_no_allocation() {
        let upstream = make_facility("f1", "abs-3-aspect-signal", vec![
            ("input", vec!["ch-block"]),
            ("output", vec!["ch-signal-1"]),
            ("downstream-signal", vec!["ch-signal-2"]),
        ]);
        let downstream = make_facility("f2", "abs-3-aspect-signal", vec![
            ("input", vec!["ch-block-2"]),
            ("output", vec!["ch-signal-2"]),
            ("downstream-signal", vec![]),
        ]);
        let all = vec![upstream.clone(), downstream];
        let allocs = vec![]; // downstream has no allocation yet

        assert_eq!(resolve_downstream_binding(&upstream, &all, &allocs), None);
    }

    #[test]
    fn resolve_downstream_returns_binding_when_downstream_is_allocated() {
        let upstream = make_facility("f1", "abs-3-aspect-signal", vec![
            ("input", vec!["ch-block"]),
            ("output", vec!["ch-signal-1"]),
            ("downstream-signal", vec!["ch-signal-2"]),
        ]);
        let downstream = make_facility("f2", "abs-3-aspect-signal", vec![
            ("input", vec!["ch-block-2"]),
            ("output", vec!["ch-signal-2"]),
            ("downstream-signal", vec![]),
        ]);
        let all = vec![upstream.clone(), downstream];
        let allocs = vec![LogicAllocation {
            facility_id: "f2".to_string(),
            target_node_key: "node-1".to_string(),
            conditional_lines: ConditionalLineRange { start: 0, count: 2 },
        }];

        let result = resolve_downstream_binding(&upstream, &all, &allocs);
        assert_eq!(
            result,
            Some(DownstreamBinding {
                track_circuit: 1,
                speed: TrackSpeed::Stop,
            })
        );
    }
}
