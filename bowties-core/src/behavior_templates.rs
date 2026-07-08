//! Behavior template registry — declarative composition of facilities.
//!
//! A behavior template defines the slot structure and producer/consumer
//! state mapping for one named facility kind (per spec 018).  Templates
//! are code-level (hardcoded here in this slice); a future loader may
//! deserialize them from YAML without changing the wire form.

use serde::{Deserialize, Serialize};

/// Whether a slot accepts a producer channel or drives a consumer channel.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SlotKind {
    Producer,
    Consumer,
}

/// How a template's runtime behavior is realised (Spec 020 / S1).
///
/// `Composed` templates produce bowties via the existing `facility_bowties`
/// composition path. `Compiled` templates produce CDI field writes via the
/// `logic_adapter` compiler. The orchestrator dispatches to the appropriate
/// path based on this field.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompilationTarget {
    /// Runtime behavior realised by bowtie composition (event wiring).
    Composed,
    /// Runtime behavior realised by logic compilation (CDI field writes).
    Compiled,
}

/// A condition in a `ConditionActionRule` — what must be true for the
/// rule's actions to fire. The stub compiler does not interpret these;
/// they are structural declarations used by the real compiler (S2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
    /// Slot label whose channel provides the input variable.
    pub input_slot: &'static str,
    /// Producer state name that triggers this rule (e.g. "occupied", "clear").
    pub producer_state: &'static str,
}

/// A condition → action rule declared by a compiled behavior template.
///
/// Each rule maps one input condition to one output aspect. The logic
/// compiler expands these into concrete CDI field writes (conditional
/// line configurations) during the compile step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConditionActionRule {
    /// Human-readable label (e.g. "Stop", "Approach", "Clear").
    pub label: &'static str,
    /// Evaluation priority: lower number = higher priority = checked first.
    pub priority: u32,
    /// What input condition triggers this rule.
    pub condition: RuleCondition,
    /// Output aspect name to command when the condition is met.
    pub aspect: &'static str,
}

/// One slot inside a behavior template.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlotDefinition {
    /// Slot label, unique within the template (e.g. `input`, `output`).
    pub label: &'static str,
    /// Human-readable name shown in the UI header (e.g. `block`, `indicator`).
    pub display_label: &'static str,
    /// Producer or consumer role for this slot.
    pub kind: SlotKind,
    /// The channel role a binding to this slot MUST carry
    /// (e.g. `block-occupancy`, `lamp-indicator`).
    pub required_role: &'static str,
    /// Minimum channels required to consider the slot complete (Spec 018 / S4 — D8).
    pub min_channels: u32,
    /// Maximum channels accepted; `None` = unbounded. Block Indicator uses
    /// `Some(1)` on both slots in S4; future ABS aspect-slot repeaters will
    /// declare higher caps.
    pub max_channels: Option<u32>,
    /// When true, the channel picker shows all role-compatible channels
    /// even if they are already bound to another facility. When false
    /// (default), channels bound elsewhere are filtered out. ABS input
    /// slots are shared (bidirectional ABS needs the same block detector
    /// on two signals); Block Indicator slots are exclusive.
    pub shared: bool,
}

impl SlotDefinition {
    /// True when `current_count` has reached the slot's `max_channels` cap.
    /// Slots with `max_channels: None` are never at max.
    pub fn is_at_max(&self, current_count: usize) -> bool {
        match self.max_channels {
            Some(max) => current_count >= max as usize,
            None => false,
        }
    }
}

/// One producer-state → consumer-command mapping that the template's
/// underlying bowtie(s) realise once the facility is Wired.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StateMapping {
    pub producer_state: &'static str,
    pub consumer_command: &'static str,
}

/// A registered behavior template.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorTemplate {
    pub template_id: &'static str,
    pub display_name: &'static str,
    pub slots: &'static [SlotDefinition],
    pub mapping: &'static [StateMapping],
    /// How the template's runtime behavior is realised. `Composed` templates
    /// use bowtie event wiring; `Compiled` templates use the logic adapter.
    pub compilation_target: CompilationTarget,
    /// Condition → action rules (only meaningful for `Compiled` templates).
    /// Empty for `Composed` templates.
    pub rules: &'static [ConditionActionRule],
}

const BLOCK_INDICATOR_SLOTS: &[SlotDefinition] = &[
    SlotDefinition {
        label: "input",
        display_label: "block",
        kind: SlotKind::Producer,
        required_role: "block-occupancy",
        min_channels: 1,
        max_channels: Some(1),
        shared: false,
    },
    SlotDefinition {
        label: "output",
        display_label: "indicator",
        kind: SlotKind::Consumer,
        required_role: "lamp-indicator",
        min_channels: 1,
        max_channels: Some(1),
        shared: false,
    },
];

const BLOCK_INDICATOR_MAPPING: &[StateMapping] = &[
    StateMapping {
        producer_state: "occupied",
        consumer_command: "lit",
    },
    StateMapping {
        producer_state: "clear",
        consumer_command: "unlit",
    },
];

/// The Block Indicator template — the only template registered in this slice.
pub const BLOCK_INDICATOR: BehaviorTemplate = BehaviorTemplate {
    template_id: "block-indicator",
    display_name: "Block Indicator",
    slots: BLOCK_INDICATOR_SLOTS,
    mapping: BLOCK_INDICATOR_MAPPING,
    compilation_target: CompilationTarget::Composed,
    rules: &[],
};

// ── ABS 3-Aspect Signal template (Spec 020 / S1) ─────────────────────────

const ABS_3_ASPECT_SLOTS: &[SlotDefinition] = &[
    SlotDefinition {
        label: "input",
        display_label: "block",
        kind: SlotKind::Producer,
        required_role: "block-occupancy",
        min_channels: 1,
        max_channels: Some(1),
        shared: true,
    },
    SlotDefinition {
        label: "output",
        display_label: "signal",
        kind: SlotKind::Consumer,
        required_role: "signal-aspect",
        min_channels: 1,
        max_channels: Some(1),
        shared: false,
    },
];

/// State mapping for the ABS template is informational — the actual
/// runtime behavior is produced by the compiled condition-action rules.
const ABS_3_ASPECT_MAPPING: &[StateMapping] = &[
    StateMapping {
        producer_state: "occupied",
        consumer_command: "stop",
    },
    StateMapping {
        producer_state: "clear",
        consumer_command: "clear",
    },
];

const ABS_3_ASPECT_RULES: &[ConditionActionRule] = &[
    ConditionActionRule {
        label: "Stop",
        priority: 1,
        condition: RuleCondition {
            input_slot: "input",
            producer_state: "occupied",
        },
        aspect: "stop",
    },
    ConditionActionRule {
        label: "Approach",
        priority: 2,
        condition: RuleCondition {
            input_slot: "input",
            producer_state: "clear",
        },
        aspect: "approach",
    },
    ConditionActionRule {
        label: "Clear",
        priority: 3,
        condition: RuleCondition {
            input_slot: "input",
            producer_state: "clear",
        },
        aspect: "clear",
    },
];

pub const ABS_3_ASPECT_SIGNAL: BehaviorTemplate = BehaviorTemplate {
    template_id: "abs-3-aspect-signal",
    display_name: "ABS 3-Aspect Signal",
    slots: ABS_3_ASPECT_SLOTS,
    mapping: ABS_3_ASPECT_MAPPING,
    compilation_target: CompilationTarget::Compiled,
    rules: ABS_3_ASPECT_RULES,
};

const REGISTRY: &[BehaviorTemplate] = &[BLOCK_INDICATOR, ABS_3_ASPECT_SIGNAL];

/// All registered templates.
pub fn registered_templates() -> &'static [BehaviorTemplate] {
    REGISTRY
}

/// Look up a template by its `template_id`.
pub fn find_template(template_id: &str) -> Option<&'static BehaviorTemplate> {
    registered_templates()
        .iter()
        .find(|t| t.template_id == template_id)
}

impl BehaviorTemplate {
    /// Look up a slot by label within this template.
    pub fn find_slot(&self, label: &str) -> Option<&'static SlotDefinition> {
        self.slots.iter().find(|s| s.label == label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_block_indicator() {
        let templates = registered_templates();
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].template_id, "block-indicator");
        assert_eq!(templates[0].display_name, "Block Indicator");
    }

    #[test]
    fn registry_contains_abs_3_aspect_signal() {
        let t = find_template("abs-3-aspect-signal").expect("ABS template registered");
        assert_eq!(t.display_name, "ABS 3-Aspect Signal");
        assert_eq!(t.compilation_target, CompilationTarget::Compiled);
    }

    #[test]
    fn abs_template_has_block_occupancy_input_and_signal_aspect_output() {
        let input = ABS_3_ASPECT_SIGNAL.find_slot("input").expect("input slot");
        assert_eq!(input.kind, SlotKind::Producer);
        assert_eq!(input.required_role, "block-occupancy");

        let output = ABS_3_ASPECT_SIGNAL.find_slot("output").expect("output slot");
        assert_eq!(output.kind, SlotKind::Consumer);
        assert_eq!(output.required_role, "signal-aspect");
    }

    #[test]
    fn abs_template_has_condition_action_rules_in_priority_order() {
        let rules = ABS_3_ASPECT_SIGNAL.rules;
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].label, "Stop");
        assert_eq!(rules[0].priority, 1);
        assert_eq!(rules[0].aspect, "stop");
        assert_eq!(rules[1].label, "Approach");
        assert_eq!(rules[1].priority, 2);
        assert_eq!(rules[2].label, "Clear");
        assert_eq!(rules[2].priority, 3);
    }

    #[test]
    fn block_indicator_is_composed_with_no_rules() {
        assert_eq!(BLOCK_INDICATOR.compilation_target, CompilationTarget::Composed);
        assert!(BLOCK_INDICATOR.rules.is_empty());
    }

    #[test]
    fn abs_template_serialises_compilation_target_and_rules() {
        let json = serde_json::to_value(&ABS_3_ASPECT_SIGNAL).unwrap();
        assert_eq!(json["compilationTarget"], "compiled");
        assert_eq!(json["rules"][0]["label"], "Stop");
        assert_eq!(json["rules"][0]["condition"]["inputSlot"], "input");
        assert_eq!(json["rules"][0]["condition"]["producerState"], "occupied");
        assert_eq!(json["rules"][0]["aspect"], "stop");
    }

    #[test]
    fn block_indicator_has_input_and_output_slots() {
        let labels: Vec<&str> = BLOCK_INDICATOR.slots.iter().map(|s| s.label).collect();
        assert_eq!(labels, vec!["input", "output"]);

        let input = &BLOCK_INDICATOR.slots[0];
        assert_eq!(input.kind, SlotKind::Producer);
        assert_eq!(input.required_role, "block-occupancy");

        let output = &BLOCK_INDICATOR.slots[1];
        assert_eq!(output.kind, SlotKind::Consumer);
        assert_eq!(output.required_role, "lamp-indicator");
    }

    #[test]
    fn block_indicator_mapping_is_pass_through() {
        let mapping = BLOCK_INDICATOR.mapping;
        assert_eq!(mapping.len(), 2);
        assert_eq!(mapping[0].producer_state, "occupied");
        assert_eq!(mapping[0].consumer_command, "lit");
        assert_eq!(mapping[1].producer_state, "clear");
        assert_eq!(mapping[1].consumer_command, "unlit");
    }

    #[test]
    fn find_template_resolves_by_id_and_misses_unknown() {
        assert!(find_template("block-indicator").is_some());
        assert!(find_template("nope").is_none());
    }

    #[test]
    fn behavior_template_serialises_to_camel_case_json() {
        let json = serde_json::to_value(&BLOCK_INDICATOR).unwrap();
        assert_eq!(json["templateId"], "block-indicator");
        assert_eq!(json["displayName"], "Block Indicator");
        assert_eq!(json["slots"][0]["label"], "input");
        assert_eq!(json["slots"][0]["kind"], "producer");
        assert_eq!(json["slots"][0]["requiredRole"], "block-occupancy");
        assert_eq!(json["slots"][0]["minChannels"], 1);
        assert_eq!(json["slots"][0]["maxChannels"], 1);
        assert_eq!(json["mapping"][0]["producerState"], "occupied");
        assert_eq!(json["mapping"][0]["consumerCommand"], "lit");
    }

    #[test]
    fn block_indicator_slot_cardinality_is_one_to_one() {
        for slot in BLOCK_INDICATOR.slots {
            assert_eq!(slot.min_channels, 1);
            assert_eq!(slot.max_channels, Some(1));
        }
    }

    #[test]
    fn is_at_max_respects_cap_and_unbounded() {
        let bounded = SlotDefinition {
            label: "x",
            display_label: "x",
            kind: SlotKind::Producer,
            required_role: "r",
            min_channels: 0,
            max_channels: Some(2),
            shared: false,
        };
        assert!(!bounded.is_at_max(0));
        assert!(!bounded.is_at_max(1));
        assert!(bounded.is_at_max(2));
        assert!(bounded.is_at_max(3));

        let unbounded = SlotDefinition {
            label: "y",
            display_label: "y",
            kind: SlotKind::Consumer,
            required_role: "r",
            min_channels: 0,
            max_channels: None,
            shared: false,
        };
        assert!(!unbounded.is_at_max(0));
        assert!(!unbounded.is_at_max(999));
    }

    #[test]
    fn find_slot_returns_slot_by_label_or_none() {
        assert_eq!(BLOCK_INDICATOR.find_slot("input").map(|s| s.label), Some("input"));
        assert_eq!(BLOCK_INDICATOR.find_slot("output").map(|s| s.label), Some("output"));
        assert!(BLOCK_INDICATOR.find_slot("nope").is_none());
    }
}
