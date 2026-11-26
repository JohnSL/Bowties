//! Effective-configuration fact assembly for Automatic Block Signals: every
//! reader (preview, pin ownership, compose) projects the same
//! [`LayoutState::effective_config_tree`] view, so unsaved drafts and pending
//! pending changes are seen identically.

use std::collections::HashMap;

use super::compiler::{
    mast_processing_path, release_slots, untracked_release_targets, CompileIssue, CompileIssueCode, EditValue,
    NodeConfigEdit, NodeSlotFacts, UntrackedOwnerRelease,
};
use super::outputs::signal_outputs_from_configuration;
use super::slots::{node_slot_facts, LeafPathIndex};
use super::{
    ReleaseRequest, SignalAppearanceSet, SignalDriver, SignalHardwareEvidence, SignalHardwareFacts,
    SignalHardwareHost, SignalOutputOwner, SignalStyle,
};
use crate::layout::facilities::{ClaimedSlot, ClaimedSlotKind};
use crate::layout::state::LayoutState;
use crate::node_key::NodeKey;
use crate::node_tree::{ConfigNode, ConfigValue, NodeConfigTree};
use crate::profile::{make_profile_key, StructureProfile};

/// The effective configuration tree of every node with a stored tree.
pub fn effective_trees(layout: &LayoutState) -> HashMap<NodeKey, NodeConfigTree> {
    layout
        .all_tree_keys()
        .into_iter()
        .filter_map(|key| layout.effective_config_tree(&key).map(|tree| (key, tree.into_owned())))
        .collect()
}

/// The effective trees as they read once the requested releases are applied,
/// and the release edits that produce that state.
#[derive(Debug, Clone, Default)]
pub struct ReleaseOverlay {
    /// Every tree, with the release edits applied. Unchanged when `issues`
    /// is not empty.
    pub trees: HashMap<NodeKey, NodeConfigTree>,
    /// The release edits (paths on the request's node keys); empty on issues.
    pub edits: Vec<NodeConfigEdit>,
    /// Blocking issues: a named owner that cannot be released.
    pub issues: Vec<CompileIssue>,
}

/// Apply the named releases to a request-scoped copy of the effective trees.
///
/// This is the one place releases take effect: pin owners, slot facts and the
/// free-slot pick are all derived from `trees` afterwards. An owner is released
/// only when named, and only when it is a mast or Direct Lamp row that is
/// really in use and that no facility (ABS mast or panel lamp row) claims (`claimed` holds every
/// facility's slots, including the one being composed). Fresh event IDs the
/// frontend will generate read as placeholders that match no real event.
pub fn overlay_releases(
    trees: HashMap<NodeKey, NodeConfigTree>,
    release: &[ReleaseRequest],
    claimed: &[ClaimedSlot],
) -> ReleaseOverlay {
    let mut overlay = ReleaseOverlay { trees, ..Default::default() };
    if release.is_empty() {
        return overlay;
    }
    let invalid = |node_key: &str, detail: String| CompileIssue {
        code: CompileIssueCode::ReleaseInvalid,
        signal_id: None,
        node_key: Some(node_key.to_string()),
        detail,
        binding_issue: None,
        logic: Vec::new(),
    };
    let mut owners = Vec::new();
    let mut facts: Vec<NodeSlotFacts> = Vec::new();
    for request in release {
        let Some((key, tree)) = NodeKey::parse(&request.node_key)
            .ok()
            .and_then(|key| overlay.trees.get(&key).map(|tree| (key, tree)))
        else {
            overlay.issues.push(invalid(
                &request.node_key,
                format!("{} is on a node whose configuration is not loaded.", request.owner),
            ));
            continue;
        };
        let claims = claims_on(claimed, &key, &request.node_key);
        let index = LeafPathIndex::new(tree);
        let nonzero = |path: &str| {
            matches!(index.leaf(path).and_then(|leaf| leaf.value.as_ref()), Some(ConfigValue::Int { value }) if *value != 0)
        };
        let problem = match request.owner {
            SignalOutputOwner::Mast(n) if claims.iter().any(|c| c.kind == ClaimedSlotKind::Mast && c.index == n) => {
                Some("is owned by a facility and cannot be released here")
            }
            SignalOutputOwner::DirectLamp(n)
                if claims.iter().any(|c| c.kind == ClaimedSlotKind::DirectLamp && c.index == n) =>
            {
                Some("is owned by a facility and cannot be released here")
            }
            SignalOutputOwner::Mast(n) if !nonzero(&mast_processing_path(n)) => Some("is not in use"),
            SignalOutputOwner::DirectLamp(n)
                if !nonzero(&format!("Direct Lamp Control/Lamp[{n}]/Lamp Selection")) =>
            {
                Some("is not in use")
            }
            _ => None,
        };
        if let Some(problem) = problem {
            overlay.issues.push(invalid(&request.node_key, format!("{} {problem}.", request.owner)));
            continue;
        }
        if !facts.iter().any(|f| f.node_key == request.node_key) {
            facts.push(node_slot_facts(&request.node_key, tree, &claims));
        }
        owners.push((request.node_key.clone(), request.owner));
    }
    if !overlay.issues.is_empty() {
        return overlay;
    }
    let untracked = untracked_release_targets(&owners, &facts);
    let released = release_slots(&untracked.targets, &facts);
    overlay.issues = released.issues;
    if !overlay.issues.is_empty() {
        return overlay;
    }
    let mut placeholder = 0u16;
    for edit in &released.edits {
        let Some(tree) = NodeKey::parse(&edit.node_key).ok().and_then(|key| overlay.trees.get_mut(&key)) else {
            continue;
        };
        let Some(at) = LeafPathIndex::new(tree).leaf(&edit.path).map(|l| (l.space, l.address)) else { continue };
        let value = match &edit.value {
            EditValue::Int { value } => ConfigValue::Int { value: *value },
            EditValue::String { value } => ConfigValue::String { value: value.clone() },
            EditValue::EventId { bytes } => event_value(*bytes),
            EditValue::FreshEventId => {
                placeholder += 1;
                let [hi, lo] = placeholder.to_be_bytes();
                event_value([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, hi, lo])
            }
        };
        for segment in &mut tree.segments {
            set_leaf_value(&mut segment.children, at, &value);
        }
    }
    overlay.edits = released.edits;
    overlay
}

/// The facilities' claims on one node, re-keyed to the string the request uses
/// (slot facts compare node keys as strings).
fn claims_on(claimed: &[ClaimedSlot], key: &NodeKey, as_key: &str) -> Vec<ClaimedSlot> {
    claimed
        .iter()
        .filter(|c| NodeKey::parse(&c.node_key).is_ok_and(|k| &k == key))
        .map(|c| ClaimedSlot { node_key: as_key.to_string(), ..c.clone() })
        .collect()
}

fn event_value(bytes: [u8; 8]) -> ConfigValue {
    ConfigValue::EventId { bytes, hex: lcc_rs::EventID::new(bytes).to_canonical() }
}

/// Set a leaf's effective value.
fn set_leaf_value(nodes: &mut [ConfigNode], at: (u8, u32), value: &ConfigValue) {
    for node in nodes {
        match node {
            ConfigNode::Leaf(leaf) if (leaf.space, leaf.address) == at => {
                leaf.value = Some(value.clone());
            }
            ConfigNode::Leaf(_) => {}
            ConfigNode::Group(group) => set_leaf_value(&mut group.children, at, value),
        }
    }
}

/// Every mast outside Bowties that drives a pin on a host, with the Logic
/// that sets its aspects, so the Wiring step can describe it before any
/// release. Read from the same (overlaid) trees as the hosts' pin owners.
pub fn outside_mast_owners(
    trees: &HashMap<NodeKey, NodeConfigTree>,
    hosts: &[SignalHardwareHost],
    claimed: &[ClaimedSlot],
) -> Vec<UntrackedOwnerRelease> {
    let mut owners: Vec<(String, SignalOutputOwner)> = Vec::new();
    let mut facts = Vec::new();
    for host in hosts {
        let Ok(key) = NodeKey::parse(&host.node_key) else { continue };
        let Some(tree) = trees.get(&key) else { continue };
        facts.push(node_slot_facts(&host.node_key, tree, &claims_on(claimed, &key, &host.node_key)));
        for output in &host.outputs {
            let Some(owner @ SignalOutputOwner::Mast(_)) = output.owner else { continue };
            let entry = (host.node_key.clone(), owner);
            if output.facility.is_none() && !owners.contains(&entry) {
                owners.push(entry);
            }
        }
    }
    untracked_release_targets(&owners, &facts).owners
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
    use crate::layout::pending_changes::{PendingChange, PendingChangeKind, PendingChangeStatus};
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
            pending_changes: Vec::new(),
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
    fn pending_change_deactivating_a_mast_frees_its_pin_and_slot() {
        let mut layout = layout();
        let (space, address) = mast_processing_location(&active_mast_tree());
        layout.set_pending_changes(vec![PendingChange {
            change_id: "c1".to_string(),
            kind: PendingChangeKind::Config,
            node_key: Some(NODE.to_string()),
            space: Some(space),
            offset: Some(format!("0x{address:08X}")),
            baseline_value: "1".to_string(),
            planned_value: "0".to_string(),
            status: PendingChangeStatus::Pending,
            error: None,
            updated_at: "2026-06-28T00:00:00Z".to_string(),
            origin: None,
        }]);
        assert_eq!(mast_owner(&layout), None);
        assert!(!mast_slot_in_use(&layout));
    }

    fn set_event(tree: &mut NodeConfigTree, path: &str, bytes: [u8; 8]) {
        let at = LeafPathIndex::new(tree).leaf(path).map(|l| (l.space, l.address)).expect(path);
        fn go(nodes: &mut [ConfigNode], at: (u8, u32), bytes: [u8; 8]) {
            for node in nodes {
                match node {
                    ConfigNode::Leaf(l) if (l.space, l.address) == at => {
                        l.value = Some(ConfigValue::EventId { bytes, hex: String::new() });
                    }
                    ConfigNode::Leaf(_) => {}
                    ConfigNode::Group(g) => go(&mut g.children, at, bytes),
                }
            }
        }
        for seg in &mut tree.segments {
            go(&mut seg.children, at, bytes);
        }
    }

    fn set_path_int(tree: &mut NodeConfigTree, path: &str, value: i64) {
        let at = LeafPathIndex::new(tree).leaf(path).map(|l| (l.space, l.address)).expect(path);
        for seg in &mut tree.segments {
            set_leaf(&mut seg.children, at.0, at.1, value);
        }
    }

    /// Mast 1 (Processing Normal) owns pins 3 and 1 and its rule 4 lamp 5; Logic 1 sets
    /// its aspects. Masts 1 and 2 carry Stop/Clear set-aspect events.
    fn leftover_tree() -> NodeConfigTree {
        let mut tree = active_mast_tree();
        for mast in 1..=2u8 {
            for rule in 1..=3u8 {
                set_event(&mut tree, &format!("Rule to Aspect/Mast[{mast}]/Rule[{rule}]/set aspect"), [2, 0, 0, 0, 0, 0, mast, rule]);
            }
        }
        set_path_int(&mut tree, "Rule to Aspect/Mast[1]/Rule[2]/Appearance[1]/Lamp Selection", 1);
        set_path_int(&mut tree, "Rule to Aspect/Mast[1]/Rule[4]/Appearance[1]/Lamp Selection", 5);
        set_path_int(&mut tree, &crate::signaling::compiler::logic_function_path(1), 2);
        set_path_int(&mut tree, "Conditionals/Logic[1]/Action#2[1]/Condition", 3);
        set_event(&mut tree, "Conditionals/Logic[1]/Action#2[1]/Action Event", [2, 0, 0, 0, 0, 0, 1, 1]);
        tree
    }

    fn compile_with(
        trees: &HashMap<NodeKey, NodeConfigTree>,
    ) -> crate::signaling::compiler::CompiledBlockSignals {
        use crate::signaling::compiler::{compile_block_signals, BlockOccupancyEvents, CompileInput, SignalWiring};
        use crate::signaling::{preview_block_signals, BlockSignalsRequest, SelectedBlock};
        let line = BlockSignalsRequest {
            facility_id: "abs:test".into(),
            facility_name: "Main".into(),
            blocks: vec![SelectedBlock {
                block_id: "block:a".into(),
                name: "Block A".into(),
                joined_to_previous: false,
                turnouts: vec![],
            }],
            ..Default::default()
        };
        let signal_id = preview_block_signals(line.clone()).signals[0].signal_id.clone();
        let key = NodeKey::parse(NODE).unwrap();
        compile_block_signals(&CompileInput {
            line,
            wiring: vec![SignalWiring { signal_id, node_key: NODE.into(), red_output: 3, green_output: 1 }],
            hardware: signal_hardware_facts(trees, &profiles()),
            nodes: vec![node_slot_facts(NODE, &trees[&key], &[])],
            occupancy: vec![BlockOccupancyEvents {
                block_id: "block:a".into(),
                occupied: Some([1, 1, 1, 1, 1, 1, 0, 1]),
                clear: Some([1, 1, 1, 1, 1, 1, 0, 2]),
            }],
            saved_resources: vec![],
            logic_placement: vec![],
        })
    }

    fn release_of(owner: SignalOutputOwner) -> Vec<crate::signaling::ReleaseRequest> {
        vec![crate::signaling::ReleaseRequest { node_key: NODE.into(), owner }]
    }

    #[test]
    fn releasing_an_untracked_mast_frees_its_pins_and_slots_and_release_edits_merge_under_the_recipe() {
        use crate::signaling::compiler::{merge_release_edits, EditValue};
        let key = NodeKey::parse(NODE).unwrap();
        let trees = HashMap::from([(key.clone(), leftover_tree())]);
        let blocked = compile_with(&trees);
        assert!(blocked.issues.iter().any(|i| i.code == crate::signaling::compiler::CompileIssueCode::OutputInUse));

        let overlay = overlay_releases(trees, &release_of(SignalOutputOwner::Mast(1)), &[]);
        assert_eq!(overlay.issues, vec![]);
        let hardware = signal_hardware_facts(&overlay.trees, &profiles());
        assert!(hardware.hosts[0].outputs.iter().all(|o| o.owner.is_none()), "released mast's pins have no owner");

        let compiled = compile_with(&overlay.trees);
        assert_eq!(compiled.issues, vec![]);
        assert_eq!(compiled.resources[0].resources.mast, 1, "the freed Mast 1 is the lowest free mast");
        assert_eq!(compiled.placement[0].groups[0].first, 1, "the freed Logic 1 is the lowest free Logic");

        let merged = merge_release_edits(overlay.edits, compiled.edits);
        let at = |path: &str| -> Vec<EditValue> {
            merged.iter().filter(|e| e.path == path).map(|e| e.value.clone()).collect()
        };
        let mut leaves: Vec<_> = merged.iter().map(|e| (e.node_key.clone(), e.path.clone())).collect();
        let total = leaves.len();
        leaves.sort();
        leaves.dedup();
        assert_eq!(leaves.len(), total, "no leaf is edited twice");
        // Release-only leaves survive: rule 4 lamp and a Logic variable event.
        assert_eq!(at("Rule to Aspect/Mast[1]/Rule[4]/Appearance[1]/Lamp Selection"), vec![EditValue::Int { value: 0 }]);
        assert_eq!(at("Conditionals/Logic[1]/Variable #2/set true"), vec![EditValue::FreshEventId]);
        // Leaves both write take the recipe value.
        assert_eq!(at("Rule to Aspect/Mast[1]/Rule[1]/Appearance[1]/Lamp Selection"), vec![EditValue::Int { value: 3 }]);
        assert_eq!(
            at("Conditionals/Logic[1]/Variable #1/set true"),
            vec![EditValue::EventId { bytes: [1, 1, 1, 1, 1, 1, 0, 1] }]
        );
    }

    #[test]
    fn outside_masts_on_a_host_report_matched_logic_and_facility_masts_are_left_out() {
        let mut tree = leftover_tree();
        // Mast 2 drives pin 7 and a facility owns it.
        set_path_int(&mut tree, &mast_processing_path(2), 1);
        set_path_int(&mut tree, "Rule to Aspect/Mast[2]/Rule[1]/Appearance[1]/Lamp Selection", 7);
        let trees = HashMap::from([(NodeKey::parse(NODE).unwrap(), tree)]);
        let mut hosts = signal_hardware_facts(&trees, &profiles()).hosts;
        hosts[0].outputs[6].facility =
            Some(crate::signaling::OutputFacility { facility_id: "f".into(), name: "F".into() });

        let outside = outside_mast_owners(&trees, &hosts, &[]);
        assert_eq!(outside.len(), 1);
        assert_eq!(outside[0].owner, SignalOutputOwner::Mast(1));
        assert_eq!(outside[0].matched_logic, vec![1]);
        assert!(outside[0].kept_logic.is_empty());
    }

    #[test]
    fn releasing_a_facility_owned_mast_or_a_non_owner_is_a_blocking_issue_never_a_release() {
        use crate::layout::facilities::{ClaimedSlot, ClaimedSlotKind};
        let key = NodeKey::parse(NODE).unwrap();
        let trees = HashMap::from([(key, leftover_tree())]);
        let claimed = [ClaimedSlot { node_key: NODE.into(), kind: ClaimedSlotKind::Mast, index: 1 }];
        let facility_owned = overlay_releases(trees.clone(), &release_of(SignalOutputOwner::Mast(1)), &claimed);
        let idle_mast = overlay_releases(trees.clone(), &release_of(SignalOutputOwner::Mast(2)), &[]);
        let idle_lamp = overlay_releases(trees, &release_of(SignalOutputOwner::DirectLamp(9)), &[]);
        for overlay in [facility_owned, idle_mast, idle_lamp] {
            assert_eq!(overlay.issues.len(), 1);
            assert_eq!(overlay.issues[0].code, crate::signaling::compiler::CompileIssueCode::ReleaseInvalid);
            assert!(overlay.edits.is_empty());
        }
    }
    #[test]
    fn releasing_a_panel_claimed_direct_lamp_is_a_blocking_issue_never_a_release() {
        use crate::layout::facilities::{ClaimedSlot, ClaimedSlotKind};
        let key = NodeKey::parse(NODE).unwrap();
        let trees = HashMap::from([(key, leftover_tree())]);
        let claimed = [ClaimedSlot { node_key: NODE.into(), kind: ClaimedSlotKind::DirectLamp, index: 1 }];
        let overlay = overlay_releases(trees, &release_of(SignalOutputOwner::DirectLamp(1)), &claimed);
        assert_eq!(overlay.issues.len(), 1);
        assert!(overlay.issues[0].detail.contains("owned by a facility and cannot be released here"));
        assert!(overlay.edits.is_empty());
    }
}
