//! Contract tests for the bundled Tower-LCC+Q profile.
//!
//! The representative CDI fixture preserves the profile-owned paths and enum
//! maps needed to detect v1.15 firmware, annotate event roles, and project the
//! two RR-CirKits daughterboard connectors without depending on bench hardware.

use std::collections::BTreeMap;

use bowties_core::node_tree::build_node_config_tree;
use bowties_core::profile::{
    annotate_tree, build_connector_profile_with_diagnostics, SharedDaughterboardLibrary,
    StructureProfile,
};
use lcc_rs::cdi::parser::parse_cdi;

const PROFILE_YAML: &str = include_str!(
    "../../app/src-tauri/profiles/RR-CirKits_Inc._Tower-LCC+Q.profile.yaml"
);
const SHARED_DAUGHTERBOARDS_YAML: &str = include_str!(
    "../../app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml"
);
const REPRESENTATIVE_CDI_XML: &str = include_str!("fixtures/cdi/tower-lcc-plusq-representative.xml");

#[test]
fn bundled_tower_lcc_plusq_profile_matches_representative_cdi() {
    let profile: StructureProfile =
        serde_yaml_ng::from_str(PROFILE_YAML).expect("Tower-LCC+Q profile must parse");
    let library: SharedDaughterboardLibrary = serde_yaml_ng::from_str(SHARED_DAUGHTERBOARDS_YAML)
        .expect("RR-CirKits shared daughterboards must parse");
    let cdi = parse_cdi(REPRESENTATIVE_CDI_XML).expect("representative Tower-LCC+Q CDI must parse");

    assert_eq!(profile.node_type.manufacturer, "RR-CirKits, Inc.");
    assert_eq!(profile.node_type.model, "Tower-LCC+Q");

    let mut tree = build_node_config_tree("05.01.01.01.7E.00.00.01", &cdi);
    let annotation = annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);
    assert!(
        annotation.warnings.is_empty(),
        "all Tower-LCC+Q event-role paths must resolve: {:?}",
        annotation.warnings
    );
    assert!(
        annotation.event_roles_applied > 0,
        "the profile must annotate representative event leaves"
    );

    let outcome = build_connector_profile_with_diagnostics(
        "05.01.01.01.7E.00.00.01",
        &profile,
        Some(&library),
        &cdi,
    );
    assert!(
        outcome.warning.is_none(),
        "representative CDI must match the Tower-LCC+Q firmware signature: {:?}",
        outcome.warning
    );

    let connector_profile = outcome
        .profile
        .expect("Tower-LCC+Q must project a connector profile");
    assert_eq!(connector_profile.carrier_key, "rr-cirkits, inc.::tower-lcc+q");
    assert_eq!(connector_profile.slots.len(), 2);

    for (slot_id, first_line) in [("connector-a", 1), ("connector-b", 9)] {
        let expected_first_path = format!("Port I/O/Line#{first_line}");
        let slot = connector_profile
            .slots
            .iter()
            .find(|candidate| candidate.slot_id == slot_id)
            .unwrap_or_else(|| panic!("{slot_id} must be present"));
        assert_eq!(slot.affected_paths.len(), 8);
        assert_eq!(slot.resolved_affected_paths.len(), 8);
        assert_eq!(
            slot.affected_paths.first().map(String::as_str),
            Some(expected_first_path.as_str())
        );
        assert!(
            slot.supported_daughterboard_ids
                .iter()
                .any(|id| id == "BOD-8-SM"),
            "{slot_id} must offer the occupancy-detector daughterboard"
        );

        let bod_constraints = slot
            .supported_daughterboard_constraints
            .iter()
            .find(|entry| entry.daughterboard_id == "BOD-8-SM")
            .expect("BOD-8-SM constraints must be projected");
        assert!(
            bod_constraints.validity_rules.iter().any(|rule| {
                rule.target_path == "Port I/O/Line/Actions/Producers/Upon this action"
            }),
            "Tower-LCC+Q must use the producer-action path from its CDI"
        );
        assert!(
            bod_constraints.validity_rules.iter().any(|rule| {
                rule.target_path == "Port I/O/Line/Commands/Consumers"
            }),
            "Tower-LCC+Q must hide its consumer-command group for detector inputs"
        );
    }
}
