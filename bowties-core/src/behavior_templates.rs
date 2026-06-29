//! Behavior template registry — declarative composition of facilities.
//!
//! A behavior template defines the slot structure and producer/consumer
//! state mapping for one named facility kind (per spec 018).  Templates
//! are code-level (hardcoded here in this slice); a future loader may
//! deserialize them from YAML without changing the wire form.

use serde::Serialize;

/// Whether a slot accepts a producer channel or drives a consumer channel.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SlotKind {
    Producer,
    Consumer,
}

/// One slot inside a behavior template.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlotDefinition {
    /// Slot label, unique within the template (e.g. `input`, `output`).
    pub label: &'static str,
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
}

const BLOCK_INDICATOR_SLOTS: &[SlotDefinition] = &[
    SlotDefinition {
        label: "input",
        kind: SlotKind::Producer,
        required_role: "block-occupancy",
        min_channels: 1,
        max_channels: Some(1),
    },
    SlotDefinition {
        label: "output",
        kind: SlotKind::Consumer,
        required_role: "lamp-indicator",
        min_channels: 1,
        max_channels: Some(1),
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
};

const REGISTRY: &[BehaviorTemplate] = &[BLOCK_INDICATOR];

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
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].template_id, "block-indicator");
        assert_eq!(templates[0].display_name, "Block Indicator");
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
            kind: SlotKind::Producer,
            required_role: "r",
            min_channels: 0,
            max_channels: Some(2),
        };
        assert!(!bounded.is_at_max(0));
        assert!(!bounded.is_at_max(1));
        assert!(bounded.is_at_max(2));
        assert!(bounded.is_at_max(3));

        let unbounded = SlotDefinition {
            label: "y",
            kind: SlotKind::Consumer,
            required_role: "r",
            min_channels: 0,
            max_channels: None,
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
