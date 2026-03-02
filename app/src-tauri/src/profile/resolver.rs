//! Profile path resolution
//!
//! Converts name-based profile group paths (e.g., `"Port I/O/Line/Event#1"`)
//! to index-based tree path prefixes (e.g., `["seg:1", "elem:2", "elem:5"]`)
//! that can be matched against [`crate::node_tree::GroupNode::path`].

use std::collections::HashMap;

use lcc_rs::cdi::{Cdi, DataElement};

use crate::profile::types::StructureProfile;

/// Maps a profile group-path string to a resolved index-based path prefix.
///
/// Key:   profile group path, e.g. `"Port I/O/Line/Event#1"`
/// Value: path prefix without `inst:M` steps, e.g. `["seg:1", "elem:0", "elem:3"]`
pub type ProfilePathMap = HashMap<String, Vec<String>>;

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

    map
}

/// Parse a name-based profile path and walk the CDI by name+ordinal to produce
/// an index-based path prefix (without `inst:M` steps).
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

/// Strip `inst:M` steps from an index-based path, returning only `seg:N` and
/// `elem:K` components.
///
/// Used during tree annotation to compare a GroupNode's actual path (which
/// includes instance steps) against a resolved profile path prefix.
pub fn strip_instance_steps(path: &[String]) -> Vec<String> {
    path.iter()
        .filter(|s| !s.starts_with("inst:"))
        .cloned()
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
            }],
            relevance_rules: vec![],
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
}
