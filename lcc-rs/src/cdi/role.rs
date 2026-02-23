//! CDI Event Slot Role Classification
//!
//! Provides `EventRole` and `classify_event_slot()` — a pure, two-tier heuristic
//! used **only** when the Identify Events protocol exchange (Tier 0) is inconclusive.
//!
//! Tier 0 is inconclusive when a node replies BOTH `ProducerIdentified` AND
//! `ConsumerIdentified` for the same event ID (i.e., the node produces *and*
//! consumes the same event).  In that case the element-level role cannot be
//! determined from the protocol reply alone, and this fallback is applied.
//!
//! Tier 1: Check ancestor `<group><name>` strings for producer/consumer keywords.
//! Tier 2: Check the element's `<description>` for producer/consumer phrase patterns.
//! Default: `Ambiguous` when neither tier fires.

use serde::{Deserialize, Serialize};
use super::EventIdElement;

/// Classified role of a single event ID slot in a CDI configuration.
///
/// Producer and Consumer are determined either by the Identify Events protocol
/// (Tier 0, cross-node ground truth) or by the CDI heuristic (Tier 1/2, same-node
/// fallback).  Ambiguous is the fallback when neither mechanism can determine the
/// role; these entries surface in `BowtieCard.ambiguous_entries` for future user
/// clarification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventRole {
    /// This slot generates / sends the event (output to the network).
    Producer,
    /// This slot responds to / receives the event (input from the network).  
    Consumer,
    /// Role could not be determined by any available mechanism.
    Ambiguous,
}

/// Classify a single event ID element given its CDI context.
///
/// Called **only** when Tier 0 (Identify Events protocol) is inconclusive —
/// i.e., the same node replied both `ProducerIdentified` and `ConsumerIdentified`
/// for the event value stored in this slot.
///
/// # Arguments
/// * `element` — The `EventIdElement` (carries `name` and `description`).
/// * `parent_group_names` — Slice of all ancestor group `<name>` strings,
///   outermost-first.  Empty slice is valid (yields `Ambiguous` if Tier 2 also fails).
///
/// # Returns
/// * `Producer` — Tier 1 or Tier 2 fired with a producer signal.
/// * `Consumer` — Tier 1 or Tier 2 fired with a consumer signal.
/// * `Ambiguous` — Neither tier provided a conclusive signal.
pub fn classify_event_slot(
    element: &EventIdElement,
    parent_group_names: &[&str],
) -> EventRole {
    // ── Tier 1: parent group name keywords (case-insensitive substring match) ──
    //
    // Producer keywords (per research.md RQ-3)
    const PRODUCER_GROUP_KEYWORDS: &[&str] = &[
        "producer",
        "producers",
        "input",
        "inputs",
        "generated",
        "output activat",
    ];
    // Consumer keywords
    const CONSUMER_GROUP_KEYWORDS: &[&str] = &[
        "consumer",
        "consumers",
        "output",
        "outputs",
        "responded",
        "activates turnout",
    ];

    for name in parent_group_names {
        let lower = name.to_lowercase();
        for kw in PRODUCER_GROUP_KEYWORDS {
            if lower.contains(kw) {
                return EventRole::Producer;
            }
        }
        for kw in CONSUMER_GROUP_KEYWORDS {
            if lower.contains(kw) {
                return EventRole::Consumer;
            }
        }
    }

    // ── Tier 2: element <description> phrase patterns ──
    //
    // Only applied when Tier 1 was inconclusive.
    const PRODUCER_DESC_PATTERNS: &[&str] = &[
        "generated when",
        "sent when",
        "produced when",
        "trigger when",
    ];
    const CONSUMER_DESC_PATTERNS: &[&str] = &[
        "when this event",
        "activates",
        "causes",
        "responds to",
    ];

    if let Some(desc) = &element.description {
        let lower = desc.to_lowercase();
        for pattern in PRODUCER_DESC_PATTERNS {
            if lower.contains(pattern) {
                return EventRole::Producer;
            }
        }
        for pattern in CONSUMER_DESC_PATTERNS {
            if lower.contains(pattern) {
                return EventRole::Consumer;
            }
        }
    }

    // Neither tier fired — role is indeterminate.
    EventRole::Ambiguous
}

// ============================================================================
// T001: Unit tests for classify_event_slot
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdi::EventIdElement;

    fn event_id(name: Option<&str>, description: Option<&str>) -> EventIdElement {
        EventIdElement {
            name: name.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            offset: 0,
        }
    }

    // ── Tier 1: parent group name keyword matches ──────────────────────────

    #[test]
    fn tier1_producers_group_name_producer() {
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &["Producers"]),
            EventRole::Producer,
            "Parent group 'Producers' should yield Producer"
        );
    }

    #[test]
    fn tier1_consumers_group_name_consumer() {
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &["Consumers"]),
            EventRole::Consumer,
            "Parent group 'Consumers' should yield Consumer"
        );
    }

    #[test]
    fn tier1_inputs_group_name_producer() {
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &["Inputs"]),
            EventRole::Producer,
            "Parent group 'Inputs' should yield Producer"
        );
    }

    #[test]
    fn tier1_outputs_group_name_consumer() {
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &["Outputs"]),
            EventRole::Consumer,
            "Parent group 'Outputs' should yield Consumer"
        );
    }

    #[test]
    fn tier1_case_insensitive_match() {
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &["PRODUCERS"]),
            EventRole::Producer,
            "Keyword match should be case-insensitive"
        );
    }

    #[test]
    fn tier1_substring_match_within_longer_name() {
        // "Generated Events" contains "generated" keyword
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &["Generated Events"]),
            EventRole::Producer,
            "'Generated Events' contains 'generated' → Producer"
        );
    }

    #[test]
    fn tier1_outermost_ancestor_wins() {
        // outermost = "Producers", inner = "Consumers" — first ancestor wins
        let elem = event_id(None, None);
        let result = classify_event_slot(&elem, &["Producers", "Consumers"]);
        // T001 spec says "Producers" parent → Producer. The outermost name is checked
        // first and returns immediately, so Producer is expected here.
        assert_eq!(
            result,
            EventRole::Producer,
            "Outermost ancestor 'Producers' fires first"
        );
    }

    // ── Tier 2: description phrase patterns ──────────────────────────────────

    #[test]
    fn tier2_generated_when_producer() {
        let elem = event_id(None, Some("Generated when input goes active"));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Producer,
            "'generated when' phrase → Producer"
        );
    }

    #[test]
    fn tier2_when_this_event_consumer() {
        let elem = event_id(None, Some("When this event arrives, turnout moves to closed"));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Consumer,
            "'when this event' phrase → Consumer"
        );
    }

    #[test]
    fn tier2_sent_when_producer() {
        let elem = event_id(None, Some("Sent when the occupancy detector fires"));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Producer,
            "'sent when' phrase → Producer"
        );
    }

    #[test]
    fn tier2_activates_consumer() {
        let elem = event_id(None, Some("Activates relay output 1"));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Consumer,
            "'activates' phrase → Consumer"
        );
    }

    #[test]
    fn tier2_responds_to_consumer() {
        let elem = event_id(None, Some("Responds to the occupancy event"));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Consumer,
            "'responds to' phrase → Consumer"
        );
    }

    #[test]
    fn tier2_case_insensitive_description() {
        let elem = event_id(None, Some("GENERATED WHEN signal goes active"));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Producer,
            "Description match should be case-insensitive"
        );
    }

    // ── No-match cases → Ambiguous ────────────────────────────────────────

    #[test]
    fn no_match_returns_ambiguous() {
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Ambiguous,
            "No group names and no description → Ambiguous"
        );
    }

    #[test]
    fn empty_string_description_returns_ambiguous() {
        let elem = event_id(None, Some(""));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Ambiguous,
            "Empty description → Ambiguous"
        );
    }

    #[test]
    fn unrelated_group_name_returns_ambiguous() {
        let elem = event_id(None, None);
        assert_eq!(
            classify_event_slot(&elem, &["Configuration", "Advanced"]),
            EventRole::Ambiguous,
            "Unrelated group names → Ambiguous"
        );
    }

    #[test]
    fn unrelated_description_returns_ambiguous() {
        let elem = event_id(None, Some("Set the node ID for this slot"));
        assert_eq!(
            classify_event_slot(&elem, &[]),
            EventRole::Ambiguous,
            "Description with no matching phrase → Ambiguous"
        );
    }

    // ── Tier 1 takes precedence over Tier 2 ──────────────────────────────

    #[test]
    fn tier1_fires_before_tier2_producer_group_consumer_desc() {
        // Group name says Producer, description says Consumer — Tier 1 should win.
        let elem = event_id(None, Some("Activates output relay"));
        assert_eq!(
            classify_event_slot(&elem, &["Producers"]),
            EventRole::Producer,
            "Tier 1 (Producers group) takes precedence over Tier 2 (Activates → Consumer)"
        );
    }
}
