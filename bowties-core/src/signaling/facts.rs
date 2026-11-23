//! Effective-configuration fact assembly for Automatic Block Signals: every
//! reader (preview, pin ownership, compose) projects the same
//! [`LayoutState::effective_config_tree`] view, so unsaved drafts and pending
//! offline changes are seen identically.

use std::collections::HashMap;

use super::outputs::signal_outputs_from_configuration;
use super::{
    SignalAppearanceSet, SignalDriver, SignalHardwareEvidence, SignalHardwareFacts,
    SignalHardwareHost, SignalStyle,
};
use crate::layout::state::LayoutState;
use crate::node_key::NodeKey;
use crate::node_tree::NodeConfigTree;
use crate::profile::{make_profile_key, StructureProfile};

/// The effective configuration tree of every node with a stored tree.
pub fn effective_trees(layout: &LayoutState) -> HashMap<NodeKey, NodeConfigTree> {
    layout
        .all_tree_keys()
        .into_iter()
        .filter_map(|key| layout.effective_config_tree(&key).map(|tree| (key, tree.into_owned())))
        .collect()
}

/// Project the profile declarations and complete effective configuration of
/// each compatible Signal-LCC into read-only output facts. This is deliberately
/// a narrow interpreter, not an allocator or claims registry.
pub fn signal_hardware_facts(
    trees: &HashMap<NodeKey, NodeConfigTree>,
    profiles: &HashMap<String, Option<StructureProfile>>,
) -> SignalHardwareFacts {
    let hosts = trees
        .iter()
        .filter_map(|(node_key, tree)| signal_hardware_host(node_key, tree, profiles))
        .collect();
    SignalHardwareFacts { hosts }
}

fn signal_hardware_host(
    node_key: &NodeKey,
    tree: &NodeConfigTree,
    profiles: &HashMap<String, Option<StructureProfile>>,
) -> Option<SignalHardwareHost> {
    let identity = tree.identity.as_ref()?;
    let manufacturer = identity.manufacturer.as_deref()?;
    let model = identity.model.as_deref()?;
    let profile = profiles
        .get(&make_profile_key(manufacturer, model))
        .and_then(|profile| profile.as_ref())?;
    let style = profile
        .styles
        .iter()
        .find(|style| {
            style.binding_kind == "signalLccRgOutputs" && style.signal_appearances.is_some()
        })?;
    let style = signal_style_from_profile(style).ok()?;
    let node_name = format!("{} · {}", model, tree.node_id);
    match signal_outputs_from_configuration(tree) {
        Ok(outputs) => Some(SignalHardwareHost {
            node_key: node_key.to_string(),
            node_name,
            style,
            evidence: SignalHardwareEvidence::Complete,
            evidence_detail: None,
            outputs,
        }),
        Err(detail) => Some(SignalHardwareHost {
            node_key: node_key.to_string(),
            node_name,
            style,
            evidence: SignalHardwareEvidence::Invalid,
            evidence_detail: Some(detail),
            outputs: Vec::new(),
        }),
    }
}

fn signal_style_from_profile(style: &crate::profile::Style) -> Result<SignalStyle, String> {
    let appearances = style
        .signal_appearances
        .as_ref()
        .ok_or_else(|| format!("Style '{}' has no signal appearance map.", style.style_id))?;
    let drivers = |values: &[String]| {
        values
            .iter()
            .map(|value| match value.as_str() {
                "red" => Ok(SignalDriver::Red),
                "green" => Ok(SignalDriver::Green),
                _ => Err(format!(
                    "Style '{}' declares unsupported signal driver '{}'.",
                    style.style_id, value
                )),
            })
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(SignalStyle {
        style_id: style.style_id.clone(),
        binding_kind: style.binding_kind.clone(),
        appearances: SignalAppearanceSet {
            stop: drivers(&appearances.stop)?,
            approach: drivers(&appearances.approach)?,
            clear: drivers(&appearances.clear)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::layout::channels::ChannelsDocument;
    use crate::layout::facilities::FacilitiesDocument;
    use crate::layout::io::LayoutDirectoryReadData;
    use crate::layout::manifest::LayoutManifest;
    use crate::layout::offline_changes::{OfflineChange, OfflineChangeKind, OfflineChangeStatus};
    use crate::layout::state::LayoutState;
    use crate::layout::types::LayoutFile;
    use crate::node_key::NodeKey;
    use crate::node_tree::{build_node_config_tree, ConfigNode, ConfigValue, LeafType, NodeConfigTree};
    use crate::profile::{make_profile_key, StructureProfile};
    use crate::signaling::compiler::mast_processing_path;
    use crate::signaling::slots::{node_slot_facts, DraftEdit, LeafPathIndex};
    use crate::signaling::{SignalHardwareEvidence, SignalOutputOwner};
    use lcc_rs::cdi::parser::parse_cdi;

    const CDI: &str = include_str!("../../../docs/ref/RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml");
    const PROFILE: &str = include_str!("../../../app/src-tauri/profiles/RR-CirKits_Inc._Signal-LCC.profile.yaml");
    const NODE: &str = "050101010700";

    fn zero_ints(nodes: &mut [ConfigNode]) {
        for node in nodes {
            match node {
                ConfigNode::Leaf(l) if l.element_type == LeafType::Int => {
                    l.value = Some(ConfigValue::Int { value: 0 });
                }
                ConfigNode::Leaf(_) => {}
                ConfigNode::Group(g) => zero_ints(&mut g.children),
            }
        }
    }

    fn set_leaf(nodes: &mut [ConfigNode], space: u8, address: u32, value: i64) {
        for node in nodes {
            match node {
                ConfigNode::Leaf(l) if l.space == space && l.address == address => {
                    l.value = Some(ConfigValue::Int { value });
                }
                ConfigNode::Leaf(_) => {}
                ConfigNode::Group(g) => set_leaf(&mut g.children, space, address, value),
            }
        }
    }

    /// A Signal-LCC whose Mast 1 is active (Normal) and drives output 3.
    fn active_mast_tree() -> NodeConfigTree {
        let mut tree = build_node_config_tree("05.01.01.01.07.00", &parse_cdi(CDI).expect("CDI parses"));
        for seg in &mut tree.segments {
            zero_ints(&mut seg.children);
        }
        for (path, value) in [
            (mast_processing_path(1), 1),
            ("Rule to Aspect/Mast[1]/Rule[1]/Appearance[1]/Lamp Selection".to_string(), 3),
        ] {
            let at = LeafPathIndex::new(&tree).leaf(&path).map(|l| (l.space, l.address)).expect(&path);
            for seg in &mut tree.segments {
                set_leaf(&mut seg.children, at.0, at.1, value);
            }
        }
        tree
    }

    fn mast_processing_location(tree: &NodeConfigTree) -> (u8, u32) {
        LeafPathIndex::new(tree)
            .leaf(&mast_processing_path(1))
            .map(|l| (l.space, l.address))
            .expect("leaf")
    }

    fn layout() -> LayoutState {
        let loaded = LayoutDirectoryReadData {
            manifest: LayoutManifest {
                schema_version: 4,
                layout_id: "test".to_string(),
                captured_at: "2026-06-28T00:00:00Z".to_string(),
                last_saved_at: "2026-06-28T00:00:00Z".to_string(),
                active_mode: "offline_file".to_string(),
                match_thresholds: Default::default(),
                connections: Vec::new(),
            },
            node_snapshots: Vec::new(),
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            recovery_occurred: false,
            channels: ChannelsDocument::default(),
            facilities: FacilitiesDocument::default(),
            load_warnings: Vec::new(),
        };
        let mut state = LayoutState::from_loaded(PathBuf::from("/tmp"), loaded, HashMap::new(), HashMap::new());
        state.set_config_tree(NodeKey::parse(NODE).unwrap(), active_mast_tree());
        state
    }

    fn profiles() -> HashMap<String, Option<StructureProfile>> {
        let profile: StructureProfile = serde_yaml_ng::from_str(PROFILE).expect("shipped profile parses");
        HashMap::from([(make_profile_key("RR-CirKits, Inc.", "Signal-LCC"), Some(profile))])
    }

    fn mast_owner(layout: &LayoutState) -> Option<SignalOutputOwner> {
        let trees = effective_trees(layout);
        let hardware = signal_hardware_facts(&trees, &profiles());
        let host = &hardware.hosts[0];
        assert_eq!(host.evidence, SignalHardwareEvidence::Complete);
        host.outputs[2].owner
    }

    fn mast_slot_in_use(layout: &LayoutState) -> bool {
        let trees = effective_trees(layout);
        node_slot_facts(NODE, &trees[&NodeKey::parse(NODE).unwrap()], &[]).masts[0].in_use
    }

    #[test]
    fn saved_active_mast_owns_its_pin_and_slot() {
        let layout = layout();
        assert_eq!(mast_owner(&layout), Some(SignalOutputOwner::Mast(1)));
        assert!(mast_slot_in_use(&layout));
    }

    #[test]
    fn synced_draft_deactivating_a_mast_frees_its_pin_and_slot() {
        let mut layout = layout();
        let (space, address) = mast_processing_location(&active_mast_tree());
        layout.sync_config_drafts(vec![DraftEdit {
            node_key: NODE.to_string(),
            space,
            address,
            value: ConfigValue::Int { value: 0 },
        }]);
        assert_eq!(mast_owner(&layout), None);
        assert!(!mast_slot_in_use(&layout));
    }

    #[test]
    fn pending_offline_change_deactivating_a_mast_frees_its_pin_and_slot() {
        let mut layout = layout();
        let (space, address) = mast_processing_location(&active_mast_tree());
        layout.set_offline_changes(vec![OfflineChange {
            change_id: "c1".to_string(),
            kind: OfflineChangeKind::Config,
            node_key: Some(NODE.to_string()),
            space: Some(space),
            offset: Some(format!("0x{address:08X}")),
            baseline_value: "1".to_string(),
            planned_value: "0".to_string(),
            status: OfflineChangeStatus::Pending,
            error: None,
            updated_at: "2026-06-28T00:00:00Z".to_string(),
        }]);
        assert_eq!(mast_owner(&layout), None);
        assert!(!mast_slot_in_use(&layout));
    }
}
