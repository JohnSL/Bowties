//! Offline change helpers — pure functions for managing change caches.

use crate::layout::offline_changes::OfflineChange;

/// Returns `true` when two offline changes target the same field on the same node.
///
/// Used for upsert-on-same-target semantics: when a new change targets a field
/// that already has a pending change, the existing entry is updated in place
/// rather than creating a duplicate.
pub fn same_change_target(a: &OfflineChange, b: &OfflineChange) -> bool {
    a.kind == b.kind
        && a.node_key == b.node_key
        && a.space == b.space
        && a.offset == b.offset
        && a.baseline_value == b.baseline_value
}

/// Remove offline changes whose IDs appear in `cleared_ids`.
///
/// Returns the number of changes removed.
pub fn remove_changes_by_id(
    cache: &mut Vec<OfflineChange>,
    cleared_ids: &std::collections::HashSet<String>,
) -> usize {
    let initial_len = cache.len();
    cache.retain(|change| !cleared_ids.contains(&change.change_id));
    initial_len.saturating_sub(cache.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::offline_changes::{OfflineChange, OfflineChangeKind, OfflineChangeStatus};
    use std::collections::HashSet;

    fn make_change(change_id: &str) -> OfflineChange {
        OfflineChange {
            change_id: change_id.to_string(),
            kind: OfflineChangeKind::Config,
            node_key: Some("050201020300".to_string()),
            space: Some(253),
            offset: Some("0x00000010".to_string()),
            baseline_value: "10".to_string(),
            planned_value: "20".to_string(),
            status: OfflineChangeStatus::Pending,
            error: None,
            updated_at: "2026-04-25T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn removes_only_cleared_change_ids_from_cache() {
        let mut cache = vec![make_change("row-1"), make_change("row-2"), make_change("row-3")];
        let cleared_ids = HashSet::from(["row-2".to_string(), "row-3".to_string()]);

        let removed = remove_changes_by_id(&mut cache, &cleared_ids);

        assert_eq!(removed, 2);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].change_id, "row-1");
    }

    #[test]
    fn does_not_change_cache_when_no_ids_match() {
        let mut cache = vec![make_change("row-1")];
        let cleared_ids = HashSet::from(["row-9".to_string()]);

        let removed = remove_changes_by_id(&mut cache, &cleared_ids);

        assert_eq!(removed, 0);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].change_id, "row-1");
    }

    #[test]
    fn same_change_target_matches_identical_targets() {
        let a = make_change("a");
        let b = make_change("b");
        assert!(same_change_target(&a, &b));
    }

    #[test]
    fn same_change_target_rejects_different_offset() {
        let a = make_change("a");
        let mut b = make_change("b");
        b.offset = Some("0x00000020".to_string());
        assert!(!same_change_target(&a, &b));
    }

    #[test]
    fn same_change_target_rejects_different_kind() {
        let a = make_change("a");
        let mut b = make_change("b");
        b.kind = OfflineChangeKind::BowtieMetadata;
        assert!(!same_change_target(&a, &b));
    }
}
