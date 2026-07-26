/// Event-wiring plan: describes the event-ID slots the compiler needs filled.
///
/// This is a pure derivation from the same `CompileInput` that the compiler
/// takes, expressed in channel/role vocabulary (slot labels, role hints) rather
/// than event IDs. The composer consumes a `WiringPlan` alongside the structural
/// field writes to fill in the event-ID slots with actual event IDs and metadata.

use serde::{Deserialize, Serialize};

/// The complete wiring plan for a compiled facility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WiringPlan {
    /// One slot per event-ID field the template needs filled.
    pub slots: Vec<WiringSlot>,
}

/// A single event-ID slot in the wiring plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WiringSlot {
    /// The target CDI location: line index + field kind.
    pub target: ConditionalLineEventSlot,
    /// The source channel: slot label + role hint.
    pub source: SlotRef,
    /// Bowtie identity: rule label + aspect.
    pub bowtie_identity: BowtieIdentity,
}

/// A location in the conditional-line field space for an event-ID slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionalLineEventSlot {
    /// 0-based conditional line index.
    pub line_index: u32,
    /// The field type: V1SetTrueEvent, V1SetFalseEvent, or ActionEventId(slot).
    pub field: super::ConditionalLineField,
}

/// A reference to a source slot on a facility channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotRef {
    /// Human-readable label for this slot (e.g. "block occupancy" or "red on").
    pub slot_label: String,
    /// The role and state this slot represents on the source channel.
    pub role_hint: RoleHint,
}

/// Role and state vocabulary for a wiring source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleHint {
    /// Block occupancy role: block is occupied.
    BlockOccupied,
    /// Block occupancy role: block is clear.
    BlockClear,
    /// Output pin role: specific pin (e.g. "red", "green", "yellow").
    LedPin(String),
}

/// Identity of the bowtie that will occupy this event-ID slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BowtieIdentity {
    /// The compilation rule label (e.g. "Stop", "Approach", "Clear").
    pub rule_label: String,
    /// The aspect this rule produces (e.g. "stop", "approach", "clear").
    pub aspect: String,
}

/// Plan the event-wiring slots a facility template requires, without assigning event IDs.
///
/// This is a pure function: the same `CompileInput` always produces an equal `WiringPlan`.
pub fn plan_facility_wiring(input: &super::CompileInput) -> WiringPlan {
    use crate::behavior_templates;

    let template = match behavior_templates::find_template(&input.template_id) {
        Some(t) if t.compilation_target == behavior_templates::CompilationTarget::Compiled => t,
        _ => return WiringPlan { slots: vec![] },
    };

    // Filter and sort rules: same as compile_facility.
    let mut rules: Vec<&behavior_templates::ConditionActionRule> = template
        .rules
        .iter()
        .filter(|r| !(r.aspect == "approach" && input.downstream.is_none()))
        .collect();
    rules.sort_by_key(|r| r.priority);

    let start = super::used_lines_on_node(&input.target_node_key, &input.existing_allocations);
    let mut slots = Vec::new();

    for (i, rule) in rules.iter().enumerate() {
        let line_index = start + i as u32;

        // V1SetTrueEvent and V1SetFalseEvent for non-approach rules.
        if rule.aspect != "approach" {
            slots.push(WiringSlot {
                target: ConditionalLineEventSlot {
                    line_index,
                    field: super::ConditionalLineField::V1SetTrueEvent,
                },
                source: SlotRef {
                    slot_label: "block occupancy".to_string(),
                    role_hint: RoleHint::BlockOccupied,
                },
                bowtie_identity: BowtieIdentity {
                    rule_label: rule.label.to_string(),
                    aspect: rule.aspect.to_string(),
                },
            });
            slots.push(WiringSlot {
                target: ConditionalLineEventSlot {
                    line_index,
                    field: super::ConditionalLineField::V1SetFalseEvent,
                },
                source: SlotRef {
                    slot_label: "block clear".to_string(),
                    role_hint: RoleHint::BlockClear,
                },
                bowtie_identity: BowtieIdentity {
                    rule_label: rule.label.to_string(),
                    aspect: rule.aspect.to_string(),
                },
            });
        }

        // Action event slots.
        if let Some(pin_actions) = super::find_aspect_pin_actions(rule.aspect) {
            let mut slot_idx = 0u8;

            // Pin action slots.
            for pa in pin_actions.iter() {
                if slot_idx >= super::MAX_ACTIONS_PER_LINE as u8 {
                    break;
                }
                // Map pin index to name. For 2-LED signal: 0 = red, 1 = green.
                let pin_name = match pa.pin_index {
                    0 => "red",
                    1 => "green",
                    n => &format!("pin{}", n),
                };
                let pin_label = match pa.on {
                    true => format!("{} on", pin_name),
                    false => format!("{} off", pin_name),
                };
                slots.push(WiringSlot {
                    target: ConditionalLineEventSlot {
                        line_index,
                        field: super::ConditionalLineField::ActionEventId(slot_idx),
                    },
                    source: SlotRef {
                        slot_label: pin_label,
                        role_hint: RoleHint::LedPin(pin_name.to_string()),
                    },
                    bowtie_identity: BowtieIdentity {
                        rule_label: rule.label.to_string(),
                        aspect: rule.aspect.to_string(),
                    },
                });
                slot_idx += 1;
            }

            // Track circuit action slot.
            if let Some(_tc) = input.tc_output {
                if slot_idx < super::MAX_ACTIONS_PER_LINE as u8 {
                    slots.push(WiringSlot {
                        target: ConditionalLineEventSlot {
                            line_index,
                            field: super::ConditionalLineField::ActionEventId(slot_idx),
                        },
                        source: SlotRef {
                            slot_label: "track circuit".to_string(),
                            role_hint: RoleHint::BlockOccupied, // Placeholder; TC doesn't use event IDs
                        },
                        bowtie_identity: BowtieIdentity {
                            rule_label: rule.label.to_string(),
                            aspect: rule.aspect.to_string(),
                        },
                    });
                }
            }
        }
    }

    WiringPlan { slots }
}
