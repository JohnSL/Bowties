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

use crate::node_tree::{ConfigNode, LeafType, NodeConfigTree};

pub use types::{
    StructureProfile,
    ProfileNodeType,
    FirmwareVersionRange,
    EventRoleDecl,
    ProfileEventRole,
    RelevanceRule,
    RelevanceCondition,
    RelevanceAnnotation,
};
pub use loader::load_profile;
pub use resolver::{ProfilePathMap, resolve_profile_paths};

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
/// `inst:N` steps stripped) equals `resolved_path`.
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
}
