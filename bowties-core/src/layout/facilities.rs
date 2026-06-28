//! Facility persistence for layout files.
//!
//! A **facility** is a named instance of a behavior template (spec 018):
//! it owns a slot map keyed by template slot label, with each slot
//! holding either a channel ID (bound) or `None` (empty). Facility
//! status (`Incomplete` / `Wired`) is derived from slot fullness — it
//! is never persisted on the facility entity itself.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    /// Slot bindings keyed by template slot label.
    /// `None` means the slot is empty; `Some(channel_id)` means
    /// it is bound to a channel by id.
    pub slot_bindings: BTreeMap<String, Option<String>>,
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
    pub const SCHEMA_VERSION: &'static str = "1.0";
    pub const FILE_NAME: &'static str = "facilities.yaml";

    pub fn new(facilities: Vec<Facility>) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            facilities,
        }
    }
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
pub fn apply_facility_deltas(
    doc: &mut FacilitiesDocument,
    deltas: &[crate::layout::types::LayoutEditDelta],
) {
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
            _ => {}
        }
    }
    if touched && doc.schema_version.is_empty() {
        doc.schema_version = FacilitiesDocument::SCHEMA_VERSION.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_indicator_with_empty_slots(facility_id: &str, name: &str) -> Facility {
        let mut slot_bindings = BTreeMap::new();
        slot_bindings.insert("input".to_string(), None);
        slot_bindings.insert("output".to_string(), None);
        Facility {
            facility_id: facility_id.to_string(),
            template_id: "block-indicator".to_string(),
            name: name.to_string(),
            slot_bindings,
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

        assert_eq!(parsed.schema_version, "1.0");
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
            Some("0c3e5cfa-1d54-4e3e-9c12-7c6c8b5b6f01".to_string()),
        );
        let doc = FacilitiesDocument::new(vec![facility.clone()]);

        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        let parsed: FacilitiesDocument = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(parsed.facilities[0].slot_bindings.get("input"), Some(&Some(
            "0c3e5cfa-1d54-4e3e-9c12-7c6c8b5b6f01".to_string(),
        )));
        assert_eq!(parsed.facilities[0].slot_bindings.get("output"), Some(&None));
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
        );
        assert_eq!(doc.schema_version, "1.0");
        assert_eq!(doc.facilities, vec![f]);
    }

    #[test]
    fn apply_add_facility_is_idempotent_on_existing_id() {
        let f = block_indicator_with_empty_slots("id-1", "Block 5");
        let mut doc = FacilitiesDocument::new(vec![f.clone()]);
        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::AddFacility { facility: f.clone() }],
        );
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
        );
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
        );
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
        );
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
                },
            ],
        );
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
        );
        assert_eq!(doc.facilities.len(), 1);
        assert_eq!(doc.facilities[0].name, "Block 7");

        apply_facility_deltas(
            &mut doc,
            &[LayoutEditDelta::DeleteFacility {
                facility_id: "id-1".into(),
            }],
        );
        assert!(doc.facilities.is_empty());
    }
}
