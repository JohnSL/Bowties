//! Profile module — public API
//!
//! Exposes profile loading, path resolution, and tree annotation to the rest
//! of the Tauri backend.  `annotate_tree` applies profile-declared event roles
//! (Phase 3 / US1) and relevance rule annotations (Phase 4 / US2, stub for
//! now) to a `NodeConfigTree`.

pub mod types;
pub mod loader;
pub mod resolver;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::node_tree::{
    ConfigNode,
    ConnectorConstraint,
    ConnectorConstraintEffect,
    ConnectorProfile,
    ConnectorScalarValue,
    ConnectorSlot,
    EmptyConnectorBehavior as NodeTreeEmptyConnectorBehavior,
    EmptyConnectorConstraintEffect,
    LeafType,
    NodeConfigTree,
    SlotSupportedDaughterboard,
    SupportedDaughterboard,
};
use lcc_rs::cdi::DataElement;

pub use types::{
    StructureProfile,
    ProfileNodeType,
    FirmwareVersionRange,
    ConnectorConstraintVariant,
    ConnectorCdiSignature,
    ConnectorCdiEnumCount,
    EventRoleDecl,
    ProfileEventRole,
    RelevanceRule,
    RelevanceCondition,
    RelevanceAnnotation,
    ConnectorSlotDefinition,
    EmptyConnectorBehavior,
    EmptyConnectorEffect,
    CarrierOverrideRule,
    ConnectorConstraintRule,
    ConnectorConstraintType,
    RepairRule,
    RepairStrategy,
    ProfileScalarValue,
    SharedDaughterboardLibrary,
    DaughterboardDefinition,
    DaughterboardConstraintVariant,
    DaughterboardMetadata,
};
pub use loader::{load_profile, load_shared_daughterboards};
pub use resolver::{DaughterboardReferenceSet, ProfilePathMap, referenced_daughterboard_ids, resolve_named_path, resolve_profile_paths};

// ─────────────────────────────────────────────────────────────────────────────
// Cache types
// ─────────────────────────────────────────────────────────────────────────────

/// Key: `"{manufacturer}::{model}"` — normalised (lowercase, trimmed).
pub type ProfileKey = String;

/// Build a [`ProfileKey`] from raw manufacturer and model strings.
///
/// Keys are normalised (lowercase, trimmed) so that minor whitespace variations
/// in the SNIP data do not cause cache misses.
pub fn make_profile_key(manufacturer: &str, model: &str) -> ProfileKey {
    format!(
        "{}::{}",
        manufacturer.trim().to_lowercase(),
        model.trim().to_lowercase()
    )
}

/// In-memory cache of loaded structure profiles.
///
/// `None` entry means "profile was looked up but not found", preventing
/// repeated file-system scans for node types without a bundled profile.
pub type ProfileCache = Arc<RwLock<HashMap<ProfileKey, Option<StructureProfile>>>>;

// ─────────────────────────────────────────────────────────────────────────────
// Annotation report
// ─────────────────────────────────────────────────────────────────────────────

/// Summary returned by [`annotate_tree`].
#[derive(Debug, Default)]
pub struct AnnotationReport {
    /// Number of eventid leaf roles overridden by profile declarations.
    pub event_roles_applied: usize,
    /// Number of relevance rules applied to the tree.
    pub rules_applied: usize,
    /// Warnings collected during annotation (also printed to stderr immediately).
    pub warnings: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// annotate_tree — Phase 3 (US1) implementation
// ─────────────────────────────────────────────────────────────────────────────

/// Annotate a `NodeConfigTree` with profile-sourced event roles and relevance
/// rule annotations.
///
/// **Phase 3 (US1)**: applies event role overrides from `profile.event_roles`.
/// For each declaration, resolves the name-based CDI group path to an index-
/// based path prefix, then walks the tree to find every matching `GroupNode`
/// (across all replicated instances) and sets `leaf.event_role` on every
/// `EventId` leaf inside.
///
/// **Phase 4 (US2)** relevance rule evaluation is a stub — returns zero
/// `rules_applied` until T020 is implemented.
///
/// Returns an [`AnnotationReport`] summarising the changes and any warnings.
pub fn annotate_tree(
    tree: &mut NodeConfigTree,
    profile: &StructureProfile,
    cdi: &lcc_rs::cdi::Cdi,
) -> AnnotationReport {
    let path_map = resolver::resolve_profile_paths(profile, cdi);
    let mut report = AnnotationReport::default();

    // ── US1: Event role overrides ────────────────────────────────────────────
    for decl in &profile.event_roles {
        match path_map.get(&decl.group_path) {
            Some(resolved_path) => {
                let role: lcc_rs::cdi::EventRole = decl.role.into();
                let applied = apply_event_role(tree, resolved_path, role, decl.label.as_deref());
                if applied == 0 {
                    let w = format!(
                        "[profile] Event role path '{}' resolved but matched no groups in tree",
                        decl.group_path
                    );
                    eprintln!("{}", w);
                    report.warnings.push(w);
                }
                report.event_roles_applied += applied;
            }
            None => {
                let w = format!(
                    "[profile] Event role path '{}' could not be resolved in CDI — skipped",
                    decl.group_path
                );
                eprintln!("{}", w);
                report.warnings.push(w);
            }
        }
    }

    // ── US2 stub (Phase 4 / T020) ────────────────────────────────────────────
    // Relevance rule evaluation is not yet implemented.
    // Phase 4 will annotate GroupNodes with RelevanceAnnotation here.

    report
}

pub fn build_connector_profile(
    node_id: &str,
    profile: &StructureProfile,
    library: Option<&SharedDaughterboardLibrary>,
    cdi: &lcc_rs::cdi::Cdi,
) -> Option<ConnectorProfile> {
    build_connector_profile_with_diagnostics(node_id, profile, library, cdi).profile
}

pub struct ConnectorProfileBuildOutcome {
    pub profile: Option<ConnectorProfile>,
    pub warning: Option<String>,
}

pub fn build_connector_profile_with_diagnostics(
    node_id: &str,
    profile: &StructureProfile,
    library: Option<&SharedDaughterboardLibrary>,
    cdi: &lcc_rs::cdi::Cdi,
) -> ConnectorProfileBuildOutcome {
    if profile.connector_slots.is_empty() {
        return ConnectorProfileBuildOutcome {
            profile: None,
            warning: None,
        };
    }

    let active_variant_id = match match_connector_constraint_variant(
        &profile.connector_constraint_variants,
        cdi,
    ) {
        Ok(variant_id) => variant_id,
        Err(reason) => {
            return ConnectorProfileBuildOutcome {
                profile: None,
                warning: Some(format!(
                    "Daughterboard constraints are unavailable for this node because its CDI does not match any declared connector settings variant ({}). Bowties is falling back to no daughterboard-specific filtering.",
                    reason
                )),
            };
        }
    };

    let carrier_key = make_profile_key(&profile.node_type.manufacturer, &profile.node_type.model);

    let supported_daughterboards = referenced_daughterboard_ids(profile)
        .into_iter()
        .map(|daughterboard_id| {
            let shared = library
                .and_then(|shared_library| {
                    shared_library
                        .daughterboards
                        .iter()
                        .find(|candidate| candidate.daughterboard_id == daughterboard_id)
                });

            SupportedDaughterboard {
                daughterboard_id: daughterboard_id.clone(),
                display_name: shared
                    .map(|candidate| candidate.display_name.clone())
                    .unwrap_or_else(|| daughterboard_id.clone()),
                kind: shared.and_then(|candidate| candidate.kind.clone()),
                description: shared.and_then(|candidate| {
                    candidate
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.notes.clone())
                }),
            }
        })
        .collect();

    ConnectorProfileBuildOutcome {
        profile: Some(ConnectorProfile {
            node_id: node_id.to_string(),
            carrier_key: carrier_key.clone(),
            slots: profile
                .connector_slots
                .iter()
                .map(|slot| ConnectorSlot {
                    slot_id: slot.slot_id.clone(),
                    label: slot.label.clone(),
                    order: slot.order,
                    allow_none_installed: slot.allow_none_installed,
                    supported_daughterboard_ids: slot.supported_daughterboard_ids.clone(),
                    affected_paths: slot.affected_paths.clone(),
                    resolved_affected_paths: slot
                        .affected_paths
                        .iter()
                        .filter_map(|path| resolver::resolve_named_path(path, cdi).ok())
                        .collect(),
                    base_behavior_when_empty: slot.base_behavior_when_empty.as_ref().map(map_empty_behavior),
                    supported_daughterboard_constraints: slot
                        .supported_daughterboard_ids
                        .iter()
                        .map(|daughterboard_id| SlotSupportedDaughterboard {
                            daughterboard_id: daughterboard_id.clone(),
                            validity_rules: collect_validity_rules(
                                profile,
                                library,
                                &carrier_key,
                                &slot.slot_id,
                                daughterboard_id,
                                active_variant_id.as_deref(),
                                cdi,
                            ),
                        })
                        .collect(),
                })
                .collect(),
            supported_daughterboards,
        }),
        warning: None,
    }
}

fn validate_connector_cdi_signature(
    signature: &types::ConnectorCdiSignature,
    cdi: &lcc_rs::cdi::Cdi,
) -> Result<(), String> {
    let mut mismatches = Vec::new();

    for path in &signature.required_paths {
        if let Err(error) = resolve_named_data_element(path, cdi) {
            mismatches.push(format!("missing '{}': {}", path, error));
        }
    }

    for count_rule in &signature.enum_entry_counts {
        match resolve_named_data_element(&count_rule.path, cdi) {
            Ok(DataElement::Int(int_element)) => {
                let actual_count = int_element
                    .map
                    .as_ref()
                    .map(|value_map| value_map.entries.len())
                    .unwrap_or(0);
                if actual_count != count_rule.count {
                    mismatches.push(format!(
                        "'{}' has {} enum values (expected {})",
                        count_rule.path, actual_count, count_rule.count
                    ));
                }
            }
            Ok(_) => mismatches.push(format!(
                "'{}' is not an integer enum field",
                count_rule.path
            )),
            Err(error) => mismatches.push(format!("missing '{}': {}", count_rule.path, error)),
        }
    }

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(mismatches.join("; "))
    }
}

fn match_connector_constraint_variant(
    variants: &[types::ConnectorConstraintVariant],
    cdi: &lcc_rs::cdi::Cdi,
) -> Result<Option<String>, String> {
    if variants.is_empty() {
        return Ok(None);
    }

    let mut matched_variant_ids = Vec::new();
    let mut mismatches = Vec::new();

    for variant in variants {
        match validate_connector_cdi_signature(&variant.cdi_signature, cdi) {
            Ok(()) => matched_variant_ids.push(variant.variant_id.clone()),
            Err(reason) => mismatches.push(format!("{}: {}", variant.variant_id, reason)),
        }
    }

    match matched_variant_ids.len() {
        0 => Err(mismatches.join("; ")),
        1 => Ok(matched_variant_ids.into_iter().next()),
        _ => Err(format!(
            "multiple connector variants matched the connected CDI ({})",
            matched_variant_ids.join(", ")
        )),
    }
}

fn resolve_named_data_element<'a>(
    path: &str,
    cdi: &'a lcc_rs::cdi::Cdi,
) -> Result<&'a DataElement, String> {
    let resolved_path = resolver::resolve_named_path(path, cdi)?;
    let (first_step, remaining_steps) = resolved_path
        .split_first()
        .ok_or_else(|| format!("resolved path for '{}' is empty", path))?;
    let seg_idx = parse_segment_step(first_step)?;
    let mut elements = &cdi
        .segments
        .get(seg_idx)
        .ok_or_else(|| format!("segment index {} out of range for '{}'", seg_idx, path))?
        .elements;

    for (index, step) in remaining_steps.iter().enumerate() {
        let elem_idx = parse_element_step(step)?;
        let element = elements
            .get(elem_idx)
            .ok_or_else(|| format!("element index {} out of range for '{}'", elem_idx, path))?;

        if index == remaining_steps.len() - 1 {
            return Ok(element);
        }

        match element {
            DataElement::Group(group) => {
                elements = &group.elements;
            }
            _ => {
                return Err(format!(
                    "'{}' traverses through non-group element '{}'",
                    path, step
                ));
            }
        }
    }

    Err(format!("'{}' resolved to a segment, not an element", path))
}

fn parse_segment_step(step: &str) -> Result<usize, String> {
    step.strip_prefix("seg:")
        .ok_or_else(|| format!("invalid segment step '{}'", step))?
        .parse::<usize>()
        .map_err(|error| format!("invalid segment index '{}': {}", step, error))
}

fn parse_element_step(step: &str) -> Result<usize, String> {
    let raw = step
        .strip_prefix("elem:")
        .ok_or_else(|| format!("invalid element step '{}'", step))?;
    let index_text = raw.split('#').next().unwrap_or(raw);
    index_text
        .parse::<usize>()
        .map_err(|error| format!("invalid element index '{}': {}", step, error))
}

fn collect_validity_rules(
    profile: &StructureProfile,
    library: Option<&SharedDaughterboardLibrary>,
    carrier_key: &str,
    slot_id: &str,
    daughterboard_id: &str,
    active_variant_id: Option<&str>,
    cdi: &lcc_rs::cdi::Cdi,
) -> Vec<ConnectorConstraint> {
    let mut rules = Vec::new();

    let matching_overrides: Vec<_> = profile
        .carrier_overrides
        .iter()
        .filter(|candidate| {
            candidate.daughterboard_id == daughterboard_id
                && candidate.carrier_key.trim().eq_ignore_ascii_case(carrier_key)
                && candidate.slot_id.as_deref().map(|value| value == slot_id).unwrap_or(true)
        })
        .collect();

    if let Some(shared_definition) = library
        .and_then(|shared_library| {
            shared_library
                .daughterboards
                .iter()
                .find(|candidate| candidate.daughterboard_id == daughterboard_id)
        })
    {
        let matched_shared_variant = active_variant_id.and_then(|variant_id| {
            shared_definition
                .constraint_variants
                .iter()
                .find(|candidate| candidate.variant_id == variant_id)
        });
        let replace_shared_via_override = matching_overrides
            .iter()
            .any(|candidate| candidate.replace_shared_validity_rules);
        let replace_base_via_variant = matched_shared_variant
            .map(|candidate| candidate.replace_base_validity_rules)
            .unwrap_or(false);

        if !replace_shared_via_override && !replace_base_via_variant {
            for rule in &shared_definition.validity_rules {
                if let Some(mapped) = map_constraint_rule(rule, cdi) {
                    rules.push(mapped);
                }
            }
        }

        if !replace_shared_via_override {
            if let Some(shared_variant) = matched_shared_variant {
                for rule in &shared_variant.validity_rules {
                    if let Some(mapped) = map_constraint_rule(rule, cdi) {
                        rules.push(mapped);
                    }
                }
            }
        }
    }

    for override_rule in matching_overrides {
        for rule in &override_rule.override_validity_rules {
            if let Some(mapped) = map_constraint_rule(rule, cdi) {
                rules.push(mapped);
            }
        }
    }

    rules
}

fn map_constraint_rule(
    rule: &ConnectorConstraintRule,
    cdi: &lcc_rs::cdi::Cdi,
) -> Option<ConnectorConstraint> {
    let resolved_path = resolver::resolve_named_path(&rule.target_path, cdi).ok()?;
    Some(ConnectorConstraint {
        target_path: rule.target_path.clone(),
        resolved_path,
        effect: map_constraint_effect(rule.constraint_type),
        line_ordinals: rule.line_ordinals.clone(),
        allowed_values: rule.allowed_values.iter().cloned().map(map_scalar_value).collect(),
        denied_values: rule.denied_values.iter().cloned().map(map_scalar_value).collect(),
        explanation: rule.explanation.clone(),
    })
}

fn map_constraint_effect(effect: ConnectorConstraintType) -> ConnectorConstraintEffect {
    match effect {
        ConnectorConstraintType::AllowValues => ConnectorConstraintEffect::AllowValues,
        ConnectorConstraintType::DenyValues => ConnectorConstraintEffect::DenyValues,
        ConnectorConstraintType::ShowSection => ConnectorConstraintEffect::Show,
        ConnectorConstraintType::HideSection => ConnectorConstraintEffect::Hide,
        ConnectorConstraintType::ReadOnly => ConnectorConstraintEffect::ReadOnly,
    }
}

fn map_scalar_value(value: ProfileScalarValue) -> ConnectorScalarValue {
    match value {
        ProfileScalarValue::String(value) => ConnectorScalarValue::String(value),
        ProfileScalarValue::Integer(value) => ConnectorScalarValue::Integer(value),
    }
}

fn map_empty_behavior(value: &EmptyConnectorBehavior) -> NodeTreeEmptyConnectorBehavior {
    NodeTreeEmptyConnectorBehavior {
        effect: match value.effect {
            EmptyConnectorEffect::HideDependent => EmptyConnectorConstraintEffect::Hide,
            EmptyConnectorEffect::DisableDependent => EmptyConnectorConstraintEffect::Disable,
            EmptyConnectorEffect::AllowSubset => EmptyConnectorConstraintEffect::AllowValues,
        },
        allowed_values: value.allowed_values.iter().cloned().map(map_scalar_value).collect(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private tree-traversal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Walk the entire tree and set `leaf.event_role = role` on every `EventId`
/// leaf inside groups whose stripped path equals `resolved_path`.
///
/// If `label` is `Some`, also sets `group.display_name` on every directly
/// matched group (but not its descendants).
///
/// Returns the number of `EventId` leaves modified.
fn apply_event_role(
    tree: &mut NodeConfigTree,
    resolved_path: &[String],
    role: lcc_rs::cdi::EventRole,
    label: Option<&str>,
) -> usize {
    let mut applied = 0usize;
    for segment in &mut tree.segments {
        walk_for_role(&mut segment.children, resolved_path, role, label, &mut applied);
    }
    applied
}

/// Recursive descent: for each `GroupNode`, check whether its path (with
/// instance suffixes stripped) equals `resolved_path`.
///
/// - **Match**: apply `role` to every `EventId` leaf descendant of this group;
///   if `label` is `Some`, set `group.display_name` on the matched group only
///   (not its descendants).  Do not recurse further for path matching.
/// - **No match**: recurse into this group's children to search deeper.
fn walk_for_role(
    children: &mut Vec<ConfigNode>,
    resolved_path: &[String],
    role: lcc_rs::cdi::EventRole,
    label: Option<&str>,
    applied: &mut usize,
) {
    for node in children.iter_mut() {
        if let ConfigNode::Group(group) = node {
            let stripped = resolver::strip_instance_steps(&group.path);
            if stripped == resolved_path {
                // Found a matching group — optionally override its display name.
                if let Some(lbl) = label {
                    group.display_name = Some(lbl.to_string());
                }
                // Apply the role to all EventId leaves within this group.
                *applied += set_roles_on_descendants(&mut group.children, role);
            } else {
                // Not a match at this level; keep searching deeper.
                walk_for_role(&mut group.children, resolved_path, role, label, applied);
            }
        }
        // Leaf nodes at this level are not traversed for path matching.
    }
}

/// Recursively set `event_role = role` on every `EventId` leaf found anywhere
/// in `children` (including nested groups).
///
/// Returns the count of leaves modified.
fn set_roles_on_descendants(
    children: &mut Vec<ConfigNode>,
    role: lcc_rs::cdi::EventRole,
) -> usize {
    let mut count = 0usize;
    for node in children.iter_mut() {
        match node {
            ConfigNode::Leaf(leaf) if leaf.element_type == LeafType::EventId => {
                leaf.event_role = Some(role);
                count += 1;
            }
            ConfigNode::Group(group) => {
                count += set_roles_on_descendants(&mut group.children, role);
            }
            _ => {}
        }
    }
    count
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use lcc_rs::cdi::{parser::parse_cdi, EventRole};
    use crate::node_tree::{build_node_config_tree, ConfigNode, LeafType};

    /// Minimal CDI with two non-replicated groups in one segment, each
    /// containing exactly one EventId leaf.
    const CDI_TWO_EVENT_GROUPS: &str = r#"<cdi>
        <segment space="253" origin="0">
            <name>TestSeg</name>
            <group>
                <name>GroupA</name>
                <eventid><name>EventA</name></eventid>
            </group>
            <group>
                <name>GroupB</name>
                <eventid><name>EventB</name></eventid>
            </group>
        </segment>
    </cdi>"#;

    /// Helper: find the first `EventId` leaf inside the group named `group_name`
    /// within `children`.  Looks at top-level direct children only.
    fn find_event_leaf_in_named_group<'a>(
        children: &'a [ConfigNode],
        group_name: &str,
    ) -> Option<&'a crate::node_tree::LeafNode> {
        for node in children {
            if let ConfigNode::Group(g) = node {
                if g.name == group_name {
                    for child in &g.children {
                        if let ConfigNode::Leaf(l) = child {
                            if l.element_type == LeafType::EventId {
                                return Some(l);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// T011 — annotate_tree must override event roles on matching group leaves.
    ///
    /// Build a tree with GroupA and GroupB, both with a single EventId leaf.
    /// Declare GroupA → Producer and GroupB → Consumer.  After annotation,
    /// verify the leaves carry those roles.
    #[test]
    fn annotate_tree_applies_event_roles() {
        let cdi = parse_cdi(CDI_TWO_EVENT_GROUPS).expect("CDI parse should succeed");
        let mut tree = build_node_config_tree("test:node", &cdi);

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test Node".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![
                types::EventRoleDecl {
                    group_path: "TestSeg/GroupA".to_string(),
                    role: types::ProfileEventRole::Producer,
                    label: None,
                },
                types::EventRoleDecl {
                    group_path: "TestSeg/GroupB".to_string(),
                    role: types::ProfileEventRole::Consumer,
                    label: None,
                },
            ],
            relevance_rules: vec![],
            connector_slots: vec![],
            connector_constraint_variants: vec![],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &cdi);

        assert_eq!(report.event_roles_applied, 2, "both EventId leaves should be annotated");
        assert!(
            report.warnings.is_empty(),
            "no warnings expected; got: {:?}",
            report.warnings
        );

        let seg_children = &tree.segments[0].children;

        let leaf_a = find_event_leaf_in_named_group(seg_children, "GroupA")
            .expect("GroupA must contain an EventId leaf");
        let leaf_b = find_event_leaf_in_named_group(seg_children, "GroupB")
            .expect("GroupB must contain an EventId leaf");

        assert_eq!(leaf_a.event_role, Some(EventRole::Producer),
            "GroupA leaf should have Producer role");
        assert_eq!(leaf_b.event_role, Some(EventRole::Consumer),
            "GroupB leaf should have Consumer role");
    }

    // ── make_profile_key ──────────────────────────────────────────────────────

    #[test]
    fn make_profile_key_strips_whitespace_and_lowercases() {
        let key = make_profile_key("  Acme Corp  ", "  Widget 100  ");
        assert_eq!(key, "acme corp::widget 100");
    }

    #[test]
    fn make_profile_key_already_lowercase_no_change() {
        let key = make_profile_key("acme", "model");
        assert_eq!(key, "acme::model");
    }

    // ── annotate_tree with label ───────────────────────────────────────────────

    #[test]
    fn annotate_tree_label_sets_display_name_on_matched_group() {
        let cdi = parse_cdi(CDI_TWO_EVENT_GROUPS).expect("CDI parse should succeed");
        let mut tree = build_node_config_tree("test:node", &cdi);

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test Node".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![
                types::EventRoleDecl {
                    group_path: "TestSeg/GroupA".to_string(),
                    role: types::ProfileEventRole::Producer,
                    label: Some("Output Events".to_string()),
                },
                types::EventRoleDecl {
                    group_path: "TestSeg/GroupB".to_string(),
                    role: types::ProfileEventRole::Consumer,
                    label: None, // no label
                },
            ],
            relevance_rules: vec![],
            connector_slots: vec![],
            connector_constraint_variants: vec![],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &cdi);
        assert_eq!(report.event_roles_applied, 2);
        assert!(report.warnings.is_empty());

        let seg_children = &tree.segments[0].children;
        // GroupA should have display_name set from label
        if let ConfigNode::Group(g) = &seg_children[0] {
            assert_eq!(g.display_name.as_deref(), Some("Output Events"), "GroupA should have display_name");
        } else {
            panic!("Expected GroupA");
        }
        // GroupB should have no display_name (label was None)
        if let ConfigNode::Group(g) = &seg_children[1] {
            assert!(g.display_name.is_none(), "GroupB should not have display_name");
        } else {
            panic!("Expected GroupB");
        }
    }

    // ── annotate_tree with replicated group ───────────────────────────────────

    const CDI_REPLICATED: &str = r#"<cdi>
        <segment space="253" origin="0">
            <name>Config</name>
            <group replication="3">
                <name>Events</name>
                <repname>Event</repname>
                <eventid><name>Event ID</name></eventid>
            </group>
        </segment>
    </cdi>"#;

    #[test]
    fn annotate_tree_replicated_group_annotates_all_instances() {
        let cdi = parse_cdi(CDI_REPLICATED).expect("CDI parse should succeed");
        let mut tree = build_node_config_tree("test:node", &cdi);

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![types::EventRoleDecl {
                group_path: "Config/Events".to_string(),
                role: types::ProfileEventRole::Producer,
                label: None,
            }],
            relevance_rules: vec![],
            connector_slots: vec![],
            connector_constraint_variants: vec![],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &cdi);
        // 3 replicated instances × 1 EventId each = 3 roles applied
        assert_eq!(report.event_roles_applied, 3, "All 3 replicated instances should be annotated");
        assert!(report.warnings.is_empty());

        // Verify all 3 instance EventId leaves have Producer role
        let wrapper = match &tree.segments[0].children[0] {
            ConfigNode::Group(g) => g,
            _ => panic!("Expected wrapper group"),
        };
        for instance in &wrapper.children {
            match instance {
                ConfigNode::Group(g) => match &g.children[0] {
                    ConfigNode::Leaf(l) => {
                        assert_eq!(l.event_role, Some(EventRole::Producer),
                            "Instance {:?} EventId should be Producer", g.instance);
                    }
                    _ => panic!("Expected leaf"),
                },
                _ => panic!("Expected instance group"),
            }
        }
    }

    // ── annotate_tree unresolved path ─────────────────────────────────────────

    #[test]
    fn annotate_tree_unresolved_path_produces_warning() {
        let cdi = parse_cdi(CDI_TWO_EVENT_GROUPS).expect("CDI parse should succeed");
        let mut tree = build_node_config_tree("test:node", &cdi);

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![types::EventRoleDecl {
                group_path: "TestSeg/NonExistentGroup".to_string(),
                role: types::ProfileEventRole::Producer,
                label: None,
            }],
            relevance_rules: vec![],
            connector_slots: vec![],
            connector_constraint_variants: vec![],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &cdi);
        assert_eq!(report.event_roles_applied, 0, "No roles should be applied for unresolved path");
        assert_eq!(report.warnings.len(), 1, "Should have exactly one warning");
        assert!(report.warnings[0].contains("TestSeg/NonExistentGroup"));
    }

    // ── annotate_tree resolved but no EventId leaves ──────────────────────────

    #[test]
    fn annotate_tree_resolved_group_without_eventid_leaves_produces_warning() {
        // CDI with a group that contains only Int leaves (no EventId)
        let cdi_int_only = r#"<cdi>
            <segment space="253" origin="0">
                <name>TestSeg</name>
                <group>
                    <name>Settings</name>
                    <int size="1"><name>Value</name></int>
                </group>
            </segment>
        </cdi>"#;
        let cdi = parse_cdi(cdi_int_only).expect("CDI parse should succeed");
        let mut tree = build_node_config_tree("test:node", &cdi);

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![types::EventRoleDecl {
                group_path: "TestSeg/Settings".to_string(),
                role: types::ProfileEventRole::Producer,
                label: None,
            }],
            relevance_rules: vec![],
            connector_slots: vec![],
            connector_constraint_variants: vec![],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &cdi);
        // Path resolves but no EventId leaves → applied == 0 → warning
        assert_eq!(report.event_roles_applied, 0);
        assert_eq!(report.warnings.len(), 1, "Should have one warning for matched-but-no-eventid");
        assert!(report.warnings[0].contains("TestSeg/Settings"));
    }

    #[test]
    fn build_connector_profile_keeps_shared_leaf_validity_rules() {
        let cdi = parse_cdi(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Port I/O</name>
                    <group replication="8">
                        <name>Line</name>
                        <repname>Line</repname>
                        <int size="1"><name>Output Function</name></int>
                    </group>
                </segment>
            </cdi>"#,
        )
        .expect("CDI parse should succeed");

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            connector_slots: vec![types::ConnectorSlotDefinition {
                slot_id: "connector-a".to_string(),
                label: "Connector A".to_string(),
                order: 0,
                allow_none_installed: true,
                supported_daughterboard_ids: vec!["BOD4".to_string()],
                affected_paths: vec!["Port I/O/Line#1".to_string()],
                base_behavior_when_empty: None,
            }],
            connector_constraint_variants: vec![],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let library = types::SharedDaughterboardLibrary {
            schema_version: "1.0".to_string(),
            manufacturer: "RR-CirKits".to_string(),
            daughterboards: vec![types::DaughterboardDefinition {
                daughterboard_id: "BOD4".to_string(),
                display_name: "BOD4".to_string(),
                kind: Some("detection".to_string()),
                validity_rules: vec![types::ConnectorConstraintRule {
                    target_path: "Port I/O/Line/Output Function".to_string(),
                    constraint_type: types::ConnectorConstraintType::AllowValues,
                    line_ordinals: vec![1, 2, 3, 4],
                    allowed_values: vec![types::ProfileScalarValue::Integer(0)],
                    denied_values: vec![],
                    explanation: Some("Input-only board".to_string()),
                }],
                repair_rules: vec![],
                defaults_when_selected: std::collections::BTreeMap::new(),
                constraint_variants: vec![],
                metadata: None,
            }],
        };

        let connector_profile = build_connector_profile(
            "05.02.01.02.03.00.00.01",
            &profile,
            Some(&library),
            &cdi,
        )
        .expect("connector profile should be built");

        let slot = connector_profile
            .slots
            .iter()
            .find(|candidate| candidate.slot_id == "connector-a")
            .expect("slot should be present");

        assert_eq!(
            slot.resolved_affected_paths,
            vec![vec!["seg:0".to_string(), "elem:0#1".to_string()]]
        );

        let rules = &slot.supported_daughterboard_constraints[0].validity_rules;
        assert_eq!(rules.len(), 1, "shared leaf rule should be preserved");
        assert_eq!(
            rules[0].resolved_path,
            vec!["seg:0".to_string(), "elem:0".to_string(), "elem:0".to_string()]
        );
        assert_eq!(rules[0].line_ordinals, vec![1, 2, 3, 4]);
        assert_eq!(rules[0].allowed_values, vec![ConnectorScalarValue::Integer(0)]);
    }

    #[test]
    fn build_connector_profile_disables_constraints_when_no_connector_variant_matches() {
        let cdi = parse_cdi(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Port I/O</name>
                    <group replication="8">
                        <name>Line</name>
                        <repname>Line</repname>
                        <int size="1">
                            <name>Output Function</name>
                            <map>
                                <relation><property>0</property><value>None</value></relation>
                                <relation><property>1</property><value>Steady</value></relation>
                                <relation><property>2</property><value>Pulse</value></relation>
                                <relation><property>3</property><value>Blink A</value></relation>
                                <relation><property>4</property><value>Blink B</value></relation>
                            </map>
                        </int>
                        <int size="1">
                            <name>Input Function</name>
                            <map>
                                <relation><property>0</property><value>None</value></relation>
                                <relation><property>1</property><value>Normal</value></relation>
                                <relation><property>2</property><value>Alternating</value></relation>
                            </map>
                        </int>
                    </group>
                </segment>
            </cdi>"#,
        )
        .expect("CDI parse should succeed");

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            connector_slots: vec![types::ConnectorSlotDefinition {
                slot_id: "connector-a".to_string(),
                label: "Connector A".to_string(),
                order: 0,
                allow_none_installed: true,
                supported_daughterboard_ids: vec!["BOD4".to_string()],
                affected_paths: vec!["Port I/O/Line#1".to_string()],
                base_behavior_when_empty: None,
            }],
            connector_constraint_variants: vec![types::ConnectorConstraintVariant {
                variant_id: "tower-lcc-legacy".to_string(),
                cdi_signature: types::ConnectorCdiSignature {
                    required_paths: vec!["Port I/O/Line/Output Function".to_string()],
                    enum_entry_counts: vec![
                        types::ConnectorCdiEnumCount {
                            path: "Port I/O/Line/Output Function".to_string(),
                            count: 17,
                        },
                        types::ConnectorCdiEnumCount {
                            path: "Port I/O/Line/Input Function".to_string(),
                            count: 9,
                        },
                    ],
                },
            }],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let outcome = build_connector_profile_with_diagnostics(
            "05.02.01.02.03.00.00.01",
            &profile,
            None,
            &cdi,
        );

        assert!(outcome.profile.is_none());
        assert!(
            outcome.warning.as_deref().unwrap_or_default().contains(
                "falling back to no daughterboard-specific filtering"
            )
        );
    }

    #[test]
    fn build_connector_profile_applies_variant_shared_validity_rules() {
        let cdi = parse_cdi(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Port I/O</name>
                    <group replication="8">
                        <name>Line</name>
                        <repname>Line</repname>
                        <int size="1"><name>Input Function</name></int>
                    </group>
                </segment>
            </cdi>"#,
        )
        .expect("CDI parse should succeed");

        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            connector_slots: vec![types::ConnectorSlotDefinition {
                slot_id: "connector-a".to_string(),
                label: "Connector A".to_string(),
                order: 0,
                allow_none_installed: true,
                supported_daughterboard_ids: vec!["BOD4".to_string()],
                affected_paths: vec!["Port I/O/Line#1".to_string()],
                base_behavior_when_empty: None,
            }],
            connector_constraint_variants: vec![types::ConnectorConstraintVariant {
                variant_id: "tower-lcc-c7".to_string(),
                cdi_signature: types::ConnectorCdiSignature {
                    required_paths: vec!["Port I/O/Line/Input Function".to_string()],
                    enum_entry_counts: vec![types::ConnectorCdiEnumCount {
                        path: "Port I/O/Line/Input Function".to_string(),
                        count: 0,
                    }],
                },
            }],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let library = types::SharedDaughterboardLibrary {
            schema_version: "1.0".to_string(),
            manufacturer: "RR-CirKits".to_string(),
            daughterboards: vec![types::DaughterboardDefinition {
                daughterboard_id: "BOD4".to_string(),
                display_name: "BOD4".to_string(),
                kind: Some("detection".to_string()),
                validity_rules: vec![types::ConnectorConstraintRule {
                    target_path: "Port I/O/Line/Input Function".to_string(),
                    constraint_type: types::ConnectorConstraintType::AllowValues,
                    line_ordinals: vec![1, 2, 3, 4],
                    allowed_values: vec![types::ProfileScalarValue::Integer(2)],
                    denied_values: vec![],
                    explanation: Some("Legacy detector mode".to_string()),
                }],
                repair_rules: vec![],
                defaults_when_selected: std::collections::BTreeMap::new(),
                constraint_variants: vec![types::DaughterboardConstraintVariant {
                    variant_id: "tower-lcc-c7".to_string(),
                    replace_base_validity_rules: true,
                    validity_rules: vec![types::ConnectorConstraintRule {
                        target_path: "Port I/O/Line/Input Function".to_string(),
                        constraint_type: types::ConnectorConstraintType::AllowValues,
                        line_ordinals: vec![1, 2, 3, 4],
                        allowed_values: vec![types::ProfileScalarValue::Integer(1)],
                        denied_values: vec![],
                        explanation: Some("C7 detector lines use Normal input mode".to_string()),
                    }],
                }],
                metadata: None,
            }],
        };

        let connector_profile = build_connector_profile(
            "05.02.01.02.03.00.00.01",
            &profile,
            Some(&library),
            &cdi,
        )
        .expect("connector profile should be built");

        let rules = &connector_profile.slots[0].supported_daughterboard_constraints[0].validity_rules;
        assert_eq!(rules.len(), 1, "variant rules should replace base rules");
        assert_eq!(rules[0].allowed_values, vec![ConnectorScalarValue::Integer(1)]);
    }
}
