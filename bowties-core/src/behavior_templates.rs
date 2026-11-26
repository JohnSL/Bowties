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

const AUTOMATIC_BLOCK_SIGNALS_SLOTS: &[SlotDefinition] = &[SlotDefinition {
    label: "signals",
    display_label: "signals",
    kind: SlotKind::Consumer,
    required_role: "signal-indication",
    min_channels: 1,
    max_channels: None,
}];

/// Automatic Block Signals (Spec 020 / E1a - D1). One unbounded `signals`
/// slot and no state mapping: its configuration is compiled by the signaling
/// compiler, so bowtie composition never applies.
pub const AUTOMATIC_BLOCK_SIGNALS: BehaviorTemplate = BehaviorTemplate {
    template_id: "automatic-block-signals",
    display_name: "Automatic Block Signals",
    slots: AUTOMATIC_BLOCK_SIGNALS_SLOTS,
    mapping: &[],
};

const PANEL_SLOTS: &[SlotDefinition] = &[SlotDefinition {
    label: "lamps",
    display_label: "lamps",
    kind: SlotKind::Consumer,
    required_role: "lamp-indicator",
    min_channels: 0,
    max_channels: None,
}];

/// Panel (Spec 021). One unbounded `lamps` slot (the only lamp membership
/// source) and no state mapping: its configuration is compiled, so bowtie
/// composition never applies.
pub const PANEL: BehaviorTemplate = BehaviorTemplate {
    template_id: "panel",
    display_name: "Panel",
    slots: PANEL_SLOTS,
    mapping: &[],
};

const REGISTRY: &[BehaviorTemplate] = &[AUTOMATIC_BLOCK_SIGNALS, PANEL];

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
    /// True when the template realises its behaviour as bowties (has a
    /// producer-state mapping). Templates without one compose nothing.
    pub fn composes_bowties(&self) -> bool {
        !self.mapping.is_empty()
    }

    /// Look up a slot by label within this template.
    pub fn find_slot(&self, label: &str) -> Option<&'static SlotDefinition> {
        self.slots.iter().find(|s| s.label == label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_retires_block_indicator_and_preserves_abs_and_panel() {
        let templates = registered_templates();
        assert_eq!(templates.len(), 2);
        assert!(find_template("block-indicator").is_none());
        let abs = find_template("automatic-block-signals").expect("ABS registered");
        assert_eq!(abs.display_name, "Automatic Block Signals");
        let signals = abs.find_slot("signals").expect("signals slot");
        assert_eq!(signals.required_role, "signal-indication");
        assert_eq!(signals.max_channels, None);
        assert!(abs.mapping.is_empty());
        assert!(!abs.composes_bowties());
    }

    #[test]
    fn panel_template_has_one_unbounded_lamps_slot_and_composes_nothing() {
        let panel = find_template("panel").expect("panel registered");
        assert_eq!(panel.display_name, "Panel");
        assert_eq!(panel.slots.len(), 1);
        let lamps = panel.find_slot("lamps").expect("lamps slot");
        assert_eq!(lamps.kind, SlotKind::Consumer);
        assert_eq!(lamps.required_role, "lamp-indicator");
        assert_eq!(lamps.min_channels, 0);
        assert_eq!(lamps.max_channels, None);
        assert!(panel.mapping.is_empty());
        assert!(!panel.composes_bowties());
    }

    #[test]
    fn find_template_resolves_by_id_and_misses_unknown() {
        assert!(find_template("panel").is_some());
        assert!(find_template("nope").is_none());
    }

    #[test]
    fn behavior_template_serialises_to_camel_case_json() {
        let json = serde_json::to_value(&PANEL).unwrap();
        assert_eq!(json["templateId"], "panel");
        assert_eq!(json["displayName"], "Panel");
        assert_eq!(json["slots"][0]["label"], "lamps");
        assert_eq!(json["slots"][0]["kind"], "consumer");
        assert_eq!(json["slots"][0]["requiredRole"], "lamp-indicator");
        assert_eq!(json["slots"][0]["minChannels"], 0);
        assert!(json["slots"][0]["maxChannels"].is_null());
        assert_eq!(json["mapping"], serde_json::json!([]));
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
        };
        assert!(!unbounded.is_at_max(0));
        assert!(!unbounded.is_at_max(999));
    }

    #[test]
    fn find_slot_returns_slot_by_label_or_none() {
        assert_eq!(PANEL.find_slot("lamps").map(|s| s.label), Some("lamps"));
        assert!(PANEL.find_slot("nope").is_none());
    }
}
