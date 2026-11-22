//! Facility persistence for layout files.
//!
//! A **facility** is a named instance of a behavior template (spec 018):
//! it owns a slot map keyed by template slot label, with each slot
//! holding a list of channel ids. Empty `Vec` means the slot is
//! unbound. Multi-element bindings are valid in the wire form (D8
//! forward-compat for ABS aspect-slot repeaters); cardinality is bounded
//! per slot by the template's `max_channels`. Facility status
//! (`Incomplete` / `Wired`) is derived from slot fullness — it is never
//! persisted on the facility entity itself.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::behavior_templates;
use crate::signaling::{
    EntranceEnd, ExitEnd, FacilitySignalSettings, Movements, SelectedBlock, TravelDirection,
};

/// A single facility persisted in `facilities.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Facility {
    /// Stable UUID v4. Globally unique within a layout.
    pub facility_id: String,
    /// Behavior template this facility instantiates
    /// (e.g. `block-indicator`).
    pub template_id: String,
    /// User-assigned display name.
    pub name: String,
    /// Slot bindings keyed by template slot label. Empty `Vec` means
    /// the slot is unbound; one or more entries means it is bound to
    /// the listed channel ids. The cap is enforced per template slot
    /// by `max_channels` (Spec 018 / S4 — D8).
    pub slot_bindings: BTreeMap<String, Vec<String>>,
    /// Compiled Automatic Block Signals intent (Spec 020 / E1a — D2).
    /// Absent for Block Indicator facilities and for documents written
    /// before schema "2.0".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_signals: Option<BlockSignalsRecord>,
}

/// Whole-record intent of an Automatic Block Signals facility. Replaced
/// wholesale by `SetBlockSignalsRecord`; never patched field by field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockSignalsRecord {
    pub direction: TravelDirection,
    pub movements: Movements,
    pub entrance_end: EntranceEnd,
    pub exit_end: ExitEnd,
    /// Blocks in travel order.
    pub blocks: Vec<SelectedBlock>,
    pub settings: FacilitySignalSettings,
    pub signals: Vec<SignalRecord>,
    /// Mast / Logic slots this facility's generated configuration occupies.
    pub claimed_slots: Vec<ClaimedSlot>,
}

/// One signal of the facility and the channel that represents it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalRecord {
    pub signal_id: String,
    pub name: String,
    pub protected_block: String,
    pub channel_id: String,
}

/// Kind of node-owned slot a facility claims.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClaimedSlotKind {
    Mast,
    Logic,
}

/// A node slot (mast or Logic) claimed by a facility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaimedSlot {
    pub node_key: String,
    pub kind: ClaimedSlotKind,
    /// 1-based slot number on the node.
    pub index: u32,
}

/// Root structure for `facilities.yaml` persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FacilitiesDocument {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub facilities: Vec<Facility>,
}

impl FacilitiesDocument {
    pub const SCHEMA_VERSION: &'static str = "2.0";
    pub const FILE_NAME: &'static str = "facilities.yaml";

    pub fn new(facilities: Vec<Facility>) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            facilities,
        }
    }
}

/// Errors `apply_facility_deltas` may return per delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FacilityApplyError {
    /// Referenced facility id is not in the document.
    UnknownFacility { facility_id: String },
    /// Referenced slot label is not in the facility's bindings map.
    UnknownSlot {
        facility_id: String,
        slot_label: String,
    },
    /// Attach would exceed the slot's `max_channels` cap.
    SlotAtMax {
        facility_id: String,
        slot_label: String,
        max_channels: u32,
    },
    /// An Automatic Block Signals facility's `blockSignals` record names
    /// signal channels that differ from its `signals` slot set.
    BlockSignalsChannelMismatch {
        facility_id: String,
        /// In the record but not bound to the slot.
        missing_from_slot: Vec<String>,
        /// Bound to the slot but not in the record.
        missing_from_record: Vec<String>,
    },
    /// The facility's template was not found in the registry.
    UnknownTemplate {
        facility_id: String,
        template_id: String,
    },
}

impl std::fmt::Display for FacilityApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFacility { facility_id } => {
                write!(f, "unknown facility id: {}", facility_id)
            }
            Self::UnknownSlot {
                facility_id,
                slot_label,
            } => write!(
                f,
                "unknown slot '{}' on facility {}",
                slot_label, facility_id
            ),
            Self::SlotAtMax {
                facility_id,
                slot_label,
                max_channels,
            } => write!(
                f,
                "slot '{}' on facility {} is already at max ({} channels)",
                slot_label, facility_id, max_channels
            ),
            Self::BlockSignalsChannelMismatch {
                facility_id,
                missing_from_slot,
                missing_from_record,
            } => write!(
                f,
                "facility {} blockSignals record and 'signals' slot disagree (record channels not in slot: {:?}; slot channels not in record: {:?})",
                facility_id, missing_from_slot, missing_from_record
            ),
            Self::UnknownTemplate {
                facility_id,
                template_id,
            } => write!(
                f,
                "facility {} references unknown template {}",
                facility_id, template_id
            ),
        }
    }
}

impl std::error::Error for FacilityApplyError {}

/// Normalize `facilities.yaml` referential integrity against the loaded
/// `channels.yaml`. Drops any slot-binding channel id that is not present
/// in `channels.channels`. Returns one warning string per removed
/// reference so the caller can surface the cleanup to the user.
///
/// This runs at read time so every backend consumer of the effective
/// layout view (`compose_facility_bowties`, catalog rebuild, sync) sees
/// a referentially-consistent shape. Without this normalization the
/// composer's `UnknownChannel` error, the cardinality guard's
/// slot-binding count, and the "Used by" render path each treat a
/// dangling reference differently — a seam-symmetry violation observed
/// in practice after the split-write channel save bug left orphan slot
/// bindings behind.
///
/// The normalization only touches in-memory state. The cleaned facilities
/// document reaches disk the next time the user saves — the read path
/// deliberately does not write back so a read is side-effect free with
/// respect to the layout directory.
pub fn normalize_facility_channel_refs(
    facilities: &mut FacilitiesDocument,
    channels: &crate::layout::channels::ChannelsDocument,
) -> Vec<String> {
    let known_ids: std::collections::HashSet<&str> =
        channels.channels.iter().map(|c| c.id.as_str()).collect();
    let mut warnings = Vec::<String>::new();
    for facility in facilities.facilities.iter_mut() {
        for (slot_label, bindings) in facility.slot_bindings.iter_mut() {
            bindings.retain(|channel_id| {
                if known_ids.contains(channel_id.as_str()) {
                    true
                } else {
                    warnings.push(format!(
                        "Facility '{}' slot '{}': removed reference to unknown channel '{}'",
                        facility.name, slot_label, channel_id,
                    ));
                    false
                }
            });
        }
    }
    warnings
}

/// A facility with a `blockSignals` record must bind exactly the record's
/// signal channel ids to its `signals` slot (Spec 020 / E1a - D1). A
/// mismatch is an explicit error, never a silent drop.
pub fn check_block_signals_agreement(facility: &Facility) -> Result<(), FacilityApplyError> {
    let Some(record) = &facility.block_signals else {
        return Ok(());
    };
    let in_record: std::collections::BTreeSet<&String> =
        record.signals.iter().map(|s| &s.channel_id).collect();
    let in_slot: std::collections::BTreeSet<&String> = facility
        .slot_bindings
        .get("signals")
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    if in_record == in_slot {
        return Ok(());
    }
    Err(FacilityApplyError::BlockSignalsChannelMismatch {
        facility_id: facility.facility_id.clone(),
        missing_from_slot: in_record.difference(&in_slot).map(|s| s.to_string()).collect(),
        missing_from_record: in_slot.difference(&in_record).map(|s| s.to_string()).collect(),
    })
}

/// Apply the facility-relevant variants of [`LayoutEditDelta`] to a
/// `FacilitiesDocument`, mutating it in place.
///
/// Sibling of [`crate::layout::types::apply_layout_deltas`]: that function
/// handles bowtie / node-mode / event-id deltas against a `LayoutFile`; this
/// one handles facility deltas against `facilities.yaml`. Both are called
/// from `save_layout_directory` against the same delta list.
///
/// Non-facility deltas are skipped silently. Always ensures `schema_version`
/// is set to the current schema after applying any change, so a previously
/// empty / pre-018 document is upgraded on first write.
///
/// Cardinality enforcement (Spec 018 / S4 — D8):
///   * `AttachChannelToSlot` rejects if the slot is already at the
///     template's `max_channels` cap, or if the facility / slot is unknown.
///     Idempotent on re-attach of an already-present channel id.
///   * `DetachChannelFromSlot` removes the channel id from the slot's
///     `Vec` if present; idempotent when the channel id is absent. Returns
///     an error for an unknown facility / slot.
pub fn apply_facility_deltas(
    doc: &mut FacilitiesDocument,
    deltas: &[crate::layout::types::LayoutEditDelta],
) -> Result<(), FacilityApplyError> {
    use crate::layout::types::LayoutEditDelta;

    let mut touched = false;
    for delta in deltas {
        match delta {
            LayoutEditDelta::AddFacility { facility } => {
                touched = true;
                if !doc
                    .facilities
                    .iter()
                    .any(|f| f.facility_id == facility.facility_id)
                {
                    doc.facilities.push(facility.clone());
                }
            }
            LayoutEditDelta::RenameFacility {
                facility_id,
                new_name,
            } => {
                if let Some(f) = doc
                    .facilities
                    .iter_mut()
                    .find(|f| &f.facility_id == facility_id)
                {
                    touched = true;
                    f.name = new_name.clone();
                }
            }
            LayoutEditDelta::DeleteFacility { facility_id } => {
                let before = doc.facilities.len();
                doc.facilities.retain(|f| &f.facility_id != facility_id);
                if doc.facilities.len() != before {
                    touched = true;
                }
            }
            LayoutEditDelta::AttachChannelToSlot {
                facility_id,
                slot_label,
                channel_id,
            } => {
                let facility = doc
                    .facilities
                    .iter_mut()
                    .find(|f| &f.facility_id == facility_id)
                    .ok_or_else(|| FacilityApplyError::UnknownFacility {
                        facility_id: facility_id.clone(),
                    })?;
                let template = behavior_templates::find_template(&facility.template_id)
                    .ok_or_else(|| FacilityApplyError::UnknownTemplate {
                        facility_id: facility_id.clone(),
                        template_id: facility.template_id.clone(),
                    })?;
                let slot_def = template.find_slot(slot_label).ok_or_else(|| {
                    FacilityApplyError::UnknownSlot {
                        facility_id: facility_id.clone(),
                        slot_label: slot_label.clone(),
                    }
                })?;
                let bindings = facility
                    .slot_bindings
                    .entry(slot_label.clone())
                    .or_default();
                if bindings.iter().any(|id| id == channel_id) {
                    // Already present — idempotent.
                    touched = true;
                    continue;
                }
                if slot_def.is_at_max(bindings.len()) {
                    return Err(FacilityApplyError::SlotAtMax {
                        facility_id: facility_id.clone(),
                        slot_label: slot_label.clone(),
                        max_channels: slot_def.max_channels.unwrap_or(0),
                    });
                }
                bindings.push(channel_id.clone());
                touched = true;
            }
            LayoutEditDelta::DetachChannelFromSlot {
                facility_id,
                slot_label,
                channel_id,
            } => {
                let facility = doc
                    .facilities
                    .iter_mut()
                    .find(|f| &f.facility_id == facility_id)
                    .ok_or_else(|| FacilityApplyError::UnknownFacility {
                        facility_id: facility_id.clone(),
                    })?;
                let bindings = facility.slot_bindings.get_mut(slot_label).ok_or_else(|| {
                    FacilityApplyError::UnknownSlot {
                        facility_id: facility_id.clone(),
                        slot_label: slot_label.clone(),
                    }
                })?;
                let before = bindings.len();
                bindings.retain(|id| id != channel_id);
                if bindings.len() != before {
                    touched = true;
                }
            }
            LayoutEditDelta::SetBlockSignalsRecord { facility_id, record } => {
                let facility = doc
                    .facilities
                    .iter_mut()
                    .find(|f| &f.facility_id == facility_id)
                    .ok_or_else(|| FacilityApplyError::UnknownFacility {
                        facility_id: facility_id.clone(),
                    })?;
                facility.block_signals = Some(record.clone());
                touched = true;
            }
            _ => {}
        }
    }
    for facility in &doc.facilities {
        check_block_signals_agreement(facility)?;
    }
    if touched {
        doc.schema_version = FacilitiesDocument::SCHEMA_VERSION.to_string();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_indicator_with_empty_slots(facility_id: &str, name: &str) -> Facility {
        let mut slot_bindings = BTreeMap::new();
        slot_bindings.insert("input".to_string(), Vec::<String>::new());
        slot_bindings.insert("output".to_string(), Vec::<String>::new());
        Facility {
            facility_id: facility_id.to_string(),
            template_id: "block-indicator".to_string(),
            name: name.to_string(),
            slot_bindings,
            block_signals: None,
        }
    }

    #[test]
    fn facility_round_trips_yaml_with_empty_slots() {
        let facility = block_indicator_with_empty_slots(
            "5e8d4b22-3f10-4a4b-bf30-9d1c2e6f3a45",
            "Block 5",
        );
        let doc = FacilitiesDocument::new(vec![facility.clone()]);

        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        let parsed: FacilitiesDocument = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(parsed.schema_version, "2.0");
        assert_eq!(parsed.facilities.len(), 1);
        assert_eq!(parsed.facilities[0], facility);
    }

    #[test]
    fn facility_round_trips_yaml_with_a_bound_slot() {
        let mut facility = block_indicator_with_empty_slots(
            "5e8d4b22-3f10-4a4b-bf30-9d1c2e6f3a45",
            "Block 5",
        );
        facility.slot_bindings.insert(
            "input".to_string(),
            vec!["0c3e5cfa-1d54-4e3e-9c12-7c6c8b5b6f01".to_string()],
        );
        let doc = FacilitiesDocument::new(vec![facility.clone()]);

        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        let parsed: FacilitiesDocument = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(
            parsed.facilities[0].slot_bindings.get("input"),
            Some(&vec!["0c3e5cfa-1d54-4e3e-9c12-7c6c8b5b6f01".to_string()])
        );
        assert_eq!(
            parsed.facilities[0].slot_bindings.get("output"),
            Some(&Vec::<String>::new())
        );
    }

    #[test]
    fn facility_round_trips_yaml_with_multi_element_binding() {
        // D8 forward-compat: multi-element bindings parse and re-serialise.
        let mut facility =
            block_indicator_with_empty_slots("id-multi", "Multi");
        facility.slot_bindings.insert(
            "input".to_string(),
            vec!["ch-1".to_string(), "ch-2".to_string()],
        );
        let doc = FacilitiesDocument::new(vec![facility.clone()]);
        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        let parsed: FacilitiesDocument = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed.facilities[0].slot_bindings.get("input"), Some(&vec!["ch-1".to_string(), "ch-2".to_string()]));
    }

    #[test]
    fn empty_document_deserialises_from_empty_yaml() {
        let yaml = "schemaVersion: \"1.0\"\nfacilities: []\n";
        let doc: FacilitiesDocument = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(doc.schema_version, "1.0");
        assert!(doc.facilities.is_empty());
    }

    #[test]
    fn default_document_is_empty_and_has_no_schema_version() {
        let doc = FacilitiesDocument::default();
        assert_eq!(doc.schema_version, "");
        assert!(doc.facilities.is_empty());
    }

    #[test]
    fn yaml_keys_are_camel_case_on_the_wire() {
        let facility = block_indicator_with_empty_slots(
            "5e8d4b22-3f10-4a4b-bf30-9d1c2e6f3a45",
            "Block 5",
        );
        let doc = FacilitiesDocument::new(vec![facility]);
        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        assert!(yaml.contains("schemaVersion:"));
        assert!(yaml.contains("facilityId:"));
        assert!(yaml.contains("templateId:"));
        assert!(yaml.contains("slotBindings:"));
    }

    // ── apply_facility_deltas ────────────────────────────────────────────

    use crate::layout::types::LayoutEditDelta;

    #[test]
    fn apply_add_facility_appends_to_empty_doc_and_upgrades_schema_version() {
        let mut doc = FacilitiesDocument::default();
        assert_eq!(doc.schema_version, "");
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AddFacility { facility: f.clone() }],
        )
        .unwrap();
        assert_eq!(doc.schema_version, "2.0");
        assert_eq!(doc.facilities, vec![f]);
    }

    #[test]
    fn apply_add_facility_upgrades_a_loaded_v1_document_to_current_schema_version() {
        let mut doc: FacilitiesDocument =
            serde_yaml_ng::from_str("schemaVersion: \"1.0\"
facilities: []
").unwrap();
        assert_eq!(doc.schema_version, "1.0");
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        apply_facility_deltas(&mut doc, &[LayoutEditDelta::AddFacility { facility: f }]).unwrap();
        assert_eq!(doc.schema_version, "2.0");
    }

    #[test]
    fn apply_add_facility_is_idempotent_on_existing_id() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f.clone()]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AddFacility { facility: f.clone() }],
        )
        .unwrap();
        assert_eq!(doc.facilities.len(), 1);
    }

    #[test]
    fn apply_rename_updates_matching_facility_name() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::RenameFacility {
                facility_id: "id-1".into(),
                new_name: "Block 7".into(),
            }],
        )
        .unwrap();
        assert_eq!(doc.facilities[0].name, "Block 7");
    }

    #[test]
    fn apply_rename_is_noop_for_unknown_id() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f.clone()]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::RenameFacility {
                facility_id: "nope".into(),
                new_name: "x".into(),
            }],
        )
        .unwrap();
        assert_eq!(doc.facilities, vec![f]);
    }

    #[test]
    fn apply_delete_removes_matching_facility() {
        let f1 = block_indicator_with_empty_slots("id-1", "Block 5");
        let f2 = block_indicator_with_empty_slots("id-2", "Block 6");
        let mut doc = FacilitiesDocument::new(vec![f1.clone(), f2.clone()]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::DeleteFacility {
                facility_id: "id-1".into(),
            }],
        )
        .unwrap();
        assert_eq!(doc.facilities, vec![f2]);
    }

    #[test]
    fn apply_skips_unrelated_deltas() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f.clone()]);
        apply_facility_deltas(
            &mut doc,
            &[
                LayoutEditDelta::AddNode {
                    node_key: "050101010000".into(),
                },
                LayoutEditDelta::CreateBowtie {
                    event_id_hex: "01.00.00.00.00.00.00.01".into(),
                    name: None,
                created_by_facility: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(doc.facilities, vec![f]);
    }

    #[test]
    fn apply_sequence_add_then_rename_then_delete_lands_each_change() {
        let mut doc = FacilitiesDocument::default();
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        apply_facility_deltas(
            &mut doc,
            &[
                LayoutEditDelta::AddFacility { facility: f.clone() },
                LayoutEditDelta::RenameFacility {
                    facility_id: "id-1".into(),
                    new_name: "Block 7".into(),
                },
            ],
        )
        .unwrap();
        assert_eq!(doc.facilities.len(), 1);
        assert_eq!(doc.facilities[0].name, "Block 7");

        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::DeleteFacility {
                facility_id: "id-1".into(),
            }],
        )
        .unwrap();
        assert!(doc.facilities.is_empty());
    }

    // ── attach / detach (S4 — D8 cardinality) ────────────────────────────

    #[test]
    fn apply_attach_appends_channel_to_empty_slot() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AttachChannelToSlot {
                facility_id: "id-1".into(),
                slot_label: "input".into(),
                channel_id: "ch-1".into(),
            }],
        )
        .unwrap();
        assert_eq!(
            doc.facilities[0].slot_bindings.get("input"),
            Some(&vec!["ch-1".to_string()])
        );
    }

    #[test]
    fn apply_attach_at_max_is_rejected() {
        let mut f = block_indicator_with_empty_slots("id-1", "Block 5");
        f.slot_bindings
            .insert("input".to_string(), vec!["ch-1".to_string()]);
        let mut doc = FacilitiesDocument::new(vec![f]);
        let err = apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AttachChannelToSlot {
                facility_id: "id-1".into(),
                slot_label: "input".into(),
                channel_id: "ch-2".into(),
            }],
        )
        .unwrap_err();
        match err {
            FacilityApplyError::SlotAtMax {
                facility_id,
                slot_label,
                max_channels,
            } => {
                assert_eq!(facility_id, "id-1");
                assert_eq!(slot_label, "input");
                assert_eq!(max_channels, 1);
            }
            other => panic!("expected SlotAtMax, got {:?}", other),
        }
        // Document untouched.
        assert_eq!(
            doc.facilities[0].slot_bindings.get("input"),
            Some(&vec!["ch-1".to_string()])
        );
    }

    #[test]
    fn apply_attach_same_channel_twice_is_idempotent() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f]);
        let deltas = [
            LayoutEditDelta::AttachChannelToSlot {
                facility_id: "id-1".into(),
                slot_label: "input".into(),
                channel_id: "ch-1".into(),
            },
            LayoutEditDelta::AttachChannelToSlot {
                facility_id: "id-1".into(),
                slot_label: "input".into(),
                channel_id: "ch-1".into(),
            },
        ];
        apply_facility_deltas(&mut doc, &deltas).unwrap();
        assert_eq!(
            doc.facilities[0].slot_bindings.get("input"),
            Some(&vec!["ch-1".to_string()])
        );
    }

    #[test]
    fn apply_detach_removes_existing_channel() {
        let mut f = block_indicator_with_empty_slots("id-1", "Block 5");
        f.slot_bindings
            .insert("input".to_string(), vec!["ch-1".to_string()]);
        let mut doc = FacilitiesDocument::new(vec![f]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::DetachChannelFromSlot {
                facility_id: "id-1".into(),
                slot_label: "input".into(),
                channel_id: "ch-1".into(),
            }],
        )
        .unwrap();
        assert_eq!(
            doc.facilities[0].slot_bindings.get("input"),
            Some(&Vec::<String>::new())
        );
    }

    #[test]
    fn apply_detach_absent_channel_is_idempotent() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f.clone()]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::DetachChannelFromSlot {
                facility_id: "id-1".into(),
                slot_label: "input".into(),
                channel_id: "ch-1".into(),
            }],
        )
        .unwrap();
        assert_eq!(doc.facilities, vec![f]);
    }

    #[test]
    fn apply_attach_to_unknown_facility_is_an_error() {
        let mut doc = FacilitiesDocument::default();
        let err = apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AttachChannelToSlot {
                facility_id: "nope".into(),
                slot_label: "input".into(),
                channel_id: "ch-1".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(err, FacilityApplyError::UnknownFacility { .. }));
    }

    #[test]
    fn apply_attach_to_unknown_slot_is_an_error() {
        // The slot is unknown because the template doesn't define it,
        // even though the bindings map happens to seed it via or_default.
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f]);
        let err = apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AttachChannelToSlot {
                facility_id: "id-1".into(),
                slot_label: "nope".into(),
                channel_id: "ch-1".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(err, FacilityApplyError::UnknownSlot { .. }));
    }

    #[test]
    fn apply_detach_from_unknown_facility_is_an_error() {
        let mut doc = FacilitiesDocument::default();
        let err = apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::DetachChannelFromSlot {
                facility_id: "nope".into(),
                slot_label: "input".into(),
                channel_id: "ch-1".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(err, FacilityApplyError::UnknownFacility { .. }));
    }

    // ── normalize_facility_channel_refs (referential integrity on read) ────

    fn channel_stub(id: &str) -> crate::layout::channels::InformationChannel {
        use crate::layout::channels::{
            ChannelBinding, ChannelOwnership, ChannelRole, InformationChannel,
        };
        InformationChannel {
            id: id.to_string(),
            name: id.to_string(),
            role: ChannelRole::BlockOccupancy,
            style: "bod-block-detector-input".to_string(),
            ownership: ChannelOwnership::HardwareOwned,
            binding: ChannelBinding::ConnectorInput {
                node_key: "05010101FF000001".to_string(),
                connector: "connector-a".to_string(),
                input: 1,
            },
        }
    }

    #[test]
    fn normalize_leaves_valid_bindings_untouched_and_returns_no_warnings() {
        use crate::layout::channels::ChannelsDocument;

        let mut facility = block_indicator_with_empty_slots("f-1", "Block 5");
        facility
            .slot_bindings
            .insert("input".into(), vec!["ch-a".into()]);
        let mut facilities = FacilitiesDocument::new(vec![facility]);
        let channels = ChannelsDocument::new(vec![channel_stub("ch-a")]);

        let warnings = normalize_facility_channel_refs(&mut facilities, &channels);

        assert!(warnings.is_empty());
        assert_eq!(
            facilities.facilities[0].slot_bindings["input"],
            vec!["ch-a".to_string()]
        );
    }

    #[test]
    fn normalize_drops_dangling_channel_ids_and_reports_warnings() {
        use crate::layout::channels::ChannelsDocument;

        let mut facility = block_indicator_with_empty_slots("f-1", "Block 5");
        facility
            .slot_bindings
            .insert("input".into(), vec!["ch-a".into(), "ch-ghost".into()]);
        facility
            .slot_bindings
            .insert("output".into(), vec!["ch-orphan".into()]);
        let mut facilities = FacilitiesDocument::new(vec![facility]);
        let channels = ChannelsDocument::new(vec![channel_stub("ch-a")]);

        let warnings = normalize_facility_channel_refs(&mut facilities, &channels);

        assert_eq!(
            facilities.facilities[0].slot_bindings["input"],
            vec!["ch-a".to_string()]
        );
        assert!(facilities.facilities[0].slot_bindings["output"].is_empty());
        assert_eq!(warnings.len(), 2);
        for warning in &warnings {
            assert!(warning.contains("Block 5"), "warning names the facility: {}", warning);
            assert!(
                warning.contains("ch-ghost") || warning.contains("ch-orphan"),
                "warning names the missing channel id: {}",
                warning,
            );
        }
    }

    #[test]
    fn normalize_leaves_empty_slot_bindings_alone() {
        use crate::layout::channels::ChannelsDocument;

        let facility = block_indicator_with_empty_slots("f-1", "Block 5");
        let mut facilities = FacilitiesDocument::new(vec![facility]);
        let channels = ChannelsDocument::default();

        let warnings = normalize_facility_channel_refs(&mut facilities, &channels);

        assert!(warnings.is_empty());
        assert!(facilities.facilities[0].slot_bindings["input"].is_empty());
        assert!(facilities.facilities[0].slot_bindings["output"].is_empty());
    }

    #[test]
    fn normalize_handles_multiple_facilities_independently() {
        use crate::layout::channels::ChannelsDocument;

        let mut a = block_indicator_with_empty_slots("f-a", "A");
        a.slot_bindings.insert("input".into(), vec!["ch-a".into()]);
        let mut b = block_indicator_with_empty_slots("f-b", "B");
        b.slot_bindings
            .insert("output".into(), vec!["ch-missing".into()]);
        let mut facilities = FacilitiesDocument::new(vec![a, b]);
        let channels = ChannelsDocument::new(vec![channel_stub("ch-a")]);

        let warnings = normalize_facility_channel_refs(&mut facilities, &channels);

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("B"));
        assert_eq!(
            facilities.facilities[0].slot_bindings["input"],
            vec!["ch-a".to_string()]
        );
        assert!(facilities.facilities[1].slot_bindings["output"].is_empty());
    }

    // -- blockSignals record (Spec 020 / E1a) --------------------------------

    fn sample_record(channel_ids: &[&str]) -> BlockSignalsRecord {
        use crate::signaling::{FacilitySignalSettings, SelectedBlock, TravelDirection};
        BlockSignalsRecord {
            direction: TravelDirection::Westward,
            movements: crate::signaling::Movements::OneDirection,
            entrance_end: crate::signaling::EntranceEnd::NoPlant,
            exit_end: crate::signaling::ExitEnd::NothingBeyond,
            blocks: vec![SelectedBlock {
                block_id: "blk-1".to_string(),
                name: "Main 1".to_string(),
                joined_to_previous: false,
                turnouts: vec![],
            }],
            settings: FacilitySignalSettings::default(),
            signals: channel_ids
                .iter()
                .enumerate()
                .map(|(i, ch)| SignalRecord {
                    signal_id: format!("sig-{i}"),
                    name: format!("Signal {i}"),
                    protected_block: "blk-1".to_string(),
                    channel_id: ch.to_string(),
                })
                .collect(),
            claimed_slots: vec![ClaimedSlot {
                node_key: "050101010700".to_string(),
                kind: ClaimedSlotKind::Mast,
                index: 3,
            }],
        }
    }

    fn abs_facility(id: &str, record: Option<BlockSignalsRecord>) -> Facility {
        Facility {
            facility_id: id.to_string(),
            template_id: "automatic-block-signals".to_string(),
            name: "ABS".to_string(),
            slot_bindings: BTreeMap::from([(
                "signals".to_string(),
                record
                    .as_ref()
                    .map(|r| r.signals.iter().map(|s| s.channel_id.clone()).collect())
                    .unwrap_or_default(),
            )]),
            block_signals: record,
        }
    }

    #[test]
    fn block_signals_record_round_trips_yaml() {
        let f = abs_facility("abs-1", Some(sample_record(&["ch-a"])));
        let doc = FacilitiesDocument::new(vec![f.clone()]);
        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        assert!(yaml.contains("blockSignals:"), "{yaml}");
        assert!(yaml.contains("claimedSlots:"), "{yaml}");
        assert!(yaml.contains("protectedBlock:"), "{yaml}");
        let parsed: FacilitiesDocument = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed.facilities[0], f);
    }

    #[test]
    fn v1_document_without_block_signals_loads() {
        let yaml = "schemaVersion: \"1.0\"
facilities:
  - facilityId: f1
    templateId: block-indicator
    name: B
    slotBindings: {}
";
        let doc: FacilitiesDocument = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(doc.facilities[0].block_signals, None);
    }

    #[test]
    fn block_indicator_yaml_has_no_block_signals_key() {
        let doc = FacilitiesDocument::new(vec![block_indicator_with_empty_slots("id-1", "B")]);
        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        assert!(!yaml.contains("blockSignals"), "{yaml}");
    }

    #[test]
    fn set_block_signals_record_replaces_added_record_wholesale() {
        let mut doc = FacilitiesDocument::default();
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AddFacility {
                facility: abs_facility("abs-1", Some(sample_record(&["ch-a", "ch-b"]))),
            }],
        )
        .unwrap();
        assert_eq!(doc.facilities[0].block_signals, Some(sample_record(&["ch-a", "ch-b"])));
        let replacement = sample_record(&["ch-z"]);
        let slot = |ch: &str, attach: bool| {
            let (facility_id, slot_label, channel_id) =
                ("abs-1".to_string(), "signals".to_string(), ch.to_string());
            if attach {
                LayoutEditDelta::AttachChannelToSlot { facility_id, slot_label, channel_id }
            } else {
                LayoutEditDelta::DetachChannelFromSlot { facility_id, slot_label, channel_id }
            }
        };
        apply_facility_deltas(
            &mut doc,
            &[
                LayoutEditDelta::SetBlockSignalsRecord {
                    facility_id: "abs-1".to_string(),
                    record: replacement.clone(),
                },
                slot("ch-a", false),
                slot("ch-b", false),
                slot("ch-z", true),
            ],
        )
        .unwrap();
        assert_eq!(doc.facilities[0].block_signals, Some(replacement));
    }

    #[test]
    fn set_block_signals_record_on_unknown_facility_is_an_error() {
        let mut doc = FacilitiesDocument::default();
        let err = apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::SetBlockSignalsRecord {
                facility_id: "nope".to_string(),
                record: sample_record(&[]),
            }],
        )
        .unwrap_err();
        assert_eq!(err, FacilityApplyError::UnknownFacility { facility_id: "nope".to_string() });
    }

    // -- automatic-block-signals template + signals slot (Spec 020 / E1a) ----

    fn attach(facility_id: &str, ch: &str) -> LayoutEditDelta {
        LayoutEditDelta::AttachChannelToSlot {
            facility_id: facility_id.to_string(),
            slot_label: "signals".to_string(),
            channel_id: ch.to_string(),
        }
    }

    #[test]
    fn signals_slot_accepts_multiple_channels() {
        let mut doc = FacilitiesDocument::new(vec![abs_facility("abs-1", None)]);
        apply_facility_deltas(&mut doc, &[attach("abs-1", "ch-a"), attach("abs-1", "ch-b")])
            .unwrap();
        assert_eq!(
            doc.facilities[0].slot_bindings.get("signals"),
            Some(&vec!["ch-a".to_string(), "ch-b".to_string()])
        );
    }

    #[test]
    fn record_signal_channels_must_equal_signals_slot_set() {
        let mut doc = FacilitiesDocument::new(vec![abs_facility("abs-1", None)]);
        let err = apply_facility_deltas(
            &mut doc,
            &[
                LayoutEditDelta::SetBlockSignalsRecord {
                    facility_id: "abs-1".to_string(),
                    record: sample_record(&["ch-a", "ch-b"]),
                },
                attach("abs-1", "ch-a"),
            ],
        )
        .unwrap_err();
        assert!(
            matches!(&err, FacilityApplyError::BlockSignalsChannelMismatch { facility_id, .. } if facility_id == "abs-1"),
            "{err:?}"
        );
        assert!(err.to_string().contains("ch-b"), "{err}");
    }

    #[test]
    fn matching_record_and_slot_set_is_accepted_in_any_order() {
        let mut doc = FacilitiesDocument::new(vec![abs_facility("abs-1", None)]);
        apply_facility_deltas(
            &mut doc,
            &[
                attach("abs-1", "ch-b"),
                attach("abs-1", "ch-a"),
                LayoutEditDelta::SetBlockSignalsRecord {
                    facility_id: "abs-1".to_string(),
                    record: sample_record(&["ch-a", "ch-b"]),
                },
            ],
        )
        .unwrap();
    }
}
