use std::collections::BTreeMap;

use bowties_core::node_tree::{build_node_config_tree, ConfigNode};
use bowties_core::profile::{annotate_tree, StructureProfile};
use lcc_rs::cdi::parser::parse_cdi;

fn leaves<'a>(children: &'a [ConfigNode], found: &mut Vec<&'a bowties_core::node_tree::LeafNode>) {
    for child in children {
        match child {
            ConfigNode::Group(group) => leaves(&group.children, found),
            ConfigNode::Leaf(leaf) => found.push(leaf),
        }
    }
}

fn mark_firmware_locks_read_only(children: &mut [ConfigNode]) {
    for child in children {
        match child {
            ConfigNode::Group(group) => mark_firmware_locks_read_only(&mut group.children),
            ConfigNode::Leaf(leaf) if leaf.name == "Firmware Lock" => leaf.read_only = true,
            ConfigNode::Leaf(_) => {}
        }
    }
}

#[test]
fn profile_read_only_access_applies_to_replicated_leaves_without_unlocking_existing_state() {
    let cdi = parse_cdi(
        r#"<cdi><segment space="253" origin="0"><name>Rules</name>
            <group replication="2"><name>Mast</name><repname>Mast</repname>
                <eventid><name>Link Address</name></eventid>
                <int size="1"><name>Firmware Lock</name></int>
            </group>
        </segment></cdi>"#,
    )
    .expect("test CDI must parse");
    let profile: StructureProfile = serde_yaml_ng::from_str(
        r#"
schemaVersion: "2.0"
nodeType: { manufacturer: Test, model: Test }
fieldAccess:
  - fieldPath: "Rules/Mast/Link Address"
    readOnly: true
  - fieldPath: "Rules/Missing"
    readOnly: true
"#,
    )
    .expect("profile access declarations must parse");
    let mut tree = build_node_config_tree("test:node", &cdi);
    for segment in &mut tree.segments {
        mark_firmware_locks_read_only(&mut segment.children);
    }

    let report = annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);
    let mut annotated_leaves = Vec::new();
    for segment in &tree.segments {
        leaves(&segment.children, &mut annotated_leaves);
    }

    assert_eq!(annotated_leaves.len(), 4);
    assert!(annotated_leaves.iter().all(|leaf| leaf.read_only));
    assert_eq!(report.field_accesses_applied, 2);
    assert!(
        report.warnings.iter().any(|warning| warning.contains("Rules/Missing")),
        "unresolved field-access declarations must use annotation warnings"
    );

    let profile_without_access: StructureProfile = serde_yaml_ng::from_str(
        r#"
schemaVersion: "2.0"
nodeType: { manufacturer: Test, model: Test }
"#,
    )
    .expect("profiles without field access must remain valid");
    let mut unchanged_tree = build_node_config_tree("test:node", &cdi);
    let before = {
        let mut unchanged_leaves = Vec::new();
        for segment in &unchanged_tree.segments {
            leaves(&segment.children, &mut unchanged_leaves);
        }
        unchanged_leaves.iter().map(|leaf| leaf.read_only).collect::<Vec<_>>()
    };
    let unchanged_report =
        annotate_tree(&mut unchanged_tree, &profile_without_access, &BTreeMap::new(), &cdi);
    let mut unchanged_leaves = Vec::new();
    for segment in &unchanged_tree.segments {
        leaves(&segment.children, &mut unchanged_leaves);
    }
    assert_eq!(
        unchanged_leaves
            .iter()
            .map(|leaf| leaf.read_only)
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(unchanged_report.field_accesses_applied, 0);
}
