//! Snapshot builder — pure domain logic for layout capture.
//!
//! Extracts the tree-walking and snapshot assembly logic from
//! `app/src-tauri/src/commands/layout_capture.rs` so it can be unit-tested
//! with `cargo test` (no Tauri DLL dependency).
//!
//! The thin command layer in src-tauri fetches proxy data (SNIP, CDI, config
//! tree, PIP flags) and passes it here as pre-fetched structs.

use std::collections::BTreeMap;

use crate::layout::node_snapshot::{
    capture_status_from_missing, missing_detail, CaptureStatus, CdiReference, NodeSnapshot,
    NodeSnapshotLifecycle, SnapshotLeafValue, SnipSnapshot,
};
use crate::node_tree::{ConfigNode, ConfigValue, GroupNode};

// ─────────────────────────────────────────────────────────────────────────────
// Pre-fetched input types
// ─────────────────────────────────────────────────────────────────────────────

/// Pre-fetched data from a live or synthesized node proxy, sufficient to build
/// a `NodeSnapshot` without any async or Tauri dependency.
pub struct ProxySnapshotData {
    /// True when the proxy is a `Synthesized` variant (placeholder).
    pub is_synthesized: bool,
    /// For synthesized nodes: the placeholder node_key (e.g. `"placeholder:<uuid>"`).
    /// For live nodes: `None` (derived from `node_id`).
    pub synthesized_node_key: Option<String>,
    /// For synthesized nodes: the profile stem. For live nodes: `None`.
    pub profile_stem: Option<String>,
    /// The `DiscoveredNode` snapshot from the proxy.
    pub node_id: Option<lcc_rs::NodeID>,
    /// SNIP data from the proxy (may be None).
    pub snip_data: Option<lcc_rs::SNIPData>,
    /// CDI XML content length (for fingerprinting).
    pub cdi_xml_len: Option<usize>,
    /// PIP status and flags.
    pub pip_status: lcc_rs::PIPStatus,
    pub pip_cdi_flag: bool,
    /// Config tree from the proxy (None if not yet built).
    pub config_tree: Option<crate::node_tree::NodeConfigTree>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree-walking helpers
// ─────────────────────────────────────────────────────────────────────────────

fn config_value_to_string(value: &ConfigValue) -> String {
    value.to_snapshot_string()
}

/// Determine the hierarchy key for a group node.
///
/// Replication wrapper groups (instance=0) are structural only and produce `None`.
/// Replicated instances produce `"Name(index)"`, others just use the group name.
pub fn group_key(group: &GroupNode) -> Option<String> {
    // Replication wrapper groups (instance=0) are structural only.
    if group.instance == 0 && group.replication_count > 1 {
        return None;
    }

    if group.replication_count > 1 && group.instance > 0 {
        return Some(format!("{}({})", group.name, group.instance - 1));
    }

    Some(group.name.clone())
}

/// Recursively walk config tree nodes, collecting leaf values into the snapshot
/// and recording missing values.
///
/// `hierarchy` tracks the current named path. The caller seeds it with the
/// segment name before calling.
pub fn collect_leaf_values(
    nodes: &[ConfigNode],
    hierarchy: &mut Vec<String>,
    snapshot: &mut NodeSnapshot,
    missing: &mut Vec<String>,
    node_id_for_log: &str,
) -> Vec<String> {
    let mut log_messages = Vec::new();
    for node in nodes {
        match node {
            ConfigNode::Leaf(leaf) => {
                let offset_key = format!("0x{:08X}", leaf.address);
                if let Some(v) = &leaf.value {
                    let value = config_value_to_string(v);
                    let mut named_path = hierarchy.clone();
                    named_path.push(leaf.name.clone());
                    snapshot.add_config_leaf(
                        &named_path,
                        SnapshotLeafValue {
                            value,
                            space: Some(leaf.space),
                            offset: Some(offset_key),
                        },
                    );
                } else {
                    log_messages.push(format!(
                        "[layout capture] missing value: node={} leaf={} hierarchy={} cdi_path={} space={} offset={} type={:?}",
                        node_id_for_log,
                        leaf.name,
                        hierarchy.join(" / "),
                        leaf.path.join("/"),
                        leaf.space,
                        offset_key,
                        leaf.element_type,
                    ));
                    missing.push(missing_detail(leaf.space, &offset_key, &leaf.path));
                }
            }
            ConfigNode::Group(group) => {
                let mut pushed = false;
                if let Some(gk) = group_key(group) {
                    hierarchy.push(gk);
                    pushed = true;
                }
                let child_logs = collect_leaf_values(
                    &group.children,
                    hierarchy,
                    snapshot,
                    missing,
                    node_id_for_log,
                );
                log_messages.extend(child_logs);
                if pushed {
                    let _ = hierarchy.pop();
                }
            }
        }
    }
    log_messages
}

/// Build a `NodeSnapshot` from pre-fetched proxy data.
///
/// This is the pure core of the snapshot builder. The caller is responsible for
/// fetching proxy data (via `NodeProxyHandle`) and passing it in. Returns the
/// completed snapshot and any log messages that would have been emitted.
pub fn build_node_snapshot(
    data: &ProxySnapshotData,
    captured_at: &str,
    producer_events: Vec<String>,
) -> Result<(NodeSnapshot, Vec<String>), String> {
    let mut log_messages = Vec::new();

    let cdi_fingerprint = if let Some(len) = data.cdi_xml_len {
        format!("len:{}", len)
    } else if data.pip_status == lcc_rs::PIPStatus::Complete && !data.pip_cdi_flag {
        "not_supported".to_string()
    } else {
        "missing".to_string()
    };

    let version = data
        .snip_data
        .as_ref()
        .map(|s| s.software_version.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let snip = if let Some(snip) = &data.snip_data {
        SnipSnapshot {
            user_name: snip.user_name.clone(),
            user_description: snip.user_description.clone(),
            manufacturer_name: snip.manufacturer.clone(),
            model_name: snip.model.clone(),
        }
    } else {
        SnipSnapshot::default()
    };

    // S9: Synthesized placeholders use CdiReference::from_profile_stem so
    // the save flow resolves CDI from bundled profiles, not cdi_cache.
    let (cdi_ref, node_key, node_id, profile_stem) = if data.is_synthesized {
        let synth_key = data
            .synthesized_node_key
            .as_deref()
            .ok_or("Synthesized proxy missing node_key")?;
        let stem = data
            .profile_stem
            .as_deref()
            .ok_or("Synthesized proxy missing profile_stem")?;
        (
            CdiReference::from_profile_stem(stem),
            synth_key.to_string(),
            None,
            Some(stem.to_string()),
        )
    } else {
        let nid = data.node_id.ok_or("Live proxy missing node_id")?;
        // S8.6: single mint formula via `CdiReference::from_snip`. For nodes
        // without SNIP we keep the historical `"unknown_node_type"` cache_key.
        let cdi_ref = if snip.manufacturer_name.is_empty() && snip.model_name.is_empty() {
            CdiReference {
                cache_key: "unknown_node_type".to_string(),
                version: version.clone(),
                fingerprint: cdi_fingerprint,
            }
        } else {
            CdiReference::from_snip(&snip, version, cdi_fingerprint)
        };
        (cdi_ref, nid.to_canonical(), Some(nid), None)
    };

    let tree_segment_count = data
        .config_tree
        .as_ref()
        .map(|t| t.segments.len())
        .unwrap_or(0);
    log_messages.push(format!(
        "[layout capture] snapshot start: node={} manufacturer={} model={} tree_available={} segments={}",
        node_key,
        snip.manufacturer_name,
        snip.model_name,
        data.config_tree.is_some(),
        tree_segment_count,
    ));

    let mut snapshot = NodeSnapshot {
        node_key: node_key.clone(),
        node_id,
        profile_stem,
        lifecycle: NodeSnapshotLifecycle::Persisted,
        captured_at: captured_at.to_string(),
        capture_status: CaptureStatus::Complete,
        missing: Vec::new(),
        snip,
        cdi_ref,
        config: BTreeMap::new(),
        producer_identified_events: producer_events,
    };

    let mut missing = Vec::new();
    if let Some(tree) = &data.config_tree {
        for segment in &tree.segments {
            let mut hierarchy = vec![segment.name.clone()];
            let seg_logs = collect_leaf_values(
                &segment.children,
                &mut hierarchy,
                &mut snapshot,
                &mut missing,
                &node_key,
            );
            log_messages.extend(seg_logs);
        }
    } else {
        log_messages.push(format!(
            "[layout capture] missing configuration tree: node={} manufacturer={} model={}",
            snapshot.node_key, snapshot.snip.manufacturer_name, snapshot.snip.model_name,
        ));
        missing.push("configuration tree not available".to_string());
    }

    snapshot.missing = missing;
    snapshot.capture_status = capture_status_from_missing(&snapshot.missing);
    log_messages.push(format!(
        "[layout capture] snapshot complete: node={} status={:?} missing_count={}",
        snapshot.node_key, snapshot.capture_status, snapshot.missing.len(),
    ));

    Ok((snapshot, log_messages))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_tree::{LeafNode, LeafType, NodeConfigTree, SegmentNode};

    fn make_live_proxy_data(tree: Option<NodeConfigTree>) -> ProxySnapshotData {
        ProxySnapshotData {
            is_synthesized: false,
            synthesized_node_key: None,
            profile_stem: None,
            node_id: Some(lcc_rs::NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9])),
            snip_data: Some(lcc_rs::SNIPData {
                manufacturer: "Test Mfg".to_string(),
                model: "Test Model".to_string(),
                hardware_version: String::new(),
                software_version: "1.0".to_string(),
                user_name: "My Node".to_string(),
                user_description: String::new(),
            }),
            cdi_xml_len: Some(500),
            pip_status: lcc_rs::PIPStatus::Complete,
            pip_cdi_flag: true,
            config_tree: tree,
        }
    }

    fn make_synthesized_proxy_data(tree: Option<NodeConfigTree>) -> ProxySnapshotData {
        ProxySnapshotData {
            is_synthesized: true,
            synthesized_node_key: Some("placeholder:test-uuid".to_string()),
            profile_stem: Some("Mustangpeak-Engineering_TurnoutBoss".to_string()),
            node_id: None,
            snip_data: Some(lcc_rs::SNIPData {
                manufacturer: "Mustangpeak Engineering".to_string(),
                model: "TurnoutBoss".to_string(),
                hardware_version: String::new(),
                software_version: "1.0".to_string(),
                user_name: String::new(),
                user_description: String::new(),
            }),
            cdi_xml_len: Some(100),
            pip_status: lcc_rs::PIPStatus::Complete,
            pip_cdi_flag: true,
            config_tree: tree,
        }
    }

    fn make_tree_with_leaf(value: Option<ConfigValue>) -> NodeConfigTree {
        NodeConfigTree {
            node_id: "02.01.57.00.02.D9".into(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            profile_applied: false,
            unknown_variants: Vec::new(),
            segments: vec![SegmentNode {
                name: "Config".into(),
                description: None,
                origin: 0,
                space: 0xFD,
                children: vec![ConfigNode::Leaf(LeafNode {
                    name: "Event ID".into(),
                    description: None,
                    element_type: LeafType::EventId,
                    address: 0,
                    size: 8,
                    space: 0xFD,
                    path: vec!["seg:0".into(), "elem:0".into()],
                    value,
                    event_role: None,
                    constraints: None,
                    button_text: None,
                    dialog_text: None,
                    action_value: 0,
                    hint_slider: None,
                    hint_radio: false,
                    modified_value: None,
                    write_state: None,
                    write_error: None,
                    read_only: false,
                })],
            }],
        }
    }

    #[test]
    fn group_key_wrapper_returns_none() {
        let group = GroupNode {
            name: "Line".into(),
            has_name: true,
            description: None,
            instance: 0,
            instance_label: String::new(),
            replication_of: "Line".into(),
            replication_count: 4,
            path: vec![],
            children: vec![],
            display_name: None,
            hideable: false,
            hidden_by_default: false,
            read_only: false,
        };
        assert_eq!(group_key(&group), None);
    }

    #[test]
    fn group_key_replicated_instance() {
        let group = GroupNode {
            name: "Line".into(),
            has_name: true,
            description: None,
            instance: 3,
            instance_label: String::new(),
            replication_of: "Line".into(),
            replication_count: 4,
            path: vec![],
            children: vec![],
            display_name: None,
            hideable: false,
            hidden_by_default: false,
            read_only: false,
        };
        assert_eq!(group_key(&group), Some("Line(2)".to_string()));
    }

    #[test]
    fn group_key_singleton() {
        let group = GroupNode {
            name: "Settings".into(),
            has_name: true,
            description: None,
            instance: 1,
            instance_label: String::new(),
            replication_of: "Settings".into(),
            replication_count: 1,
            path: vec![],
            children: vec![],
            display_name: None,
            hideable: false,
            hidden_by_default: false,
            read_only: false,
        };
        assert_eq!(group_key(&group), Some("Settings".to_string()));
    }

    #[test]
    fn build_snapshot_placeholder_uses_bundled_cdi_ref() {
        let data = make_synthesized_proxy_data(None);
        let (snap, _logs) =
            build_node_snapshot(&data, "2026-05-31T00:00:00Z", vec![]).unwrap();

        assert_eq!(snap.node_key, "placeholder:test-uuid");
        assert!(snap.node_id.is_none());
        assert_eq!(snap.profile_stem.as_deref(), Some("Mustangpeak-Engineering_TurnoutBoss"));
        assert!(snap.cdi_ref.is_bundled());
        assert_eq!(snap.cdi_ref.cache_key, "Mustangpeak-Engineering_TurnoutBoss");
    }

    #[test]
    fn populated_tree_yields_complete_capture() {
        let tree = make_tree_with_leaf(Some(ConfigValue::EventId {
            bytes: [1, 2, 3, 4, 5, 6, 7, 8],
            hex: "01.02.03.04.05.06.07.08".into(),
        }));
        let data = make_live_proxy_data(Some(tree));
        let (snap, _logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", vec![]).unwrap();

        assert!(snap.missing.is_empty());
        assert_eq!(snap.capture_status, CaptureStatus::Complete);
        assert_eq!(snap.config.len(), 1);
    }

    #[test]
    fn missing_value_yields_partial_capture() {
        let tree = make_tree_with_leaf(None);
        let data = make_live_proxy_data(Some(tree));
        let (snap, logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", vec![]).unwrap();

        assert_eq!(snap.capture_status, CaptureStatus::Partial);
        assert!(!snap.missing.is_empty());
        assert!(logs.iter().any(|l| l.contains("missing value")));
    }

    #[test]
    fn no_tree_yields_partial_capture() {
        let data = make_live_proxy_data(None);
        let (snap, logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", vec![]).unwrap();

        assert_eq!(snap.capture_status, CaptureStatus::Partial);
        assert!(snap.missing.contains(&"configuration tree not available".to_string()));
        assert!(logs.iter().any(|l| l.contains("missing configuration tree")));
    }

    #[test]
    fn live_node_without_snip_uses_unknown_cache_key() {
        let mut data = make_live_proxy_data(None);
        data.snip_data = None;
        let (snap, _logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", vec![]).unwrap();

        assert_eq!(snap.cdi_ref.cache_key, "unknown_node_type");
    }

    #[test]
    fn live_node_cdi_fingerprint_not_supported() {
        let mut data = make_live_proxy_data(None);
        data.cdi_xml_len = None;
        data.pip_status = lcc_rs::PIPStatus::Complete;
        data.pip_cdi_flag = false;
        let (snap, _logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", vec![]).unwrap();

        assert_eq!(snap.cdi_ref.fingerprint, "not_supported");
    }

    #[test]
    fn producer_events_propagated() {
        let data = make_live_proxy_data(None);
        let events = vec!["01.02.03.04.05.06.07.08".to_string()];
        let (snap, _logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", events.clone()).unwrap();

        assert_eq!(snap.producer_identified_events, events);
    }

    // ADR-0015 contract: the src-tauri `proxy_snapshot_data` helper
    // composes `proxy.cdi.is_none()` with `LayoutState::cdi_xml(key)` to
    // fill `cdi_xml_len`. The structural rule that makes that fix work is
    // asserted here: `cdi_xml_len: Some` ⇒ fingerprint never collapses to
    // `"missing"`. Without that, the legacy `.retain(fingerprint != "missing")`
    // silently dropped saved nodes from disk when their proxy didn't
    // currently hold CDI.
    #[test]
    fn cdi_xml_len_some_produces_len_fingerprint_not_missing() {
        let mut data = make_live_proxy_data(None);
        data.cdi_xml_len = Some(1234);
        // Even when PIP hasn't completed, a known XML length must win
        // over a "missing" classification.
        data.pip_status = lcc_rs::PIPStatus::Unknown;
        data.pip_cdi_flag = false;
        let (snap, _logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", vec![]).unwrap();

        assert_eq!(snap.cdi_ref.fingerprint, "len:1234");
        assert_ne!(snap.cdi_ref.fingerprint, "missing");
    }

    #[test]
    fn cdi_xml_len_none_with_unknown_pip_falls_through_to_missing() {
        let mut data = make_live_proxy_data(None);
        data.cdi_xml_len = None;
        data.pip_status = lcc_rs::PIPStatus::Unknown;
        data.pip_cdi_flag = false;
        let (snap, _logs) =
            build_node_snapshot(&data, "2026-06-01T00:00:00Z", vec![]).unwrap();

        // The truly-no-data case is the only path that should ever
        // produce `"missing"` after slice 2 lands. Save-time logging in
        // `save_layout_directory` flags this and does not persist.
        assert_eq!(snap.cdi_ref.fingerprint, "missing");
    }
}
