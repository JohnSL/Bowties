//! Profile path resolution
//!
//! Converts name-based profile group paths (e.g., `"Port I/O/Line/Event#1"`)
//! to index-based tree path prefixes (e.g., `["seg:1", "elem:2", "elem:5"]`)
//! that can be matched against [`crate::node_tree::GroupNode::path`].

use std::collections::{BTreeSet, HashMap};

use lcc_rs::cdi::{Cdi, DataElement};

use crate::profile::types::StructureProfile;

/// Maps a profile group-path string to a resolved index-based path prefix.
///
/// Key:   profile group path, e.g. `"Port I/O/Line/Event#1"`
/// Value: path prefix without instance suffixes, e.g. `["seg:1", "elem:0", "elem:3"]`
pub type ProfilePathMap = HashMap<String, Vec<String>>;

/// Deterministic set of reusable daughterboard IDs referenced by a profile.
pub type DaughterboardReferenceSet = Vec<String>;

/// Resolve all paths declared in a profile against the parsed CDI.
///
/// Paths that do not resolve produce a `eprintln!` warning and are excluded
/// from the returned map (so callers can gracefully skip them).
pub fn resolve_profile_paths(profile: &StructureProfile, cdi: &Cdi) -> ProfilePathMap {
    let mut map = HashMap::new();

    for decl in &profile.event_roles {
        match resolve_one_path(&decl.group_path, cdi) {
            Ok(path) => {
                map.insert(decl.group_path.clone(), path);
            }
            Err(e) => eprintln!(
                "[profile] Could not resolve event role path '{}': {}",
                decl.group_path, e
            ),
        }
    }

    for rule in &profile.relevance_rules {
        match resolve_one_path(&rule.affected_group_path, cdi) {
            Ok(path) => {
                map.insert(rule.affected_group_path.clone(), path);
            }
            Err(e) => eprintln!(
                "[profile] Could not resolve relevance rule path '{}' (rule {}): {}",
                rule.affected_group_path, rule.id, e
            ),
        }
    }

    for slot in &profile.connector_slots {
        for affected_path in &slot.affected_paths {
            match resolve_one_path(affected_path, cdi) {
                Ok(path) => {
                    map.insert(affected_path.clone(), path);
                }
                Err(e) => eprintln!(
                    "[profile] Could not resolve connector slot path '{}' (slot {}): {}",
                    affected_path, slot.slot_id, e
                ),
            }
        }
    }

    for override_rule in &profile.carrier_overrides {
        for validity_rule in &override_rule.override_validity_rules {
            match resolve_one_path(&validity_rule.target_path, cdi) {
                Ok(path) => {
                    map.insert(validity_rule.target_path.clone(), path);
                }
                Err(e) => eprintln!(
                    "[profile] Could not resolve override validity path '{}' (daughterboard {}): {}",
                    validity_rule.target_path, override_rule.daughterboard_id, e
                ),
            }
        }

        for repair_rule in &override_rule.override_repair_rules {
            match resolve_one_path(&repair_rule.target_path, cdi) {
                Ok(path) => {
                    map.insert(repair_rule.target_path.clone(), path);
                }
                Err(e) => eprintln!(
                    "[profile] Could not resolve override repair path '{}' (daughterboard {}): {}",
                    repair_rule.target_path, override_rule.daughterboard_id, e
                ),
            }
        }
    }

    map
}

pub fn referenced_daughterboard_ids(profile: &StructureProfile) -> DaughterboardReferenceSet {
    let mut ids = BTreeSet::new();

    for reference in &profile.daughterboard_references {
        ids.insert(reference.clone());
    }

    for slot in &profile.connector_slots {
        for daughterboard_id in &slot.supported_daughterboard_ids {
            ids.insert(daughterboard_id.clone());
        }
    }

    for override_rule in &profile.carrier_overrides {
        ids.insert(override_rule.daughterboard_id.clone());
    }

    ids.into_iter().collect()
}

/// Parse a name-based profile path and walk the CDI by name+ordinal to produce
/// an index-based path prefix (without instance suffixes).
///
/// ## Path format
/// `"Segment Name/Group Name[/#N]/..."`
/// - `#N` suffix (1-based) selects among same-named sibling groups
/// - Groups with unique names require no suffix (implicit `#1`)
/// - The first component must match a segment name
///
/// ## Name matching
/// Segment and group names may themselves contain `/` (e.g. `"Port I/O"`).
/// The resolver uses a **greedy longest-match** strategy: it tries the longest
/// possible prefix first so that a slash-in-name like `"Port I/O"` is correctly
/// recognised before the shorter `"Port I"` split.
fn resolve_one_path(profile_path: &str, cdi: &Cdi) -> Result<Vec<String>, String> {
    if profile_path.is_empty() {
        return Err("path is empty".to_string());
    }

    // ── Step 1: find the segment using greedy longest-prefix match ──────────
    let (seg_idx, mut remaining) = find_segment_prefix(profile_path, cdi).ok_or_else(|| {
        format!(
            "no segment name matches the start of path '{}'",
            profile_path
        )
    })?;

    let mut path = vec![format!("seg:{}", seg_idx)];
    let mut elements: &[DataElement] = &cdi.segments[seg_idx].elements;

    // ── Step 2: match groups one at a time from the remaining path ──────────
    while !remaining.is_empty() {
        let (elem_idx, group, next_remaining) =
            find_group_prefix(remaining, elements).ok_or_else(|| {
                format!(
                    "no group name matches the start of remaining path '{}'",
                    remaining
                )
            })?;
        path.push(format!("elem:{}", elem_idx));
        elements = &group.elements;
        remaining = next_remaining;
    }

    Ok(path)
}

/// Find the segment whose name is the **longest** prefix of `path` that is
/// followed by either `/` or end-of-string.
///
/// Returns `(segment_index, remaining_path_after_separator)` on success.
fn find_segment_prefix<'a>(path: &'a str, cdi: &Cdi) -> Option<(usize, &'a str)> {
    // Collect all candidate split positions (right after each '/' and at end).
    let mut split_positions: Vec<usize> =
        path.match_indices('/').map(|(i, _)| i).collect();
    split_positions.push(path.len());

    // Try longest candidate name first.
    for pos in split_positions.into_iter().rev() {
        let candidate_name = &path[..pos];
        if let Some(seg_idx) = cdi
            .segments
            .iter()
            .position(|s| s.name.as_deref().unwrap_or("") == candidate_name)
        {
            let remaining = if pos == path.len() {
                ""
            } else {
                &path[pos + 1..]
            };
            return Some((seg_idx, remaining));
        }
    }
    None
}

/// Find the group element whose name (plus optional `#N` ordinal suffix) is
/// the **longest** prefix of `remaining` that is followed by `/` or end.
///
/// Returns `(element_index, &Group, remaining_path_after_separator)`.
fn find_group_prefix<'a, 'e>(
    remaining: &'a str,
    elements: &'e [DataElement],
) -> Option<(usize, &'e lcc_rs::cdi::Group, &'a str)> {
    let mut split_positions: Vec<usize> =
        remaining.match_indices('/').map(|(i, _)| i).collect();
    split_positions.push(remaining.len());

    for pos in split_positions.into_iter().rev() {
        let candidate = &remaining[..pos];
        let next_remaining = if pos == remaining.len() {
            ""
        } else {
            &remaining[pos + 1..]
        };

        // ── Pass 1: exact name match (whole candidate is the group name) ────
        // This handles groups whose names contain '#', e.g. "Variable #1",
        // where the '#' is part of the actual name, not an ordinal suffix.
        for (i, elem) in elements.iter().enumerate() {
            if let DataElement::Group(g) = elem {
                if g.name.as_deref().unwrap_or("") == candidate {
                    return Some((i, g, next_remaining));
                }
            }
        }

        // ── Pass 2: base-name + ordinal (e.g. "Event#2" → 2nd "Event" group) ─
        // Only reached when no group has the exact candidate as its name.
        let (base_name, ordinal) = parse_name_ordinal(candidate);
        // Skip if parse_name_ordinal returned the whole string unchanged
        // (no '#' suffix) — that was already tried as exact match above.
        if base_name == candidate {
            continue;
        }
        let mut count = 0usize;
        for (i, elem) in elements.iter().enumerate() {
            if let DataElement::Group(g) = elem {
                if g.name.as_deref().unwrap_or("") == base_name {
                    count += 1;
                    if count == ordinal {
                        return Some((i, g, next_remaining));
                    }
                }
            }
        }
    }
    None
}

/// Parse a name component into `(base_name, ordinal)`.
///
/// - `"Event#2"` → `("Event", 2)`
/// - `"Event#1"` → `("Event", 1)`
/// - `"Event"`   → `("Event", 1)` (no suffix → first match)
fn parse_name_ordinal(s: &str) -> (&str, usize) {
    if let Some(hash_pos) = s.rfind('#') {
        let base = &s[..hash_pos];
        let ord: usize = s[hash_pos + 1..].parse().unwrap_or(1);
        (base, ord.max(1))
    } else {
        (s, 1)
    }
}

/// Strip instance suffixes from an index-based path, returning the
/// template-level path with `seg:N` and `elem:K` components.
///
/// Converts `elem:N#M` → `elem:N` so instance paths can be compared
/// against resolved profile path prefixes.
pub fn strip_instance_steps(path: &[String]) -> Vec<String> {
    path.iter()
        .map(|s| {
            // "elem:2#3" → "elem:2"
            if let Some(hash_pos) = s.find('#') {
                if s.starts_with("elem:") {
                    return s[..hash_pos].to_string();
                }
            }
            s.clone()
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Build a minimal CDI with the structure that matches Tower-LCC's
    /// Port I/O segment for basic resolver testing.
    fn make_test_cdi() -> Cdi {
        use lcc_rs::cdi::{EventIdElement, Group, IntElement, Segment};

        // Inner Event groups (two groups both named "Event")
        let event_group_1 = DataElement::Group(Group {
            name: Some("Event".to_string()),
            description: Some("Consumer event".to_string()),
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![DataElement::EventId(EventIdElement {
                name: Some("Event ID".to_string()),
                description: None,
                offset: 0,
            })],
            hints: None,
        });
        let event_group_2 = DataElement::Group(Group {
            name: Some("Event".to_string()),
            description: Some("Producer event".to_string()),
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![DataElement::EventId(EventIdElement {
                name: Some("Event ID".to_string()),
                description: None,
                offset: 0,
            })],
            hints: None,
        });

        // Line group (replicated, contains output fn + events)
        let output_fn = DataElement::Int(IntElement {
            name: Some("Output Function".to_string()),
            description: None,
            size: 1,
            offset: 0,
            min: None,
            max: None,
            default: None,
            map: None,
            hints: None,
        });
        let line_group = DataElement::Group(Group {
            name: Some("Line".to_string()),
            description: None,
            offset: 0,
            replication: 8,
            repname: vec!["Line".to_string()],
            elements: vec![output_fn, event_group_1, event_group_2],
            hints: None,
        });

        // Port I/O segment containing Line
        let segment = Segment {
            name: Some("Port I/O".to_string()),
            description: None,
            space: 253,
            origin: 0,
            elements: vec![line_group],
        };

        Cdi {
            identification: None,
            acdi: None,
            segments: vec![segment],
        }
    }

    #[test]
    fn resolve_profile_paths_basic() {
        let cdi = make_test_cdi();
        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: crate::profile::types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![crate::profile::types::EventRoleDecl {
                group_path: "Port I/O/Line/Event".to_string(),
                role: crate::profile::types::ProfileEventRole::Consumer,
                label: None,
            }],
            relevance_rules: vec![],
            connector_slots: vec![],
            daughterboard_references: vec![],
            carrier_overrides: vec![],
        };

        let map = resolve_profile_paths(&profile, &cdi);
        // "Port I/O/Line/Event" (== "#1") → seg:0, elem:0 (Line), elem:1 (first Event)
        let resolved = map.get("Port I/O/Line/Event").expect("path should resolve");
        assert_eq!(resolved, &["seg:0", "elem:0", "elem:1"]);
    }

    #[test]
    fn resolve_profile_paths_ordinal_suffix() {
        let cdi = make_test_cdi();

        let resolved_1 =
            resolve_one_path("Port I/O/Line/Event#1", &cdi).expect("should resolve");
        let resolved_2 =
            resolve_one_path("Port I/O/Line/Event#2", &cdi).expect("should resolve");

        // Event#1 = first "Event" group = elem:1 (after the Int at elem:0)
        assert_eq!(resolved_1, vec!["seg:0", "elem:0", "elem:1"]);
        // Event#2 = second "Event" group = elem:2
        assert_eq!(resolved_2, vec!["seg:0", "elem:0", "elem:2"]);
        // They must differ
        assert_ne!(resolved_1, resolved_2);
    }

    #[test]
    fn resolve_profile_paths_includes_connector_targets_and_overrides() {
        let cdi = make_test_cdi();
        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: crate::profile::types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            connector_slots: vec![crate::profile::types::ConnectorSlotDefinition {
                slot_id: "serial-a".to_string(),
                label: "Serial A".to_string(),
                order: 0,
                allow_none_installed: true,
                supported_daughterboard_ids: vec!["db-8in".to_string()],
                affected_paths: vec!["Port I/O/Line/Event#2".to_string()],
                base_behavior_when_empty: None,
            }],
            daughterboard_references: vec!["db-4io".to_string()],
            carrier_overrides: vec![crate::profile::types::CarrierOverrideRule {
                carrier_key: "test::test".to_string(),
                slot_id: Some("serial-a".to_string()),
                daughterboard_id: "db-8in".to_string(),
                override_validity_rules: vec![crate::profile::types::ConnectorConstraintRule {
                    target_path: "Port I/O/Line/Event#1".to_string(),
                    constraint_type: crate::profile::types::ConnectorConstraintType::HideSection,
                    allowed_values: vec![],
                    denied_values: vec![],
                    explanation: None,
                }],
                override_repair_rules: vec![crate::profile::types::RepairRule {
                    target_path: "Port I/O/Line/Event#2".to_string(),
                    replacement_strategy: crate::profile::types::RepairStrategy::ClearEmpty,
                    replacement_value: None,
                    priority: Some(1),
                }],
            }],
        };

        let map = resolve_profile_paths(&profile, &cdi);

        assert_eq!(map.get("Port I/O/Line/Event#1"), Some(&vec!["seg:0".to_string(), "elem:0".to_string(), "elem:1".to_string()]));
        assert_eq!(map.get("Port I/O/Line/Event#2"), Some(&vec!["seg:0".to_string(), "elem:0".to_string(), "elem:2".to_string()]));
    }

    #[test]
    fn referenced_daughterboard_ids_collects_all_profile_references() {
        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: crate::profile::types::ProfileNodeType {
                manufacturer: "Test".to_string(),
                model: "Test".to_string(),
            },
            firmware_version_range: None,
            event_roles: vec![],
            relevance_rules: vec![],
            connector_slots: vec![crate::profile::types::ConnectorSlotDefinition {
                slot_id: "serial-a".to_string(),
                label: "Serial A".to_string(),
                order: 0,
                allow_none_installed: true,
                supported_daughterboard_ids: vec!["db-8in".to_string(), "db-4io".to_string()],
                affected_paths: vec![],
                base_behavior_when_empty: None,
            }],
            daughterboard_references: vec!["db-relay".to_string(), "db-8in".to_string()],
            carrier_overrides: vec![crate::profile::types::CarrierOverrideRule {
                carrier_key: "test::test".to_string(),
                slot_id: None,
                daughterboard_id: "db-relay".to_string(),
                override_validity_rules: vec![],
                override_repair_rules: vec![],
            }],
        };

        assert_eq!(
            referenced_daughterboard_ids(&profile),
            vec!["db-4io".to_string(), "db-8in".to_string(), "db-relay".to_string()]
        );
    }

    // ── parse_name_ordinal ────────────────────────────────────────────────────

    #[test]
    fn parse_name_ordinal_no_suffix_gives_ordinal_one() {
        let (base, ord) = parse_name_ordinal("Event");
        assert_eq!(base, "Event");
        assert_eq!(ord, 1);
    }

    #[test]
    fn parse_name_ordinal_hash_two() {
        let (base, ord) = parse_name_ordinal("Event#2");
        assert_eq!(base, "Event");
        assert_eq!(ord, 2);
    }

    #[test]
    fn parse_name_ordinal_invalid_suffix_defaults_to_one() {
        // Non-numeric suffix → parse fails → ordinal defaults to 1 (max(1, 0) = 1)
        let (base, ord) = parse_name_ordinal("Event#x");
        assert_eq!(base, "Event");
        assert_eq!(ord, 1);
    }

    // ── strip_instance_steps ──────────────────────────────────────────────────

    #[test]
    fn strip_instance_steps_removes_inst_steps() {
        let path = vec![
            "seg:0".to_string(),
            "elem:0#3".to_string(),
        ];
        let stripped = strip_instance_steps(&path);
        assert_eq!(stripped, vec!["seg:0", "elem:0"]);
    }

    #[test]
    fn strip_instance_steps_preserves_seg_and_elem() {
        let path = vec![
            "seg:1".to_string(),
            "elem:2#1".to_string(),
            "elem:5".to_string(),
        ];
        let stripped = strip_instance_steps(&path);
        assert_eq!(stripped, vec!["seg:1", "elem:2", "elem:5"]);
    }

    // ── find_segment_prefix ────────────────────────────────────────────────────

    #[test]
    fn find_segment_prefix_simple_name_match() {
        let cdi = make_test_cdi();
        // "Port I/O" is the segment name; succeeds with greedy match
        let result = find_segment_prefix("Port I/O/Line/Event", &cdi);
        let (seg_idx, remaining) = result.expect("Should match 'Port I/O'");
        assert_eq!(seg_idx, 0);
        assert_eq!(remaining, "Line/Event");
    }

    #[test]
    fn find_segment_prefix_slash_in_name_greedy_match() {
        // The segment is named "Port I/O" which contains a slash.
        // A short-match strategy would incorrectly match "Port I" if there were such a segment;
        // the greedy strategy must prefer the longer name.
        let cdi = make_test_cdi();
        let result = find_segment_prefix("Port I/O", &cdi);
        let (seg_idx, remaining) = result.expect("Should match 'Port I/O'");
        assert_eq!(seg_idx, 0);
        assert_eq!(remaining, ""); // consumed the whole path
    }

    #[test]
    fn find_segment_prefix_no_match_returns_none() {
        let cdi = make_test_cdi();
        let result = find_segment_prefix("NonExistentSegment/Foo", &cdi);
        assert!(result.is_none());
    }

    // ── resolve_one_path ──────────────────────────────────────────────────────

    #[test]
    fn resolve_one_path_segment_not_found_returns_err() {
        let cdi = make_test_cdi();
        let result = resolve_one_path("NoSuchSegment/Line", &cdi);
        assert!(result.is_err(), "Unknown segment must return Err");
    }

    #[test]
    fn resolve_one_path_group_not_found_returns_err() {
        let cdi = make_test_cdi();
        let result = resolve_one_path("Port I/O/NoSuchGroup", &cdi);
        assert!(result.is_err(), "Unknown group must return Err");
    }

    #[test]
    fn resolve_one_path_empty_returns_err() {
        let cdi = make_test_cdi();
        let result = resolve_one_path("", &cdi);
        assert!(result.is_err(), "Empty path must return Err");
    }
}
