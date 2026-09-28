//! Contract tests for the bundled Signal-LCC profile.
//!
//! The shipping profile is intentionally the production input under test. Its
//! role declarations are evaluated against a stable, test-owned CDI excerpt so
//! documentation and extraction-file changes cannot alter the fixture.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use bowties_core::node_tree::{build_node_config_tree, ConfigNode, LeafType};
use bowties_core::profile::{
    annotate_tree, resolve_named_path, resolver::strip_instance_steps, StructureProfile,
};
use lcc_rs::cdi::{parser::parse_cdi, EventRole};

fn repo_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

#[test]
fn shipping_signal_lcc_profile_assigns_distinct_rule_event_roles() {
    let cdi_xml = include_str!("fixtures/cdi/signal-lcc-c7c-rule-to-aspect.xml");
    let cdi = parse_cdi(cdi_xml).expect("Signal-LCC test CDI must parse");

    let profile_yaml = fs::read_to_string(repo_path(
        "app/src-tauri/profiles/RR-CirKits_Inc._Signal-LCC.profile.yaml",
    ))
    .expect("bundled Signal-LCC profile must be readable");
    let profile: StructureProfile =
        serde_yaml_ng::from_str(&profile_yaml).expect("bundled Signal-LCC profile must parse");

    let mut unique_targets = HashSet::new();
    for declaration in &profile.event_roles {
        assert!(
            unique_targets.insert(declaration.group_path.as_str()),
            "duplicate shipping event-role target: {}",
            declaration.group_path,
        );
    }

    let expected = [
        ("Rule to Aspect/Mast/Link Address", EventRole::Producer, 8),
        ("Rule to Aspect/Mast/Rule/set aspect", EventRole::Consumer, 64),
        (
            "Rule to Aspect/Mast/Rule/aspect is set",
            EventRole::Producer,
            64,
        ),
        (
            "Rule to Aspect/Mast/Rule/aspect cleared",
            EventRole::Producer,
            64,
        ),
    ];
    let expected_targets: HashSet<&str> = expected.iter().map(|(target, _, _)| *target).collect();
    let actual_targets: HashSet<&str> = profile
        .event_roles
        .iter()
        .map(|declaration| declaration.group_path.as_str())
        .filter(|target| target.starts_with("Rule to Aspect/"))
        .collect();
    assert_eq!(
        actual_targets, expected_targets,
        "unexpected Rule-to-Aspect event-role targets"
    );

    let expected_resolved: HashMap<Vec<String>, (EventRole, usize)> = expected
        .iter()
        .map(|(target, role, count)| {
            let declaration = profile
                .event_roles
                .iter()
                .find(|declaration| declaration.group_path == *target)
                .unwrap_or_else(|| panic!("missing shipping event-role target: {target}"));
            let declared_role: EventRole = declaration.role.into();
            assert_eq!(declared_role, *role, "wrong declared role for {target}");
            (
                resolve_named_path(target, &cdi)
                    .unwrap_or_else(|error| panic!("{target} must resolve: {error}")),
                (*role, *count),
            )
        })
        .collect();

    let mut tree = build_node_config_tree("signal-lcc:test", &cdi);
    annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);

    let mut matched_counts: HashMap<Vec<String>, usize> = HashMap::new();
    fn verify_leaves(
        children: &[ConfigNode],
        expected: &HashMap<Vec<String>, (EventRole, usize)>,
        counts: &mut HashMap<Vec<String>, usize>,
    ) {
        for child in children {
            match child {
                ConfigNode::Group(group) => verify_leaves(&group.children, expected, counts),
                ConfigNode::Leaf(leaf) if leaf.element_type == LeafType::EventId => {
                    let path = strip_instance_steps(&leaf.path);
                    if let Some((role, _)) = expected.get(&path) {
                        assert_eq!(
                            leaf.event_role,
                            Some(*role),
                            "wrong annotated role for {}",
                            leaf.name
                        );
                        *counts.entry(path).or_default() += 1;
                    }
                }
                ConfigNode::Leaf(_) => {}
            }
        }
    }
    for segment in &tree.segments {
        verify_leaves(&segment.children, &expected_resolved, &mut matched_counts);
    }
    for (target, (_, expected_count)) in &expected_resolved {
        assert_eq!(
            matched_counts.get(target),
            Some(expected_count),
            "unexpected annotated leaf count for resolved target {target:?}",
        );
    }
}
