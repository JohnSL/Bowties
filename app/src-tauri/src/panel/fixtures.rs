//! Shared test fixtures: a layout holding a real Signal LCC configuration tree
//! (from the rev-C7c CDI), block detector channels and panel / Automatic Block
//! Signals facilities.

use std::collections::HashMap;

use bowties_core::layout::channels::{
    ChannelBinding, ChannelOwnership, ChannelRole, InformationChannel,
};
use bowties_core::layout::facilities::{BlockSignalsRecord, FacilitiesDocument, Facility, SignalRecord, SignalResources};
use bowties_core::layout::state::{CapturedNode, LayoutState};
use bowties_core::node_key::NodeKey;
use bowties_core::node_tree::{build_node_config_tree, ConfigNode, ConfigValue, LeafType, NodeConfigTree};
use bowties_core::profile::{make_profile_key, StructureProfile};
use bowties_core::signaling::slots::LeafPathIndex;
use bowties_core::signaling::{
    EntranceEnd, ExitEnd, FacilitySignalSettings, Movements, TravelDirection,
};
use lcc_rs::cdi::{parser::parse_cdi, Identification};

use crate::signaling::tests::{detected_block, detected_block_tree, layout_with_facilities};

const CDI: &str = include_str!("../../../../docs/ref/RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml");
pub const BOARD: &str = "05010101FF02";

fn zero_ints(nodes: &mut [ConfigNode]) {
    for node in nodes {
        match node {
            ConfigNode::Leaf(l) if l.element_type == LeafType::Int => l.value = Some(ConfigValue::Int { value: 0 }),
            ConfigNode::Leaf(_) => {}
            ConfigNode::Group(g) => zero_ints(&mut g.children),
        }
    }
}

fn set_at(nodes: &mut [ConfigNode], space: u8, address: u32, value: &ConfigValue) {
    for node in nodes {
        match node {
            ConfigNode::Leaf(l) if l.space == space && l.address == address => l.value = Some(value.clone()),
            ConfigNode::Leaf(_) => {}
            ConfigNode::Group(g) => set_at(&mut g.children, space, address, value),
        }
    }
}

/// A Signal LCC tree with every Int zeroed (all outputs free).
pub fn signal_lcc_tree() -> NodeConfigTree {
    let mut tree = build_node_config_tree("05.01.01.01.FF.02", &parse_cdi(CDI).expect("CDI parses"));
    tree.identity = Some(Identification {
        manufacturer: Some("RR-CirKits, Inc.".into()),
        model: Some("Signal-LCC".into()),
        hardware_version: None,
        software_version: Some("C7c".into()),
    });
    for seg in &mut tree.segments {
        zero_ints(&mut seg.children);
    }
    tree
}

pub fn set_value(tree: &mut NodeConfigTree, path: &str, value: ConfigValue) {
    let at = LeafPathIndex::new(tree).leaf(path).map(|l| (l.space, l.address)).expect(path);
    for seg in &mut tree.segments {
        set_at(&mut seg.children, at.0, at.1, &value);
    }
}

pub fn set_int(tree: &mut NodeConfigTree, path: &str, value: i64) {
    set_value(tree, path, ConfigValue::Int { value });
}

pub fn mast_processing(mast: u32) -> String {
    format!("Rule to Aspect/Mast[{mast}]/Mast Processing")
}

pub fn mast_lamp(mast: u32) -> String {
    format!("Rule to Aspect/Mast[{mast}]/Rule[1]/Appearance[1]/Lamp Selection")
}

pub fn block_channel(id: &str) -> InformationChannel {
    InformationChannel { id: id.into(), name: id.into(), ..detected_block() }
}

pub fn lamp_channel(id: &str, row: u32) -> InformationChannel {
    InformationChannel {
        id: id.into(),
        name: id.into(),
        role: ChannelRole::LampIndicator,
        style: "direct-lamp".into(),
        ownership: ChannelOwnership::UserOwned,
        binding: ChannelBinding::LampRow { node_key: BOARD.into(), row_ordinal: row },
    }
}

pub fn panel_facility(id: &str, name: &str, lamp_channels: &[&str]) -> Facility {
    Facility {
        facility_id: id.into(),
        template_id: "panel".into(),
        name: name.into(),
        slot_bindings: [("lamps".to_string(), lamp_channels.iter().map(|c| c.to_string()).collect())].into(),
        block_signals: None,
        panel: None,
    }
}

/// An Automatic Block Signals facility whose one signal owns `mast` on the board.
pub fn abs_facility(id: &str, name: &str, mast: u32) -> Facility {
    Facility {
        facility_id: id.into(),
        template_id: "automatic-block-signals".into(),
        name: name.into(),
        slot_bindings: Default::default(),
        block_signals: Some(BlockSignalsRecord {
            direction: TravelDirection::default(),
            movements: Movements::default(),
            entrance_end: EntranceEnd::default(),
            exit_end: ExitEnd::default(),
            blocks: vec![],
            settings: FacilitySignalSettings::default(),
            signals: vec![SignalRecord {
                signal_id: "sig-1".into(),
                name: "Signal".into(),
                protected_block: "blk".into(),
                channel_id: "ch".into(),
                resources: SignalResources { node_key: BOARD.into(), mast, track_circuits: vec![] },
            }],
        }),
        panel: None,
    }
}

/// The detector node plus the Signal LCC `board`, three block channels and the
/// given extra channels and facilities.
pub fn layout(board: NodeConfigTree, extra_channels: Vec<InformationChannel>, facilities: Vec<Facility>) -> LayoutState {
    let mut channels = vec![block_channel("b1"), block_channel("b2"), block_channel("b3")];
    channels.extend(extra_channels);
    let mut layout = layout_with_facilities(channels, FacilitiesDocument::new(facilities), Some(detected_block_tree()));
    layout.record_captured(
        NodeKey::parse(BOARD).unwrap(),
        CapturedNode { tree: Some(board), ..Default::default() },
    );
    layout
}

pub fn profiles() -> HashMap<String, Option<StructureProfile>> {
    let profile: StructureProfile =
        serde_yaml_ng::from_str(include_str!("../../profiles/RR-CirKits_Inc._Signal-LCC.profile.yaml"))
            .expect("the shipped Signal-LCC profile parses");
    HashMap::from([(make_profile_key("RR-CirKits, Inc.", "Signal-LCC"), Some(profile))])
}

