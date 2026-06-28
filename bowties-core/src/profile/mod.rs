//! Profile module — public API
//!
//! Exposes profile loading, path resolution, and tree annotation to the rest
//! of the Tauri backend.  `annotate_tree` applies profile-declared event roles
//! (Phase 3 / US1) and relevance rule annotations (Phase 4 / US2, stub for
//! now) to a `NodeConfigTree`.

pub mod types;
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
    ConnectorCdiSignature,
    ConnectorCdiEnumCount,
    CdiSignatureVariantMatch,
    ConfigurationMode,
    Selector,
    Variant,
    Overlay,
    EventRoleDecl,
    ProfileEventRole,
    RelevanceRule,
    RelevanceCondition,
    RelevanceAnnotation,
    EmptyConnectorBehavior,
    EmptyConnectorEffect,
    ConnectorConstraintRule,
    ConnectorConstraintType,
    ProfileScalarValue,
    SharedDaughterboardLibrary,
    DaughterboardDefinition,
    DaughterboardConstraintVariant,
    DaughterboardMetadata,
    ChannelInputMapping,
};
pub use resolver::{ProfilePathMap, resolve_named_path, resolve_profile_paths};

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
// Overlay composition (v2, S2)
// ─────────────────────────────────────────────────────────────────────────────

/// One unknown-variant warning emitted by [`compose_overlays`] when a
/// `selections` entry names a variant id not declared by the mode.
///
/// Typed (rather than a free-form string) so the frontend can surface a
/// per-mode inline marker without parsing message text (FR-007).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownVariantWarning {
    /// The `ConfigurationMode.id` whose selection was invalid.
    pub mode_id: String,
    /// The `variant_id` the caller requested but the mode did not declare.
    pub requested_variant_id: String,
}

/// Result of composing every selected variant's overlay in declaration order
/// with last-write-wins per target.
///
/// Keys are the *declared* path strings from the profile YAML. Profiles use
/// canonical path strings; composition relies on string identity rather than
/// resolved-path equality to keep the function pure (CDI-free).
#[derive(Debug, Default)]
pub struct ComposedOverlays {
    /// Effective event-role declarations keyed by `group_path`.
    pub event_roles: std::collections::BTreeMap<String, types::EventRoleDecl>,
    /// Effective relevance rules keyed by `affected_target`.
    pub relevance_rules: std::collections::BTreeMap<String, types::RelevanceRule>,
    /// Effective structural-constraint rules keyed by `target_path`.
    pub structural_constraints:
        std::collections::BTreeMap<String, types::ConnectorConstraintRule>,
    /// Selections that named a variant not declared by their owning mode.
    /// Empty when every selection resolves to a declared variant.
    pub unknown_variants: Vec<UnknownVariantWarning>,
}

/// Compose the active overlays for `profile` under `selections`.
///
/// Walks `profile.configuration_modes` in declaration order. For each mode:
///
/// - If `selections` has no entry for the mode id, the mode contributes
///   nothing (the user has not picked a variant yet). FR-007 also fires here
///   when callers explicitly clear a selection.
/// - If the selected variant id is not declared, an [`UnknownVariantWarning`]
///   is appended and the mode contributes nothing (FR-007).
/// - Otherwise the variant's `Overlay` contributions are inserted into the
///   target-keyed maps. Within a single overlay, the inner array order is
///   preserved (`BTreeMap::insert` keeps the last value per key).
///
/// Then the profile's *top-level* `event_roles` and `relevance_rules` are
/// applied last, so an explicit profile-level statement wins over any overlay
/// targeting the same path (per data-model.md composition rule #3).
pub fn compose_overlays(
    profile: &StructureProfile,
    selections: &std::collections::BTreeMap<String, String>,
) -> ComposedOverlays {
    let mut out = ComposedOverlays::default();

    for mode in &profile.configuration_modes {
        let Some(requested) = selections.get(&mode.id) else {
            continue; // FR-007: no selection ⇒ no overlay
        };
        let Some(variant) = mode.variants.iter().find(|v| &v.id == requested) else {
            out.unknown_variants.push(UnknownVariantWarning {
                mode_id: mode.id.clone(),
                requested_variant_id: requested.clone(),
            });
            continue;
        };

        for decl in &variant.overlay.event_roles {
            out.event_roles.insert(decl.group_path.clone(), decl.clone());
        }
        for rule in &variant.overlay.relevance_rules {
            out.relevance_rules
                .insert(rule.affected_target.clone(), rule.clone());
        }
        for constraint in &variant.overlay.structural_constraints {
            out.structural_constraints
                .insert(constraint.target_path.clone(), constraint.clone());
        }
    }

    // Top-level (base) declarations win over overlays for the same target.
    for decl in &profile.event_roles {
        out.event_roles.insert(decl.group_path.clone(), decl.clone());
    }
    for rule in &profile.relevance_rules {
        out.relevance_rules
            .insert(rule.affected_target.clone(), rule.clone());
    }

    out
}

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
    /// Unknown-variant selections surfaced by [`compose_overlays`]. Empty when
    /// every active mode resolves to a declared variant (FR-007).
    pub unknown_variants: Vec<UnknownVariantWarning>,
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
/// **V2 (S2)**: also composes the active overlay set from
/// `profile.configuration_modes` under `selections` (FR-006: declaration order,
/// last-write-wins per target; FR-007: unknown variants surface in the report
/// and contribute no overlay) and applies its event-role contributions.
///
/// **Phase 4 (US2)** relevance rule evaluation is a stub — returns zero
/// `rules_applied` until that work lands.
///
/// `selections` is a map from `ConfigurationMode.id` to the chosen variant id.
/// Pass an empty map for callers that have no active selections yet (real-node
/// flows pre-S6, tests, etc.).
///
/// Returns an [`AnnotationReport`] summarising the changes and any warnings.
pub fn annotate_tree(
    tree: &mut NodeConfigTree,
    profile: &StructureProfile,
    selections: &std::collections::BTreeMap<String, String>,
    cdi: &lcc_rs::cdi::Cdi,
) -> AnnotationReport {
    let path_map = resolver::resolve_profile_paths(profile, cdi);
    let composed = compose_overlays(profile, selections);
    let mut report = AnnotationReport::default();
    report.unknown_variants = composed.unknown_variants;

    // ── US1 + S2: event role overrides (overlay-composed + base) ─────────────
    for decl in composed.event_roles.values() {
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

    // ── US2 stub ─────────────────────────────────────────────────────────────
    // Relevance rule evaluation (attaching RelevanceAnnotation to GroupNodes)
    // is a separate future slice. compose_overlays already exposes the
    // composed rules; the tree-walk apply step will consume them when it lands.

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
    // Collect StructuralSlot configuration modes in declaration order; these
    // describe the carrier's physical daughterboard slots.
    let slot_modes: Vec<(usize, &types::ConfigurationMode)> = profile
        .configuration_modes
        .iter()
        .enumerate()
        .filter(|(_, mode)| matches!(mode.selector, types::Selector::StructuralSlot { .. }))
        .collect();

    if slot_modes.is_empty() {
        return ConnectorProfileBuildOutcome {
            profile: None,
            warning: None,
        };
    }

    let active_variant_id = match resolve_cdi_signature_variant(&profile.configuration_modes, cdi) {
        Ok(variant_id) => variant_id,
        Err(reason) => {
            return ConnectorProfileBuildOutcome {
                profile: None,
                warning: Some(format!(
                    "Daughterboard constraints are unavailable for this node because its CDI does not match any declared firmware-revision signature ({}). Bowties is falling back to no daughterboard-specific filtering.",
                    reason
                )),
            };
        }
    };

    let carrier_key = make_profile_key(&profile.node_type.manufacturer, &profile.node_type.model);

    // supported_daughterboards = union of variant ids across all slot modes,
    // preserving first-seen declaration order.
    let mut seen_daughterboards: Vec<String> = Vec::new();
    for (_, mode) in &slot_modes {
        for variant in &mode.variants {
            if !seen_daughterboards.contains(&variant.id) {
                seen_daughterboards.push(variant.id.clone());
            }
        }
    }
    let supported_daughterboards: Vec<SupportedDaughterboard> = seen_daughterboards
        .into_iter()
        .map(|daughterboard_id| {
            let shared = library.and_then(|shared_library| {
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
                channel_inputs: shared
                    .and_then(|candidate| candidate.metadata.as_ref())
                    .map(|metadata| metadata.channel_inputs.clone())
                    .unwrap_or_default(),
            }
        })
        .collect();

    let slots: Vec<ConnectorSlot> = slot_modes
        .iter()
        .map(|(_, mode)| build_connector_slot(mode, library, &carrier_key, active_variant_id.as_deref(), cdi))
        .collect();

    ConnectorProfileBuildOutcome {
        profile: Some(ConnectorProfile {
            node_id: node_id.to_string(),
            carrier_key,
            slots,
            supported_daughterboards,
        }),
        warning: None,
    }
}

fn build_connector_slot(
    mode: &types::ConfigurationMode,
    library: Option<&SharedDaughterboardLibrary>,
    carrier_key: &str,
    active_variant_id: Option<&str>,
    cdi: &lcc_rs::cdi::Cdi,
) -> ConnectorSlot {
    let (slot_id, slot_label, slot_order, affected_paths, allow_none_installed, base_behavior_when_empty) =
        match &mode.selector {
            types::Selector::StructuralSlot {
                slot_id,
                slot_label,
                slot_order,
                affected_paths,
                allow_none_installed,
                base_behavior_when_empty,
            } => (
                slot_id.clone(),
                slot_label.clone(),
                *slot_order,
                affected_paths.clone(),
                *allow_none_installed,
                base_behavior_when_empty.as_ref().map(map_empty_behavior),
            ),
            _ => unreachable!("build_connector_slot called with non-StructuralSlot selector"),
        };

    let label = slot_label.unwrap_or_else(|| mode.label.clone());

    let supported_daughterboard_ids: Vec<String> =
        mode.variants.iter().map(|variant| variant.id.clone()).collect();

    let resolved_affected_paths: Vec<Vec<String>> = affected_paths
        .iter()
        .filter_map(|path| resolver::resolve_named_path(path, cdi).ok())
        .collect();

    let supported_daughterboard_constraints: Vec<SlotSupportedDaughterboard> =
        supported_daughterboard_ids
            .iter()
            .map(|daughterboard_id| SlotSupportedDaughterboard {
                daughterboard_id: daughterboard_id.clone(),
                validity_rules: collect_validity_rules(
                    library,
                    carrier_key,
                    &slot_id,
                    daughterboard_id,
                    active_variant_id,
                    cdi,
                ),
            })
            .collect();

    ConnectorSlot {
        slot_id,
        label,
        order: slot_order,
        allow_none_installed,
        supported_daughterboard_ids,
        affected_paths,
        resolved_affected_paths,
        base_behavior_when_empty,
        supported_daughterboard_constraints,
    }
}

fn resolve_cdi_signature_variant(
    modes: &[types::ConfigurationMode],
    cdi: &lcc_rs::cdi::Cdi,
) -> Result<Option<String>, String> {
    let signature_mode = modes.iter().find_map(|mode| match &mode.selector {
        types::Selector::CdiSignature { variants_signature } => Some(variants_signature),
        _ => None,
    });

    let Some(variants_signature) = signature_mode else {
        return Ok(None);
    };

    if variants_signature.is_empty() {
        return Ok(None);
    }

    let mut matched_variant_ids = Vec::new();
    let mut mismatches = Vec::new();

    for entry in variants_signature {
        match validate_connector_cdi_signature(&entry.signature, cdi) {
            Ok(()) => matched_variant_ids.push(entry.variant_id.clone()),
            Err(reason) => mismatches.push(format!("{}: {}", entry.variant_id, reason)),
        }
    }

    match matched_variant_ids.len() {
        0 => Err(mismatches.join("; ")),
        1 => Ok(matched_variant_ids.into_iter().next()),
        _ => Err(format!(
            "multiple firmware-revision signatures matched the connected CDI ({})",
            matched_variant_ids.join(", ")
        )),
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
    library: Option<&SharedDaughterboardLibrary>,
    _carrier_key: &str,
    _slot_id: &str,
    daughterboard_id: &str,
    active_variant_id: Option<&str>,
    cdi: &lcc_rs::cdi::Cdi,
) -> Vec<ConnectorConstraint> {
    let mut rules = Vec::new();

    let Some(shared_definition) = library.and_then(|shared_library| {
        shared_library
            .daughterboards
            .iter()
            .find(|candidate| candidate.daughterboard_id == daughterboard_id)
    }) else {
        return rules;
    };

    let matched_shared_variant = active_variant_id.and_then(|variant_id| {
        shared_definition
            .constraint_variants
            .iter()
            .find(|candidate| candidate.variant_id == variant_id)
    });
    let replace_base_via_variant = matched_shared_variant
        .map(|candidate| candidate.replace_base_validity_rules)
        .unwrap_or(false);

    if !replace_base_via_variant {
        for rule in &shared_definition.validity_rules {
            if let Some(mapped) = map_constraint_rule(rule, cdi) {
                rules.push(mapped);
            }
        }
    }

    if let Some(shared_variant) = matched_shared_variant {
        for rule in &shared_variant.validity_rules {
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
        replication_ordinals: rule.replication_ordinals.clone(),
        allowed_values: rule.allowed_values.iter().cloned().map(map_scalar_value).collect(),
        allowed_value_labels: rule.allowed_value_labels.clone(),
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
            configuration_modes: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &std::collections::BTreeMap::new(), &cdi);

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
            configuration_modes: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &std::collections::BTreeMap::new(), &cdi);
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
            configuration_modes: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &std::collections::BTreeMap::new(), &cdi);
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
            configuration_modes: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &std::collections::BTreeMap::new(), &cdi);
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
            configuration_modes: vec![],
        };

        let report = annotate_tree(&mut tree, &profile, &std::collections::BTreeMap::new(), &cdi);
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
            schema_version: "2.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            configuration_modes: vec![types::ConfigurationMode {
                id: "connector-a".to_string(),
                label: "Connector A".to_string(),
                selector: types::Selector::StructuralSlot {
                    slot_id: "connector-a".to_string(),
                    slot_label: Some("Connector A".to_string()),
                    slot_order: 0,
                    affected_paths: vec!["Port I/O/Line#1".to_string()],
                    allow_none_installed: true,
                    base_behavior_when_empty: None,
                },
                variants: vec![types::Variant {
                    id: "BOD4".to_string(),
                    label: "BOD4".to_string(),
                    overlay: types::Overlay::default(),
                }],
            }],
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
                    replication_ordinals: vec![],
                    allowed_values: vec![types::ProfileScalarValue::Integer(0)],
                    allowed_value_labels: vec![],
                    denied_values: vec![],
                    explanation: Some("Input-only board".to_string()),
                }],
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
            schema_version: "2.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            configuration_modes: vec![
                types::ConfigurationMode {
                    id: "firmware-revision".to_string(),
                    label: "Firmware revision".to_string(),
                    selector: types::Selector::CdiSignature {
                        variants_signature: vec![types::CdiSignatureVariantMatch {
                            variant_id: "tower-lcc-legacy".to_string(),
                            signature: types::ConnectorCdiSignature {
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
                    },
                    variants: vec![types::Variant {
                        id: "tower-lcc-legacy".to_string(),
                        label: "Legacy".to_string(),
                        overlay: types::Overlay::default(),
                    }],
                },
                types::ConfigurationMode {
                    id: "connector-a".to_string(),
                    label: "Connector A".to_string(),
                    selector: types::Selector::StructuralSlot {
                        slot_id: "connector-a".to_string(),
                        slot_label: Some("Connector A".to_string()),
                        slot_order: 0,
                        affected_paths: vec!["Port I/O/Line#1".to_string()],
                        allow_none_installed: true,
                        base_behavior_when_empty: None,
                    },
                    variants: vec![types::Variant {
                        id: "BOD4".to_string(),
                        label: "BOD4".to_string(),
                        overlay: types::Overlay::default(),
                    }],
                },
            ],
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
            schema_version: "2.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            configuration_modes: vec![
                types::ConfigurationMode {
                    id: "firmware-revision".to_string(),
                    label: "Firmware revision".to_string(),
                    selector: types::Selector::CdiSignature {
                        variants_signature: vec![types::CdiSignatureVariantMatch {
                            variant_id: "tower-lcc-c7".to_string(),
                            signature: types::ConnectorCdiSignature {
                                required_paths: vec!["Port I/O/Line/Input Function".to_string()],
                                enum_entry_counts: vec![types::ConnectorCdiEnumCount {
                                    path: "Port I/O/Line/Input Function".to_string(),
                                    count: 0,
                                }],
                            },
                        }],
                    },
                    variants: vec![types::Variant {
                        id: "tower-lcc-c7".to_string(),
                        label: "C7".to_string(),
                        overlay: types::Overlay::default(),
                    }],
                },
                types::ConfigurationMode {
                    id: "connector-a".to_string(),
                    label: "Connector A".to_string(),
                    selector: types::Selector::StructuralSlot {
                        slot_id: "connector-a".to_string(),
                        slot_label: Some("Connector A".to_string()),
                        slot_order: 0,
                        affected_paths: vec!["Port I/O/Line#1".to_string()],
                        allow_none_installed: true,
                        base_behavior_when_empty: None,
                    },
                    variants: vec![types::Variant {
                        id: "BOD4".to_string(),
                        label: "BOD4".to_string(),
                        overlay: types::Overlay::default(),
                    }],
                },
            ],
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
                    replication_ordinals: vec![],
                    allowed_values: vec![types::ProfileScalarValue::Integer(2)],
                    allowed_value_labels: vec![],
                    denied_values: vec![],
                    explanation: Some("Legacy detector mode".to_string()),
                }],
                constraint_variants: vec![types::DaughterboardConstraintVariant {
                    variant_id: "tower-lcc-c7".to_string(),
                    replace_base_validity_rules: true,
                    validity_rules: vec![types::ConnectorConstraintRule {
                        target_path: "Port I/O/Line/Input Function".to_string(),
                        constraint_type: types::ConnectorConstraintType::AllowValues,
                        line_ordinals: vec![1, 2, 3, 4],
                        replication_ordinals: vec![],
                        allowed_values: vec![types::ProfileScalarValue::Integer(1)],
                        allowed_value_labels: vec![],
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

    #[test]
    fn build_connector_profile_keeps_slot_validity_rules() {
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
            schema_version: "2.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            configuration_modes: vec![
                types::ConfigurationMode {
                    id: "firmware-revision".to_string(),
                    label: "Firmware revision".to_string(),
                    selector: types::Selector::CdiSignature {
                        variants_signature: vec![types::CdiSignatureVariantMatch {
                            variant_id: "tower-lcc-c7".to_string(),
                            signature: types::ConnectorCdiSignature {
                                required_paths: vec!["Port I/O/Line/Input Function".to_string()],
                                enum_entry_counts: vec![types::ConnectorCdiEnumCount {
                                    path: "Port I/O/Line/Input Function".to_string(),
                                    count: 0,
                                }],
                            },
                        }],
                    },
                    variants: vec![types::Variant {
                        id: "tower-lcc-c7".to_string(),
                        label: "C7".to_string(),
                        overlay: types::Overlay::default(),
                    }],
                },
                types::ConfigurationMode {
                    id: "connector-a".to_string(),
                    label: "Connector A".to_string(),
                    selector: types::Selector::StructuralSlot {
                        slot_id: "connector-a".to_string(),
                        slot_label: Some("Connector A".to_string()),
                        slot_order: 0,
                        affected_paths: vec!["Port I/O/Line#1".to_string()],
                        allow_none_installed: true,
                        base_behavior_when_empty: None,
                    },
                    variants: vec![types::Variant {
                        id: "BOD4-CP".to_string(),
                        label: "BOD4-CP".to_string(),
                        overlay: types::Overlay::default(),
                    }],
                },
            ],
        };

        let library = types::SharedDaughterboardLibrary {
            schema_version: "1.0".to_string(),
            manufacturer: "RR-CirKits".to_string(),
            daughterboards: vec![types::DaughterboardDefinition {
                daughterboard_id: "BOD4-CP".to_string(),
                display_name: "BOD4-CP".to_string(),
                kind: Some("detection".to_string()),
                validity_rules: vec![types::ConnectorConstraintRule {
                    target_path: "Port I/O/Line/Input Function".to_string(),
                    constraint_type: types::ConnectorConstraintType::AllowValues,
                    line_ordinals: vec![],
                    replication_ordinals: vec![],
                    allowed_values: vec![types::ProfileScalarValue::Integer(1)],
                    allowed_value_labels: vec![],
                    denied_values: vec![],
                    explanation: Some("Detector lines use Normal input mode".to_string()),
                }],
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

        let slot_rules = &connector_profile.slots[0].supported_daughterboard_constraints[0];
        assert_eq!(slot_rules.validity_rules.len(), 1);
        assert_eq!(slot_rules.validity_rules[0].target_path, "Port I/O/Line/Input Function");
        assert_eq!(slot_rules.validity_rules[0].resolved_path, vec![
            "seg:0".to_string(),
            "elem:0".to_string(),
            "elem:0".to_string(),
        ]);
        assert_eq!(slot_rules.validity_rules[0].allowed_values, vec![ConnectorScalarValue::Integer(1)]);
    }

    #[test]
    fn bundled_tower_profile_builds_quickstart_connector_slots() {
        let cdi = parse_cdi(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Port I/O</name>
                    <group replication="16">
                        <name>Line</name>
                        <repname>Line</repname>
                        <int size="1">
                            <name>Output Function</name>
                            <map>
                                <relation><property>0</property><value>None</value></relation>
                                <relation><property>1</property><value>Mode 1</value></relation>
                                <relation><property>2</property><value>Mode 2</value></relation>
                                <relation><property>3</property><value>Mode 3</value></relation>
                                <relation><property>4</property><value>Mode 4</value></relation>
                                <relation><property>5</property><value>Mode 5</value></relation>
                                <relation><property>6</property><value>Mode 6</value></relation>
                                <relation><property>7</property><value>Mode 7</value></relation>
                                <relation><property>8</property><value>Mode 8</value></relation>
                                <relation><property>9</property><value>Mode 9</value></relation>
                                <relation><property>10</property><value>Mode 10</value></relation>
                                <relation><property>11</property><value>Mode 11</value></relation>
                                <relation><property>12</property><value>Mode 12</value></relation>
                                <relation><property>13</property><value>Mode 13</value></relation>
                                <relation><property>14</property><value>Mode 14</value></relation>
                                <relation><property>15</property><value>Mode 15</value></relation>
                                <relation><property>16</property><value>Mode 16</value></relation>
                            </map>
                        </int>
                        <int size="1">
                            <name>Input Function</name>
                            <map>
                                <relation><property>0</property><value>Mode 0</value></relation>
                                <relation><property>1</property><value>Mode 1</value></relation>
                                <relation><property>2</property><value>Mode 2</value></relation>
                                <relation><property>3</property><value>Mode 3</value></relation>
                                <relation><property>4</property><value>Mode 4</value></relation>
                                <relation><property>5</property><value>Mode 5</value></relation>
                                <relation><property>6</property><value>Mode 6</value></relation>
                                <relation><property>7</property><value>Mode 7</value></relation>
                                <relation><property>8</property><value>Mode 8</value></relation>
                            </map>
                        </int>
                    </group>
                </segment>
            </cdi>"#,
        )
        .expect("CDI parse should succeed");

        let profile: StructureProfile = serde_yaml_ng::from_str(include_str!("../../../app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml"))
            .expect("bundled Tower-LCC profile should parse");
        let library: types::SharedDaughterboardLibrary = serde_yaml_ng::from_str(include_str!("../../../app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml"))
            .expect("bundled shared daughterboard library should parse");

        let connector_profile = build_connector_profile(
            "05.02.01.02.03.00.00.01",
            &profile,
            Some(&library),
            &cdi,
        )
        .expect("Tower-LCC connector profile should be built");

        assert_eq!(connector_profile.slots.len(), 2);
        assert_eq!(connector_profile.slots[0].resolved_affected_paths.len(), 8);
        assert_eq!(connector_profile.slots[1].resolved_affected_paths.len(), 8);
        assert!(connector_profile
            .supported_daughterboards
            .iter()
            .any(|candidate| candidate.daughterboard_id == "BOD4"));
    }

    // Deleted 2026-06-28 — `bundled_signal_profile_builds_aux_port_slot_without_governed_paths`
    // pinned `RR-CirKits_Signal-LCC-P.profile.yaml`, which was removed when
    // the `-P` / `-S` / AI-fabricated `-32H` variants were superseded by the
    // generic `RR-CirKits_Inc._Signal-LCC.profile.yaml`. The test's intent
    // (empty-`affectedPaths` structural slot smoke test) is reinstated under
    // a hand-crafted capability fixture in spec 018 / S1.1
    // (Two-tier profile-test fixtures + bundled-profile smoke validation).

    #[test]
    fn bundled_breakout_boards_do_not_add_line_constraints() {
        let library: types::SharedDaughterboardLibrary = serde_yaml_ng::from_str(include_str!("../../../app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml"))
            .expect("bundled shared daughterboard library should parse");

        for daughterboard_id in ["FOB-A", "FOB-C", "BOB-S"] {
            let daughterboard = library
                .daughterboards
                .iter()
                .find(|candidate| candidate.daughterboard_id == daughterboard_id)
                .unwrap_or_else(|| panic!("{daughterboard_id} should exist in the bundled daughterboard library"));

            assert!(
                daughterboard.validity_rules.is_empty(),
                "{daughterboard_id} should not add connector constraints because the manual allows per-line input or output use"
            );
        }
    }

    #[test]
    fn bundled_detector_boards_constrain_producer_actions_per_replication() {
        let library: types::SharedDaughterboardLibrary = serde_yaml_ng::from_str(include_str!("../../../app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml"))
            .expect("bundled shared daughterboard library should parse");

        for daughterboard_id in ["BOD4", "BOD4-CP", "BOD-8-SM"] {
            let daughterboard = library
                .daughterboards
                .iter()
                .find(|candidate| candidate.daughterboard_id == daughterboard_id)
                .unwrap_or_else(|| panic!("{daughterboard_id} should exist in the bundled daughterboard library"));

            let rules = match daughterboard_id {
                "BOD4" | "BOD4-CP" => &daughterboard.constraint_variants[0].validity_rules,
                _ => &daughterboard.validity_rules,
            };

            // Slot 1 (producerLeafIndex 0 = occupied) must be Input On
            let slot1_rule = rules
                .iter()
                .find(|rule| {
                    rule.target_path == "Port I/O/Line/Event#2/Upon this action"
                        && rule.constraint_type == types::ConnectorConstraintType::AllowValues
                        && rule.replication_ordinals == vec![1]
                })
                .unwrap_or_else(|| panic!("{daughterboard_id} should constrain producer slot 1"));
            assert_eq!(
                slot1_rule.allowed_values,
                vec![types::ProfileScalarValue::Integer(5)],
                "{daughterboard_id} slot 1 should be forced to Input On (occupied — detector output goes low, Active Lo / Low (0V) polarity makes logical input ON)"
            );

            // Slot 2 (producerLeafIndex 1 = clear) must be Input Off
            let slot2_rule = rules
                .iter()
                .find(|rule| {
                    rule.target_path == "Port I/O/Line/Event#2/Upon this action"
                        && rule.constraint_type == types::ConnectorConstraintType::AllowValues
                        && rule.replication_ordinals == vec![2]
                })
                .unwrap_or_else(|| panic!("{daughterboard_id} should constrain producer slot 2"));
            assert_eq!(
                slot2_rule.allowed_values,
                vec![types::ProfileScalarValue::Integer(6)],
                "{daughterboard_id} slot 2 should be forced to Input Off (clear — detector output goes high, Active Lo / Low (0V) polarity makes logical input OFF)"
            );

            // No broad all-replications rule should exist
            let broad_rule = rules.iter().any(|rule| {
                rule.target_path == "Port I/O/Line/Event#2/Upon this action"
                    && rule.constraint_type == types::ConnectorConstraintType::AllowValues
                    && rule.replication_ordinals.is_empty()
            });
            assert!(
                !broad_rule,
                "{daughterboard_id} should not have a broad constraint on all producer event replications"
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // S2 — compose_overlays: deterministic declaration-order, last-write-wins
    // ─────────────────────────────────────────────────────────────────────────

    /// Build a minimal v2 profile with no top-level event-role or relevance
    /// declarations and a caller-supplied list of configuration modes.
    fn v2_profile_with_modes(modes: Vec<types::ConfigurationMode>) -> StructureProfile {
        StructureProfile {
            schema_version: "2.0".to_string(),
            node_type: types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            configuration_modes: modes,
        }
    }

    fn enum_mode(
        id: &str,
        field_path: &str,
        variants: Vec<types::Variant>,
    ) -> types::ConfigurationMode {
        types::ConfigurationMode {
            id: id.to_string(),
            label: id.to_string(),
            selector: types::Selector::EnumField {
                field_path: field_path.to_string(),
            },
            variants,
        }
    }

    fn variant_with_event_role(
        variant_id: &str,
        group_path: &str,
        role: types::ProfileEventRole,
    ) -> types::Variant {
        types::Variant {
            id: variant_id.to_string(),
            label: variant_id.to_string(),
            overlay: types::Overlay {
                event_roles: vec![types::EventRoleDecl {
                    group_path: group_path.to_string(),
                    role,
                    label: None,
                }],
                relevance_rules: vec![],
                structural_constraints: vec![],
            },
        }
    }

    /// S2-T1: Two ConfigurationModes whose selected variants both contribute
    /// an event-role overlay for the *same* group path must compose in
    /// declaration order with last-write-wins per target.  The second mode
    /// declared in YAML wins.
    #[test]
    fn compose_overlays_two_modes_last_write_wins_event_role() {
        // Mode "side" (declared first) → its "left" variant sets the group to Producer.
        // Mode "feature" (declared second) → its "on" variant sets the same group to Consumer.
        // With both selected, the second mode's overlay must override the first.
        let profile = v2_profile_with_modes(vec![
            enum_mode(
                "side",
                "Cfg/Side",
                vec![variant_with_event_role(
                    "left",
                    "Seg/Events",
                    types::ProfileEventRole::Producer,
                )],
            ),
            enum_mode(
                "feature",
                "Cfg/Feature",
                vec![variant_with_event_role(
                    "on",
                    "Seg/Events",
                    types::ProfileEventRole::Consumer,
                )],
            ),
        ]);

        let mut selections = std::collections::BTreeMap::new();
        selections.insert("side".to_string(), "left".to_string());
        selections.insert("feature".to_string(), "on".to_string());

        let composed = compose_overlays(&profile, &selections);

        let role_decl = composed
            .event_roles
            .get("Seg/Events")
            .expect("composed overlays must include the shared target");
        assert_eq!(
            role_decl.role,
            types::ProfileEventRole::Consumer,
            "second-declared mode's overlay must win (last-write-wins per target)"
        );
        assert!(
            composed.unknown_variants.is_empty(),
            "no unknown-variant warnings expected: {:?}",
            composed.unknown_variants
        );
    }

    /// S2-T1 (cont.): Top-level (base) event-role declarations must win over
    /// any overlay touching the same target — overlays apply first, base
    /// applies last, BTreeMap last-write-wins delivers the rule.
    #[test]
    fn compose_overlays_base_event_role_overrides_variant_overlay() {
        let mut profile = v2_profile_with_modes(vec![enum_mode(
            "side",
            "Cfg/Side",
            vec![variant_with_event_role(
                "left",
                "Seg/Events",
                types::ProfileEventRole::Producer,
            )],
        )]);
        // Base-level declaration for the same path with the opposite role.
        profile.event_roles.push(types::EventRoleDecl {
            group_path: "Seg/Events".to_string(),
            role: types::ProfileEventRole::Consumer,
            label: None,
        });

        let mut selections = std::collections::BTreeMap::new();
        selections.insert("side".to_string(), "left".to_string());

        let composed = compose_overlays(&profile, &selections);

        let role_decl = composed
            .event_roles
            .get("Seg/Events")
            .expect("composed overlays must include the shared target");
        assert_eq!(
            role_decl.role,
            types::ProfileEventRole::Consumer,
            "top-level (base) declaration must override the variant overlay"
        );
    }

    /// S2-T5 (FR-007): A selection that names a variant the mode does not
    /// declare produces a typed UnknownVariantWarning and contributes no
    /// overlay — composition does NOT abort.
    #[test]
    fn compose_overlays_unknown_variant_emits_typed_warning_and_skips_overlay() {
        let profile = v2_profile_with_modes(vec![enum_mode(
            "side",
            "Cfg/Side",
            vec![variant_with_event_role(
                "left",
                "Seg/Events",
                types::ProfileEventRole::Producer,
            )],
        )]);

        let mut selections = std::collections::BTreeMap::new();
        selections.insert("side".to_string(), "diagonal".to_string()); // not declared

        let composed = compose_overlays(&profile, &selections);

        assert!(
            composed.event_roles.is_empty(),
            "unknown variant must contribute no overlay"
        );
        assert_eq!(
            composed.unknown_variants,
            vec![UnknownVariantWarning {
                mode_id: "side".to_string(),
                requested_variant_id: "diagonal".to_string(),
            }],
            "exactly one typed warning naming the offending mode + requested variant id"
        );
    }

    /// S2-T4: `annotate_tree` accepts a `selections` map and applies the
    /// composed event-role overlay to the tree. With the second-declared mode
    /// selected, its variant's overlay must win over the first mode's.
    #[test]
    fn annotate_tree_applies_composed_overlay_event_role() {
        let cdi = parse_cdi(CDI_TWO_EVENT_GROUPS).expect("CDI parse should succeed");
        let mut tree = build_node_config_tree("test:node", &cdi);

        // Two modes both target GroupA's eventid. Mode "second" is declared
        // after "first" — its selected variant must win.
        let profile = v2_profile_with_modes(vec![
            enum_mode(
                "first",
                "TestSeg/GroupB", // selector path is irrelevant to overlay application
                vec![variant_with_event_role(
                    "v",
                    "TestSeg/GroupA",
                    types::ProfileEventRole::Producer,
                )],
            ),
            enum_mode(
                "second",
                "TestSeg/GroupB",
                vec![variant_with_event_role(
                    "v",
                    "TestSeg/GroupA",
                    types::ProfileEventRole::Consumer,
                )],
            ),
        ]);

        let mut selections = std::collections::BTreeMap::new();
        selections.insert("first".to_string(), "v".to_string());
        selections.insert("second".to_string(), "v".to_string());

        let report = annotate_tree(&mut tree, &profile, &selections, &cdi);

        assert_eq!(
            report.event_roles_applied, 1,
            "exactly one EventId leaf should be annotated (deduped composed overlay)"
        );
        assert!(report.unknown_variants.is_empty());

        let leaf = find_event_leaf_in_named_group(&tree.segments[0].children, "GroupA")
            .expect("GroupA must contain an EventId leaf");
        assert_eq!(
            leaf.event_role,
            Some(EventRole::Consumer),
            "second-declared mode's Consumer overlay must win"
        );
    }

    /// S2-T5: An unknown-variant selection surfaces in
    /// `AnnotationReport.unknown_variants` (typed, not just a warning string),
    /// and the corresponding overlay is skipped — annotation does not abort.
    #[test]
    fn annotate_tree_unknown_variant_selection_surfaces_in_report() {
        let cdi = parse_cdi(CDI_TWO_EVENT_GROUPS).expect("CDI parse should succeed");
        let mut tree = build_node_config_tree("test:node", &cdi);

        let profile = v2_profile_with_modes(vec![enum_mode(
            "side",
            "TestSeg/GroupB",
            vec![variant_with_event_role(
                "left",
                "TestSeg/GroupA",
                types::ProfileEventRole::Producer,
            )],
        )]);

        let mut selections = std::collections::BTreeMap::new();
        selections.insert("side".to_string(), "diagonal".to_string());

        let report = annotate_tree(&mut tree, &profile, &selections, &cdi);

        assert_eq!(
            report.event_roles_applied, 0,
            "unknown variant must contribute no overlay"
        );
        assert_eq!(
            report.unknown_variants,
            vec![UnknownVariantWarning {
                mode_id: "side".to_string(),
                requested_variant_id: "diagonal".to_string(),
            }],
            "report must surface the typed unknown-variant warning"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // S5 — Tower-LCC v2 migration parity
    //
    // The bundled `RR-CirKits_Tower-LCC.profile.yaml` is re-expressed under
    // the v2 schema (configurationModes for firmware revision + the two
    // daughterboard slots). `build_connector_profile` derives the same
    // `ConnectorProfile` shape from configurationModes that the v1 path
    // produced from connectorSlots + connectorConstraintVariants +
    // daughterboardReferences. These tests pin the user-visible parity.
    // ─────────────────────────────────────────────────────────────────────────

    /// Expected supported daughterboards on every Tower-LCC connector slot,
    /// in the order they appear in the bundled v1 profile.
    const TOWER_LCC_SUPPORTED_DAUGHTERBOARDS: &[&str] = &[
        "BOD4",
        "BOD4-CP",
        "BOD-8-SM",
        "FOB-A",
        "FOB-C",
        "BOB-S",
        "OI-IB-8",
        "OI-OB-8",
        "RB-2",
        "RB-4",
        "SMD-8",
        "SCSD-8",
        "Isolator-8",
        "MSS I/OAdapter",
        "Sampled I/O Splitter Pair",
        "I/O Test-SM",
    ];

    /// Build a synthetic Tower-LCC CDI shaped to the legacy connector signature:
    /// `Output Function` carries 17 enum entries, `Input Function` carries 9.
    fn make_tower_lcc_legacy_cdi() -> lcc_rs::cdi::Cdi {
        let mut out_map = String::new();
        for i in 0..17 {
            out_map.push_str(&format!(
                "<relation><property>{i}</property><value>Out{i}</value></relation>"
            ));
        }
        let mut in_map = String::new();
        for i in 0..9 {
            in_map.push_str(&format!(
                "<relation><property>{i}</property><value>In{i}</value></relation>"
            ));
        }
        let xml = format!(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Port I/O</name>
                    <group replication="16">
                        <name>Line</name>
                        <repname>Line</repname>
                        <int size="1"><name>Output Function</name><map>{out_map}</map></int>
                        <int size="1"><name>Input Function</name><map>{in_map}</map></int>
                        <group replication="6">
                            <name>Event</name>
                            <eventid><name>Consumer Event</name></eventid>
                        </group>
                        <group replication="6">
                            <name>Event</name>
                            <eventid><name>Producer Event</name></eventid>
                            <int size="1"><name>Upon this action</name></int>
                        </group>
                    </group>
                </segment>
            </cdi>"#
        );
        parse_cdi(&xml).expect("legacy Tower-LCC CDI should parse")
    }

    /// Build a synthetic Tower-LCC CDI shaped to the C7 connector signature:
    /// `Output Function` carries 5 entries, `Input Function` carries 3, and
    /// the two C7-only sibling groups are present.
    fn make_tower_lcc_c7_cdi() -> lcc_rs::cdi::Cdi {
        let mut out_map = String::new();
        for i in 0..5 {
            out_map.push_str(&format!(
                "<relation><property>{i}</property><value>Out{i}</value></relation>"
            ));
        }
        let mut in_map = String::new();
        for i in 0..3 {
            in_map.push_str(&format!(
                "<relation><property>{i}</property><value>In{i}</value></relation>"
            ));
        }
        let xml = format!(
            r#"<cdi>
                <segment space="253" origin="0">
                    <name>Port I/O</name>
                    <group replication="16">
                        <name>Line</name>
                        <repname>Line</repname>
                        <int size="1"><name>Output Function</name><map>{out_map}</map></int>
                        <group>
                            <name>Receiving the configured Command (C) event(s) will drive or pulse the line:</name>
                            <int size="1"><name>Drive Polarity</name></int>
                        </group>
                        <int size="1"><name>Input Function</name><map>{in_map}</map></int>
                        <group>
                            <name>The configured Indication (P) event(s) will be sent when the line is driven:</name>
                            <int size="1"><name>Indicate Polarity</name></int>
                        </group>
                        <group replication="6">
                            <name>Event</name>
                            <eventid><name>Consumer Event</name></eventid>
                        </group>
                        <group replication="6">
                            <name>Event</name>
                            <eventid><name>Producer Event</name></eventid>
                            <int size="1"><name>Upon this action</name></int>
                        </group>
                    </group>
                </segment>
            </cdi>"#
        );
        parse_cdi(&xml).expect("C7 Tower-LCC CDI should parse")
    }

    fn load_bundled_tower_lcc_profile() -> StructureProfile {
        serde_yaml_ng::from_str(include_str!(
            "../../../app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml"
        ))
        .expect("bundled Tower-LCC profile must load under v2")
    }

    fn load_bundled_shared_daughterboards() -> types::SharedDaughterboardLibrary {
        serde_yaml_ng::from_str(include_str!(
            "../../../app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml"
        ))
        .expect("bundled shared daughterboard library must load")
    }

    /// The bundled Tower-LCC profile parses cleanly under v2 and declares the
    /// three configuration modes the migration introduces:
    ///
    /// 1. `firmware-revision` — `Selector::CdiSignature` choosing
    ///    `tower-lcc-legacy` vs `tower-lcc-c7`.
    /// 2. `connector-a` — `Selector::StructuralSlot` for the first carrier
    ///    slot, with all 16 supported daughterboard variants.
    /// 3. `connector-b` — same shape as `connector-a` for the second slot.
    #[test]
    fn s5_tower_lcc_v2_profile_declares_firmware_and_connector_modes() {
        let profile = load_bundled_tower_lcc_profile();

        assert_eq!(
            profile.schema_version, "2.0",
            "Tower-LCC profile must declare v2 schema"
        );
        assert_eq!(
            profile.configuration_modes.len(),
            3,
            "expect firmware-revision + two connector modes"
        );

        let firmware = profile
            .configuration_modes
            .iter()
            .find(|mode| mode.id == "firmware-revision")
            .expect("firmware-revision mode must be declared");
        match &firmware.selector {
            types::Selector::CdiSignature { variants_signature } => {
                let ids: Vec<&str> = variants_signature
                    .iter()
                    .map(|entry| entry.variant_id.as_str())
                    .collect();
                assert!(
                    ids.contains(&"tower-lcc-legacy"),
                    "legacy firmware signature must be declared"
                );
                assert!(
                    ids.contains(&"tower-lcc-c7"),
                    "C7 firmware signature must be declared"
                );
            }
            other => panic!("firmware mode must use CdiSignature selector, got {other:?}"),
        }

        for slot_mode_id in ["connector-a", "connector-b"] {
            let mode = profile
                .configuration_modes
                .iter()
                .find(|mode| mode.id == slot_mode_id)
                .unwrap_or_else(|| panic!("{slot_mode_id} mode must be declared"));
            match &mode.selector {
                types::Selector::StructuralSlot {
                    slot_id,
                    affected_paths,
                    allow_none_installed,
                    ..
                } => {
                    assert_eq!(slot_id, slot_mode_id);
                    assert_eq!(
                        affected_paths.len(),
                        8,
                        "{slot_mode_id} must govern 8 Port I/O lines"
                    );
                    assert!(
                        *allow_none_installed,
                        "{slot_mode_id} must allow the empty selection"
                    );
                }
                other => panic!("{slot_mode_id} must use StructuralSlot selector, got {other:?}"),
            }
            let variant_ids: Vec<&str> = mode.variants.iter().map(|v| v.id.as_str()).collect();
            for expected in TOWER_LCC_SUPPORTED_DAUGHTERBOARDS {
                assert!(
                    variant_ids.contains(expected),
                    "{slot_mode_id} must offer daughterboard variant {expected}"
                );
            }
        }
    }

    /// Under the legacy Tower-LCC CDI, `build_connector_profile` derives the
    /// same `ConnectorProfile` shape v1 produced: two slots, 16 supported
    /// daughterboards each, the eight Port I/O lines split evenly, and BOD4
    /// carries exactly the four shared-library base validity rules.
    #[test]
    fn s5_tower_lcc_v2_parity_legacy_cdi() {
        let cdi = make_tower_lcc_legacy_cdi();
        let profile = load_bundled_tower_lcc_profile();
        let library = load_bundled_shared_daughterboards();

        let connector_profile = build_connector_profile(
            "05.02.01.02.03.00.00.01",
            &profile,
            Some(&library),
            &cdi,
        )
        .expect("Tower-LCC connector profile must build under legacy CDI");

        assert_eq!(connector_profile.carrier_key, "rr-cirkits::tower-lcc");
        assert_eq!(connector_profile.slots.len(), 2);
        assert_eq!(
            connector_profile.supported_daughterboards.len(),
            TOWER_LCC_SUPPORTED_DAUGHTERBOARDS.len()
        );

        for (slot_id, expected_line_lo) in [("connector-a", 1u32), ("connector-b", 9u32)] {
            let slot = connector_profile
                .slots
                .iter()
                .find(|candidate| candidate.slot_id == slot_id)
                .unwrap_or_else(|| panic!("{slot_id} slot must be present"));
            assert!(slot.allow_none_installed);
            assert_eq!(
                slot.supported_daughterboard_ids.len(),
                TOWER_LCC_SUPPORTED_DAUGHTERBOARDS.len(),
                "{slot_id} must offer 16 daughterboards"
            );
            assert_eq!(
                slot.affected_paths.len(),
                8,
                "{slot_id} must govern 8 Port I/O lines"
            );
            assert_eq!(
                slot.affected_paths[0],
                format!("Port I/O/Line#{expected_line_lo}"),
                "{slot_id} first line must be #{expected_line_lo}"
            );
            assert_eq!(
                slot.resolved_affected_paths.len(),
                8,
                "{slot_id} affected paths must all resolve against the CDI"
            );

            let bod4_constraints = slot
                .supported_daughterboard_constraints
                .iter()
                .find(|entry| entry.daughterboard_id == "BOD4")
                .expect("BOD4 entry must be present");
            assert_eq!(
                bod4_constraints.validity_rules.len(),
                5,
                "BOD4 under legacy CDI must apply the five shared base validity rules"
            );
        }
    }

    /// Under the C7 Tower-LCC CDI, the firmware-revision selector matches the
    /// `tower-lcc-c7` variant and `build_connector_profile` swaps each
    /// daughterboard's base validity rules for the C7-specific replacement
    /// set. BOD4 grows from 5 rules (legacy) to 7 (C7).
    #[test]
    fn s5_tower_lcc_v2_parity_c7_cdi() {
        let cdi = make_tower_lcc_c7_cdi();
        let profile = load_bundled_tower_lcc_profile();
        let library = load_bundled_shared_daughterboards();

        let connector_profile = build_connector_profile(
            "05.02.01.02.03.00.00.02",
            &profile,
            Some(&library),
            &cdi,
        )
        .expect("Tower-LCC connector profile must build under C7 CDI");

        let slot_a = connector_profile
            .slots
            .iter()
            .find(|candidate| candidate.slot_id == "connector-a")
            .expect("connector-a slot must be present");
        let bod4_constraints = slot_a
            .supported_daughterboard_constraints
            .iter()
            .find(|entry| entry.daughterboard_id == "BOD4")
            .expect("BOD4 entry must be present");
        assert_eq!(
            bod4_constraints.validity_rules.len(),
            7,
            "BOD4 under C7 CDI must apply the seven C7-specific validity rules"
        );
    }

    /// Regression guard: a real captured Tower-LCC CDI (legacy firmware) must
    /// match the bundled profile's `firmware-revision` signature and build a
    /// connector profile end-to-end. Synthetic CDIs in sibling tests can drift
    /// from real-hardware shape; this fixture detects that drift loudly.
    #[test]
    fn captured_legacy_tower_lcc_cdi_builds_connector_profile() {
        let cdi = parse_cdi(include_str!(
            "../../tests/fixtures/cdi/tower-lcc-legacy.xml"
        ))
        .expect("captured legacy Tower-LCC CDI must parse");
        let profile = load_bundled_tower_lcc_profile();
        let library = load_bundled_shared_daughterboards();

        let outcome = build_connector_profile_with_diagnostics(
            "05.01.01.01.6E.00.00.01",
            &profile,
            Some(&library),
            &cdi,
        );

        assert!(
            outcome.warning.is_none(),
            "captured legacy Tower-LCC CDI must match the firmware-revision signature; got warning: {:?}",
            outcome.warning
        );
        let connector_profile = outcome
            .profile
            .expect("captured legacy Tower-LCC CDI must build a connector profile");
        assert_eq!(connector_profile.carrier_key, "rr-cirkits::tower-lcc");
        assert_eq!(connector_profile.slots.len(), 2);
    }
}
