use std::collections::BTreeMap;

use bowties_core::node_tree::{build_node_config_tree, ConfigNode, LeafNode};
use bowties_core::profile::{annotate_tree, StructureProfile};
use lcc_rs::cdi::parser::parse_cdi;

const TEST_CDI: &str = r#"
<cdi>
  <segment space="253">
    <name>Configuration</name>
    <group>
      <name>Controls</name>
      <int size="1">
        <name>Brightness</name>
        <min>0</min>
        <max>255</max>
        <hints><slider immediate="0" tickSpacing="5" showValue="0"/></hints>
      </int>
      <int size="1">
        <name>Unannotated</name>
        <min>0</min>
        <max>255</max>
        <hints><slider immediate="0" tickSpacing="7" showValue="0"/></hints>
      </int>
      <string size="8">
        <name>Label</name>
      </string>
      <group>
        <name>Nested</name>
        <int size="1">
          <name>NestedValue</name>
          <hints><slider immediate="0" tickSpacing="11" showValue="0"/></hints>
        </int>
      </group>
    </group>
    <group>
      <name>Other</name>
      <int size="1">
        <name>Outside</name>
        <hints><slider immediate="0" tickSpacing="13" showValue="0"/></hints>
      </int>
    </group>
    <group>
      <name>Ineligible</name>
      <string size="8">
        <name>TextOnly</name>
      </string>
      <group>
        <name>NestedOnly</name>
        <int size="1">
          <name>TooDeep</name>
        </int>
      </group>
    </group>
  </segment>
</cdi>
"#;

fn find_leaf<'a>(children: &'a [ConfigNode], name: &str) -> Option<&'a LeafNode> {
    for child in children {
        match child {
            ConfigNode::Group(group) => {
                if let Some(leaf) = find_leaf(&group.children, name) {
                    return Some(leaf);
                }
            }
            ConfigNode::Leaf(leaf) if leaf.name == name => return Some(leaf),
            ConfigNode::Leaf(_) => {}
        }
    }
    None
}

#[test]
fn explicit_slider_presentation_overrides_only_the_targeted_integer_leaf() {
    let profile_yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "Test"
  model: "Presentation"
fieldPresentation:
  - fieldPath: "Configuration/Controls/Brightness"
    control:
      kind: slider
      tickSpacing: 1
      immediate: true
      showValue: true
"#;
    let profile: StructureProfile =
        serde_yaml_ng::from_str(profile_yaml).expect("field presentation profile must parse");
    let cdi = parse_cdi(TEST_CDI).expect("test CDI must parse");
    let mut tree = build_node_config_tree("test:node", &cdi);

    let report = annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);

    assert_eq!(report.field_presentations_applied, 1);
    let brightness = find_leaf(&tree.segments[0].children, "Brightness")
        .expect("targeted integer leaf must exist");
    let slider = brightness
        .hint_slider
        .as_ref()
        .expect("profile must supply slider metadata");
    assert_eq!(slider.tick_spacing, 1);
    assert!(slider.immediate);
    assert!(slider.show_value);

    let unannotated = find_leaf(&tree.segments[0].children, "Unannotated")
        .expect("unannotated integer leaf must exist");
    let existing_slider = unannotated
        .hint_slider
        .as_ref()
        .expect("unannotated CDI slider hint must be preserved");
    assert_eq!(existing_slider.tick_spacing, 7);
    assert!(!existing_slider.immediate);
    assert!(!existing_slider.show_value);
}

#[test]
fn group_slider_presentation_applies_only_to_immediate_integer_leaf_children() {
    let profile_yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "Test"
  model: "Presentation"
fieldPresentation:
  - fieldPath: "Configuration/Controls"
    control:
      kind: slider
      tickSpacing: 2
      immediate: true
      showValue: true
"#;
    let profile: StructureProfile =
        serde_yaml_ng::from_str(profile_yaml).expect("field presentation profile must parse");
    let cdi = parse_cdi(TEST_CDI).expect("test CDI must parse");
    let mut tree = build_node_config_tree("test:node", &cdi);

    let report = annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);

    assert_eq!(report.field_presentations_applied, 2);
    for name in ["Brightness", "Unannotated"] {
        let slider = find_leaf(&tree.segments[0].children, name)
            .expect("direct integer leaf must exist")
            .hint_slider
            .as_ref()
            .expect("group presentation must annotate direct integer leaves");
        assert_eq!(slider.tick_spacing, 2);
        assert!(slider.immediate);
        assert!(slider.show_value);
    }

    let label = find_leaf(&tree.segments[0].children, "Label")
        .expect("direct non-integer leaf must exist");
    assert!(label.hint_slider.is_none());

    let nested = find_leaf(&tree.segments[0].children, "NestedValue")
        .expect("nested integer leaf must exist")
        .hint_slider
        .as_ref()
        .expect("nested CDI hint must remain");
    assert_eq!(nested.tick_spacing, 11);
    assert!(!nested.immediate);
    assert!(!nested.show_value);
}

#[test]
fn exact_leaf_presentation_precedes_parent_group_then_cdi_hint() {
    let profile_yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "Test"
  model: "Presentation"
fieldPresentation:
  - fieldPath: "Configuration/Controls/Brightness"
    control:
      kind: slider
      tickSpacing: 1
      immediate: true
      showValue: true
  - fieldPath: "Configuration/Controls"
    control:
      kind: slider
      tickSpacing: 2
      immediate: false
      showValue: true
"#;
    let profile: StructureProfile =
        serde_yaml_ng::from_str(profile_yaml).expect("field presentation profile must parse");
    let cdi = parse_cdi(TEST_CDI).expect("test CDI must parse");
    let mut tree = build_node_config_tree("test:node", &cdi);

    let report = annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);

    assert_eq!(report.field_presentations_applied, 2);

    let exact = find_leaf(&tree.segments[0].children, "Brightness")
        .expect("exactly targeted integer leaf must exist")
        .hint_slider
        .as_ref()
        .expect("exact leaf presentation must apply");
    assert_eq!(exact.tick_spacing, 1);
    assert!(exact.immediate);
    assert!(exact.show_value);

    let grouped = find_leaf(&tree.segments[0].children, "Unannotated")
        .expect("grouped integer leaf must exist")
        .hint_slider
        .as_ref()
        .expect("parent group presentation must apply");
    assert_eq!(grouped.tick_spacing, 2);
    assert!(!grouped.immediate);
    assert!(grouped.show_value);

    let outside = find_leaf(&tree.segments[0].children, "Outside")
        .expect("outside integer leaf must exist")
        .hint_slider
        .as_ref()
        .expect("outside CDI hint must remain");
    assert_eq!(outside.tick_spacing, 13);
    assert!(!outside.immediate);
    assert!(!outside.show_value);
}

#[test]
fn unresolved_and_ineligible_group_presentations_warn_and_apply_nothing() {
    let profile_yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "Test"
  model: "Presentation"
fieldPresentation:
  - fieldPath: "Configuration/Missing"
    control:
      kind: slider
      tickSpacing: 1
      immediate: true
      showValue: true
  - fieldPath: "Configuration/Ineligible"
    control:
      kind: slider
      tickSpacing: 1
      immediate: true
      showValue: true
"#;
    let profile: StructureProfile =
        serde_yaml_ng::from_str(profile_yaml).expect("field presentation profile must parse");
    let cdi = parse_cdi(TEST_CDI).expect("test CDI must parse");
    let mut tree = build_node_config_tree("test:node", &cdi);

    let report = annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);

    assert_eq!(report.field_presentations_applied, 0);
    assert_eq!(report.warnings.len(), 2);
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("Configuration/Missing") && warning.contains("could not be resolved")
    }));
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("Configuration/Ineligible")
            && warning.contains("resolved but matched no eligible immediate integer leaves in tree")
    }));
    assert!(find_leaf(&tree.segments[0].children, "TooDeep")
        .expect("nested integer leaf must exist")
        .hint_slider
        .is_none());
}

#[test]
fn invalid_field_presentation_targets_produce_explicit_warnings() {
    let profile_yaml = r#"
schemaVersion: "2.0"
nodeType:
  manufacturer: "Test"
  model: "Presentation"
fieldPresentation:
  - fieldPath: "Configuration/Controls/Missing"
    control:
      kind: slider
      tickSpacing: 1
      immediate: true
      showValue: true
  - fieldPath: "Configuration/Controls/Label"
    control:
      kind: slider
      tickSpacing: 1
      immediate: true
      showValue: true
"#;
    let profile: StructureProfile =
        serde_yaml_ng::from_str(profile_yaml).expect("field presentation profile must parse");
    let cdi = parse_cdi(TEST_CDI).expect("test CDI must parse");
    let mut tree = build_node_config_tree("test:node", &cdi);

    let report = annotate_tree(&mut tree, &profile, &BTreeMap::new(), &cdi);

    assert_eq!(report.field_presentations_applied, 0);
    assert_eq!(report.warnings.len(), 2);
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("Configuration/Controls/Missing")
            && warning.contains("could not be resolved")
    }));
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("Configuration/Controls/Label") && warning.contains("non-integer leaf")
    }));
}
