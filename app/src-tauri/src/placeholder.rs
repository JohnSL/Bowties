//! Placeholder factory — owns creation of synthesized placeholder nodes.
//!
//! To "Add Placeholder" as bus discovery is to "Node Appeared": the factory
//! synthesizes a fully-valid in-memory state holder and inserts it into the
//! same registry that live nodes use.  No other module knows the conventions
//! (UUID key minting, bundled CDI resolution, all-zero EventId synthesis).
//!
//! Spec 014 / S8.10.

use std::collections::HashMap;

use lcc_rs::cdi::{Cdi, DataElement};
use lcc_rs::types::{CdiData, SNIPData};

use crate::node_proxy::SynthesizedNodeProxy;
use crate::node_tree::NodeConfigTree;
use crate::state::AppState;

/// Synthesize a placeholder node from a bundled profile stem.
///
/// Mints a `placeholder:<uuid>` key, loads the bundled CDI, walks the CDI
/// for EventId leaves (pre-populating `[0u8; 8]`), builds the config tree
/// with profile metadata applied, and returns the complete
/// `SynthesizedNodeProxy` ready for registry insertion.
pub async fn synthesize(
    profile_stem: &str,
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<(String, SynthesizedNodeProxy), String> {
    let node_key = format!("placeholder:{}", uuid::Uuid::new_v4());

    // ── Load bundled CDI (raw XML + parsed) ──────────────────────────────
    let dirs = crate::commands::cdi::bundled_cdi_search_dirs(app_handle);
    let (xml, cdi) = load_bundled_cdi_with_xml(&dirs, profile_stem)?;

    // ── Walk CDI for EventId leaves → all-zero bytes ─────────────────────
    let config_values = collect_eventid_zeros(&cdi);

    // ── Resolve manufacturer / model from profile listing ────────────────
    let profiles = crate::profile::loader::list_bundled_profiles(app_handle);
    let summary = profiles
        .iter()
        .find(|p| p.stem == profile_stem)
        .ok_or_else(|| {
            format!("UnknownProfile: '{profile_stem}' not in bundled profile listing")
        })?;

    let snip = SNIPData {
        manufacturer: summary.manufacturer.clone(),
        model: summary.model.clone(),
        hardware_version: String::new(),
        software_version: String::new(),
        user_name: String::new(),
        user_description: String::new(),
    };

    // ── Build config tree + profile overlay ───────────────────────────────
    let mut tree = build_tree_with_profile(&node_key, &cdi, app_handle, state).await;
    merge_config_values_into_tree(&mut tree, &config_values);
    populate_leaf_defaults_in_tree(&mut tree);

    let cdi_data = CdiData {
        xml_content: xml,
        retrieved_at: chrono::Utc::now(),
    };

    let proxy = SynthesizedNodeProxy {
        node_key: node_key.clone(),
        profile_stem: profile_stem.to_string(),
        snip: Some(snip),
        cdi_data: Some(cdi_data),
        cdi_parsed: Some(cdi),
        config_values,
        config_tree: Some(tree),
        producer_identified_events: Vec::new(),
    };

    Ok((node_key, proxy))
}

/// Reconstitute a `SynthesizedNodeProxy` from a saved placeholder's known
/// key and profile stem.
///
/// Same pipeline as `synthesize` but skips UUID minting — the caller already
/// knows the `node_key` (e.g. from a persisted `NodeSnapshot`).  Used by
/// `get_node_tree` to lazily populate the registry for saved placeholders
/// that weren't factory-minted this session.
pub async fn reconstitute(
    node_key: &str,
    profile_stem: &str,
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<SynthesizedNodeProxy, String> {
    let dirs = crate::commands::cdi::bundled_cdi_search_dirs(app_handle);
    let (xml, cdi) = load_bundled_cdi_with_xml(&dirs, profile_stem)?;
    let config_values = collect_eventid_zeros(&cdi);

    let profiles = crate::profile::loader::list_bundled_profiles(app_handle);
    let summary = profiles
        .iter()
        .find(|p| p.stem == profile_stem)
        .ok_or_else(|| {
            format!("UnknownProfile: '{profile_stem}' not in bundled profile listing")
        })?;

    let snip = SNIPData {
        manufacturer: summary.manufacturer.clone(),
        model: summary.model.clone(),
        hardware_version: String::new(),
        software_version: String::new(),
        user_name: String::new(),
        user_description: String::new(),
    };

    let mut tree = build_tree_with_profile(node_key, &cdi, app_handle, state).await;
    merge_config_values_into_tree(&mut tree, &config_values);
    populate_leaf_defaults_in_tree(&mut tree);

    let cdi_data = CdiData {
        xml_content: xml,
        retrieved_at: chrono::Utc::now(),
    };

    Ok(SynthesizedNodeProxy {
        node_key: node_key.to_string(),
        profile_stem: profile_stem.to_string(),
        snip: Some(snip),
        cdi_data: Some(cdi_data),
        cdi_parsed: Some(cdi),
        config_values,
        config_tree: Some(tree),
        producer_identified_events: Vec::new(),
    })
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Load a bundled CDI, returning both the raw XML string and the parsed `Cdi`.
fn load_bundled_cdi_with_xml(
    search_dirs: &[std::path::PathBuf],
    profile_stem: &str,
) -> Result<(String, Cdi), String> {
    let file_name = format!("{profile_stem}.cdi.xml");
    let path = search_dirs
        .iter()
        .map(|dir| dir.join(&file_name))
        .find(|candidate| candidate.exists())
        .ok_or_else(|| {
            format!(
                "BundledCdiNotFound: '{file_name}' not present in any bundled-profiles directory"
            )
        })?;

    let xml = std::fs::read_to_string(&path)
        .map_err(|e| format!("BundledCdiReadFailed: '{}': {e}", path.display()))?;

    let cdi =
        lcc_rs::cdi::parser::parse_cdi(&xml).map_err(|e| format!("InvalidXml: {e}"))?;

    Ok((xml, cdi))
}

/// Walk the CDI tree and collect the path of every EventId leaf, mapping
/// each to `[0u8; 8]`.  Path format matches `node_tree::build_node_config_tree`
/// (e.g. `seg:0/elem:3` or `seg:0/elem:2#1/elem:0`).
fn collect_eventid_zeros(cdi: &Cdi) -> HashMap<String, [u8; 8]> {
    let mut out = HashMap::new();
    for (seg_idx, segment) in cdi.segments.iter().enumerate() {
        let prefix = format!("seg:{seg_idx}");
        collect_eventids_in_elements(&segment.elements, &prefix, &mut out);
    }
    out
}

fn collect_eventids_in_elements(
    elements: &[DataElement],
    parent_path: &str,
    out: &mut HashMap<String, [u8; 8]>,
) {
    for (i, element) in elements.iter().enumerate() {
        match element {
            DataElement::EventId(_) => {
                let path = format!("{parent_path}/elem:{i}");
                out.insert(path, [0u8; 8]);
            }
            DataElement::Group(g) => {
                let effective_replication = if g.calculate_size() == 0 && g.replication > 1 {
                    1u32
                } else {
                    g.replication
                };

                if effective_replication > 1 {
                    for instance in 0..effective_replication {
                        let inst_num = instance + 1;
                        let child_path = format!("{parent_path}/elem:{i}#{inst_num}");
                        collect_eventids_in_elements(&g.elements, &child_path, out);
                    }
                } else {
                    let child_path = format!("{parent_path}/elem:{i}");
                    collect_eventids_in_elements(&g.elements, &child_path, out);
                }
            }
            // Int, String, Float, Action, Blob — no EventId slots.
            _ => {}
        }
    }
}

/// Merge pre-populated config values (EventId zeros) into a config tree's
/// leaf nodes.  Each key in `config_values` is a CDI path string
/// (e.g. `"seg:0/elem:1"`) and each value is `[u8; 8]` (raw EventId bytes).
///
/// This bridges the gap between `collect_eventid_zeros` (which returns a
/// path-keyed HashMap) and the config tree (which stores values per-leaf).
/// Without this merge, `build_node_snapshot` reports all EventId leaves as
/// "missing" since their `leaf.value` is `None`.
pub(crate) fn merge_config_values_into_tree(
    tree: &mut NodeConfigTree,
    config_values: &HashMap<String, [u8; 8]>,
) {
    for segment in &mut tree.segments {
        merge_leaves_recursive(&mut segment.children, config_values);
    }
}

fn merge_leaves_recursive(
    nodes: &mut [crate::node_tree::ConfigNode],
    config_values: &HashMap<String, [u8; 8]>,
) {
    for node in nodes.iter_mut() {
        match node {
            crate::node_tree::ConfigNode::Leaf(leaf) => {
                let path_key = leaf.path.join("/");
                if let Some(bytes) = config_values.get(&path_key) {
                    let hex = bytes
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(".");
                    leaf.value = Some(crate::node_tree::ConfigValue::EventId {
                        bytes: *bytes,
                        hex,
                    });
                }
            }
            crate::node_tree::ConfigNode::Group(group) => {
                merge_leaves_recursive(&mut group.children, config_values);
            }
        }
    }
}

/// Populate every Int/String/Float leaf whose `value` is still `None` with
/// a typed default — CDI-declared `default` where present, otherwise the
/// type's zero. EventIds are handled by `merge_config_values_into_tree`;
/// Action and Blob have no `ConfigValue` variant and are left as-is (this
/// mirrors live-node capture behaviour).
///
/// After this runs, a placeholder's config tree looks like a fully-captured
/// node — `build_node_snapshot` reports `CaptureStatus::Complete` and no
/// "missing value" entries, so reopened layouts don't surface a misleading
/// "values were not captured" banner.
pub(crate) fn populate_leaf_defaults_in_tree(tree: &mut NodeConfigTree) {
    for segment in &mut tree.segments {
        populate_defaults_recursive(&mut segment.children);
    }
}

fn populate_defaults_recursive(nodes: &mut [crate::node_tree::ConfigNode]) {
    use crate::node_tree::{ConfigNode, ConfigValue, LeafType};
    for node in nodes.iter_mut() {
        match node {
            ConfigNode::Leaf(leaf) => {
                if leaf.value.is_some() {
                    continue;
                }
                let declared = leaf
                    .constraints
                    .as_ref()
                    .and_then(|c| c.default_value.as_deref());
                leaf.value = match leaf.element_type {
                    LeafType::Int => Some(ConfigValue::Int {
                        value: declared.and_then(|s| s.parse::<i64>().ok()).unwrap_or(0),
                    }),
                    LeafType::Float => Some(ConfigValue::Float {
                        value: declared.and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
                    }),
                    LeafType::String => Some(ConfigValue::String {
                        value: declared.unwrap_or("").to_string(),
                    }),
                    // EventId already filled by merge_config_values_into_tree.
                    // Action/Blob have no ConfigValue variant.
                    LeafType::EventId | LeafType::Action | LeafType::Blob => None,
                };
            }
            ConfigNode::Group(group) => {
                populate_defaults_recursive(&mut group.children);
            }
        }
    }
}

/// Build the config tree from a parsed CDI and apply the structure profile
/// overlay (event roles, relevance, connector profile, mode selections).
async fn build_tree_with_profile(
    node_key: &str,
    cdi: &Cdi,
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> NodeConfigTree {
    let mut tree = crate::node_tree::build_node_config_tree(node_key, cdi);

    if let Some(identity) = &cdi.identification {
        let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
        let model = identity.model.as_deref().unwrap_or("");
        if !manufacturer.is_empty() || !model.is_empty() {
            if let Some(profile) = crate::profile::load_profile(
                manufacturer,
                model,
                cdi,
                app_handle,
                &state.profiles,
            )
            .await
            {
                let shared_daughterboards =
                    crate::profile::load_shared_daughterboards(app_handle).await;
                let selections =
                    crate::commands::cdi::active_node_mode_selections(state, node_key)
                        .await;
                crate::commands::cdi::apply_profile_metadata_to_tree(
                    &mut tree,
                    node_key,
                    &profile,
                    shared_daughterboards.as_ref(),
                    cdi,
                    &selections,
                );
            }
        }
    }

    tree
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use lcc_rs::cdi::{
        EventIdElement, Group, IntElement, Segment,
    };

    fn make_test_cdi() -> Cdi {
        Cdi {
            identification: None,
            acdi: None,
            segments: vec![Segment {
                name: Some("Test".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: vec![
                    DataElement::Int(IntElement {
                        name: Some("Speed".to_string()),
                        description: None,
                        offset: 0,
                        size: 1,
                        min: None,
                        max: None,
                        default: None,
                        map: None,
                        hints: None,
                    }),
                    DataElement::EventId(EventIdElement {
                        name: Some("Event A".to_string()),
                        description: None,
                        offset: 0,
                    }),
                    DataElement::Group(Group {
                        name: Some("Outputs".to_string()),
                        description: None,
                        offset: 0,
                        replication: 2,
                        repname: vec!["Output".to_string()],
                        elements: vec![
                            DataElement::EventId(EventIdElement {
                                name: Some("On".to_string()),
                                description: None,
                                offset: 0,
                            }),
                            DataElement::EventId(EventIdElement {
                                name: Some("Off".to_string()),
                                description: None,
                                offset: 0,
                            }),
                        ],
                        hints: None,
                    }),
                ],
            }],
        }
    }

    #[test]
    fn collect_eventid_zeros_finds_all_eventid_leaves() {
        let cdi = make_test_cdi();
        let zeros = collect_eventid_zeros(&cdi);

        // Top-level EventId
        assert_eq!(zeros.get("seg:0/elem:1"), Some(&[0u8; 8]));

        // Replicated group instance 1
        assert_eq!(zeros.get("seg:0/elem:2#1/elem:0"), Some(&[0u8; 8]));
        assert_eq!(zeros.get("seg:0/elem:2#1/elem:1"), Some(&[0u8; 8]));

        // Replicated group instance 2
        assert_eq!(zeros.get("seg:0/elem:2#2/elem:0"), Some(&[0u8; 8]));
        assert_eq!(zeros.get("seg:0/elem:2#2/elem:1"), Some(&[0u8; 8]));

        // Total: 1 top-level + 2×2 replicated = 5
        assert_eq!(zeros.len(), 5);

        // No Int paths
        assert!(zeros.get("seg:0/elem:0").is_none());
    }

    #[test]
    fn collect_eventid_zeros_empty_cdi() {
        let cdi = Cdi {
            identification: None,
            acdi: None,
            segments: Vec::new(),
        };
        assert!(collect_eventid_zeros(&cdi).is_empty());
    }

    #[test]
    fn collect_eventid_zeros_non_replicated_group() {
        let cdi = Cdi {
            identification: None,
            acdi: None,
            segments: vec![Segment {
                name: None,
                description: None,
                space: 253,
                origin: 0,
                elements: vec![DataElement::Group(Group {
                    name: Some("Single".to_string()),
                    description: None,
                    offset: 0,
                    replication: 1,
                    repname: Vec::new(),
                    elements: vec![DataElement::EventId(EventIdElement {
                        name: Some("Evt".to_string()),
                        description: None,
                        offset: 0,
                    })],
                    hints: None,
                })],
            }],
        };
        let zeros = collect_eventid_zeros(&cdi);
        assert_eq!(zeros.len(), 1);
        assert_eq!(zeros.get("seg:0/elem:0/elem:0"), Some(&[0u8; 8]));
    }

    #[test]
    fn s9_merge_config_values_populates_eventid_leaves_in_tree() {
        let cdi = make_test_cdi();
        let config_values = collect_eventid_zeros(&cdi);
        let mut tree = crate::node_tree::build_node_config_tree("test-node", &cdi);

        // Before merge: EventId leaves have value = None
        let leaf_before = find_leaf(&tree, &["seg:0", "elem:1"]);
        assert!(leaf_before.is_some(), "EventId leaf should exist in tree");
        assert!(leaf_before.unwrap().value.is_none(), "value should be None before merge");

        // Merge
        merge_config_values_into_tree(&mut tree, &config_values);

        // After merge: EventId leaves have zero-byte values
        let leaf_after = find_leaf(&tree, &["seg:0", "elem:1"]).unwrap();
        assert!(leaf_after.value.is_some(), "EventId leaf should have a value after merge");
        match leaf_after.value.as_ref().unwrap() {
            crate::node_tree::ConfigValue::EventId { bytes, .. } => {
                assert_eq!(bytes, &[0u8; 8], "EventId should be all zeros");
            }
            other => panic!("Expected EventId ConfigValue, got {:?}", other),
        }

        // Int leaf should still be None (not in config_values)
        let int_leaf = find_leaf(&tree, &["seg:0", "elem:0"]).unwrap();
        assert!(int_leaf.value.is_none(), "Int leaf should not be populated by merge");

        // Replicated group EventIds should also be populated
        let rep_leaf = find_leaf(&tree, &["seg:0", "elem:2#1", "elem:0"]).unwrap();
        assert!(rep_leaf.value.is_some(), "Replicated EventId should be populated");
    }

    #[test]
    fn populate_leaf_defaults_fills_int_string_float_with_typed_defaults() {
        use crate::node_tree::{ConfigNode, ConfigValue, LeafConstraints, LeafNode, LeafType, NodeConfigTree, SegmentNode};

        fn leaf(name: &str, ty: LeafType, default_value: Option<&str>) -> ConfigNode {
            ConfigNode::Leaf(LeafNode {
                name: name.into(),
                description: None,
                element_type: ty,
                address: 0,
                size: 1,
                space: 253,
                path: vec![name.into()],
                value: None,
                event_role: None,
                constraints: default_value.map(|d| LeafConstraints {
                    min: None,
                    max: None,
                    default_value: Some(d.into()),
                    map_entries: None,
                }),
                button_text: None,
                dialog_text: None,
                action_value: 0,
                hint_slider: None,
                hint_radio: false,
                modified_value: None,
                write_state: None,
                write_error: None,
                read_only: false,
            })
        }

        let mut tree = NodeConfigTree {
            node_id: "placeholder:test".into(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: Vec::new(),
            segments: vec![SegmentNode {
                name: "Cfg".into(),
                description: None,
                origin: 0,
                space: 253,
                children: vec![
                    leaf("int_with_default", LeafType::Int, Some("42")),
                    leaf("int_no_default", LeafType::Int, None),
                    leaf("float_no_default", LeafType::Float, None),
                    leaf("string_no_default", LeafType::String, None),
                    leaf("string_with_default", LeafType::String, Some("hello")),
                    // Action / Blob have no ConfigValue variant — stay None.
                    leaf("action", LeafType::Action, None),
                    leaf("blob", LeafType::Blob, None),
                ],
            }],
        };

        populate_leaf_defaults_in_tree(&mut tree);

        let leaves: Vec<&LeafNode> = tree.segments[0]
            .children
            .iter()
            .map(|n| match n {
                ConfigNode::Leaf(l) => l,
                _ => panic!("expected leaf"),
            })
            .collect();

        assert!(matches!(leaves[0].value, Some(ConfigValue::Int { value: 42 })));
        assert!(matches!(leaves[1].value, Some(ConfigValue::Int { value: 0 })));
        assert!(matches!(leaves[2].value, Some(ConfigValue::Float { value }) if value == 0.0));
        assert!(matches!(&leaves[3].value, Some(ConfigValue::String { value }) if value.is_empty()));
        assert!(matches!(&leaves[4].value, Some(ConfigValue::String { value }) if value == "hello"));
        assert!(leaves[5].value.is_none(), "Action leaves stay unpopulated");
        assert!(leaves[6].value.is_none(), "Blob leaves stay unpopulated");
    }

    #[test]
    fn populate_leaf_defaults_does_not_overwrite_existing_values() {
        use crate::node_tree::{ConfigNode, ConfigValue, LeafNode, LeafType, NodeConfigTree, SegmentNode};

        let mut tree = NodeConfigTree {
            node_id: "placeholder:test".into(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: Vec::new(),
            segments: vec![SegmentNode {
                name: "Cfg".into(),
                description: None,
                origin: 0,
                space: 253,
                children: vec![ConfigNode::Leaf(LeafNode {
                    name: "i".into(),
                    description: None,
                    element_type: LeafType::Int,
                    address: 0,
                    size: 1,
                    space: 253,
                    path: vec!["i".into()],
                    value: Some(ConfigValue::Int { value: 99 }),
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
        };

        populate_leaf_defaults_in_tree(&mut tree);

        let ConfigNode::Leaf(l) = &tree.segments[0].children[0] else { panic!() };
        assert!(matches!(l.value, Some(ConfigValue::Int { value: 99 })));
    }

    /// Helper to find a leaf by its CDI path segments.
    fn find_leaf<'a>(
        tree: &'a crate::node_tree::NodeConfigTree,
        path: &[&str],
    ) -> Option<&'a crate::node_tree::LeafNode> {
        fn search<'b>(
            nodes: &'b [crate::node_tree::ConfigNode],
            path: &[String],
        ) -> Option<&'b crate::node_tree::LeafNode> {
            for node in nodes {
                match node {
                    crate::node_tree::ConfigNode::Leaf(leaf) => {
                        if leaf.path == path {
                            return Some(leaf);
                        }
                    }
                    crate::node_tree::ConfigNode::Group(group) => {
                        if let Some(found) = search(&group.children, path) {
                            return Some(found);
                        }
                    }
                }
            }
            None
        }
        let path_strings: Vec<String> = path.iter().map(|s| s.to_string()).collect();
        for segment in &tree.segments {
            if let Some(found) = search(&segment.children, &path_strings) {
                return Some(found);
            }
        }
        None
    }
}
