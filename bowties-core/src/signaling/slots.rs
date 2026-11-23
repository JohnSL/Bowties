//! Pure fact extraction and CDI path resolution for the Automatic Block
//! Signals compiler.
//!
//! Owns two translations the compiler deliberately does not know about:
//! node configuration tree -> slot facts (which masts and Logic conditionals
//! are free), and compiler CDI paths <-> (space, address) for the same tree.
//! No I/O; callers pass the effective tree (ADR-0015).
//!
//! Path form matches [`super::compiler`]: '/'-joined element names from the
//! segment down, `Name[k]` for the k-th instance of a replicated group and
//! `Name#k` when several siblings share a name (k-th, 1-based). Paths are
//! generated from the tree and looked up by exact string, so names that
//! contain '/' or '#' need no parsing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::compiler::{
    logic_function_path, mast_processing_path, EditValue, LogicFact, MastFact, NodeConfigEdit,
    NodeSlotFacts,
};
use crate::layout::facilities::{ClaimedSlot, ClaimedSlotKind, FacilitiesDocument};
use crate::node_tree::{
    effective_value, ConfigNode, ConfigValue, LeafNode, LeafType, NodeConfigTree,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SlotsError {
    #[error("Node {node_key} has no configuration element at '{path}'.")]
    UnknownPath { node_key: String, path: String },
    #[error("Element '{path}' on node {node_key} does not take a {value_kind} value.")]
    TypeMismatch { node_key: String, path: String, value_kind: &'static str },
}

/// A compiled edit with its resolved memory location, shaped for the
/// frontend's `configEditor.applyEdit` (value serializes as `TreeConfigValue`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedEdit {
    pub node_key: String,
    pub space: u8,
    pub address: u32,
    pub path: String,
    pub value: ConfigValue,
}

/// An unsaved edit as the frontend draft store keys it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DraftEdit {
    pub node_key: String,
    pub space: u8,
    pub address: u32,
    pub value: ConfigValue,
}

/// Path <-> location index over one node's configuration tree.
pub struct LeafPathIndex<'a> {
    by_path: HashMap<String, &'a LeafNode>,
}

impl<'a> LeafPathIndex<'a> {
    pub fn new(tree: &'a NodeConfigTree) -> Self {
        let mut leaves = Vec::new();
        let entries: Vec<(&str, &[ConfigNode])> =
            tree.segments.iter().map(|s| (s.name.as_str(), s.children.as_slice())).collect();
        for (i, (name, children)) in entries.iter().enumerate() {
            let total = entries.iter().filter(|(n, _)| n == name).count();
            let ordinal = entries[..=i].iter().filter(|(n, _)| n == name).count();
            collect_leaves(children, &component(name, total, ordinal), &mut leaves);
        }
        let mut by_path = HashMap::new();
        for (path, leaf) in leaves {
            by_path.entry(path).or_insert(leaf);
        }
        Self { by_path }
    }

    pub fn leaf(&self, path: &str) -> Option<&'a LeafNode> {
        self.by_path.get(path).copied()
    }
}

fn component(name: &str, total: usize, ordinal: usize) -> String {
    if total > 1 {
        format!("{name}#{ordinal}")
    } else {
        name.to_string()
    }
}

/// Groups count as one sibling entry per CDI element: the replication
/// wrapper (instance 0) or a plain group (replication 1).
fn is_entry(node: &ConfigNode) -> bool {
    match node {
        ConfigNode::Leaf(_) => true,
        ConfigNode::Group(g) => g.instance == 0 || g.replication_count == 1,
    }
}

fn node_name(node: &ConfigNode) -> &str {
    match node {
        ConfigNode::Leaf(l) => &l.name,
        ConfigNode::Group(g) => &g.name,
    }
}

fn collect_leaves<'a>(
    children: &'a [ConfigNode],
    prefix: &str,
    out: &mut Vec<(String, &'a LeafNode)>,
) {
    let entries: Vec<&ConfigNode> = children.iter().filter(|c| is_entry(c)).collect();
    for (i, node) in entries.iter().enumerate() {
        let name = node_name(node);
        let total = entries.iter().filter(|e| node_name(e) == name).count();
        let ordinal = entries[..=i].iter().filter(|e| node_name(e) == name).count();
        let path = format!("{prefix}/{}", component(name, total, ordinal));
        match node {
            ConfigNode::Leaf(leaf) => out.push((path, leaf)),
            ConfigNode::Group(g) if g.instance == 0 => {
                for inst in &g.children {
                    if let ConfigNode::Group(inst) = inst {
                        if inst.instance > 0 {
                            collect_leaves(&inst.children, &format!("{path}[{}]", inst.instance), out);
                        }
                    }
                }
            }
            ConfigNode::Group(g) => collect_leaves(&g.children, &path, out),
        }
    }
}

/// Slot facts for one node: which masts and Logic conditionals are in use
/// (non-zero effective processing/function, missing evidence, or claimed by
/// `claimed`) and each mast's Rule 1..3 `set aspect` events.
pub fn node_slot_facts(node_key: &str, tree: &NodeConfigTree, claimed: &[ClaimedSlot]) -> NodeSlotFacts {
    let index = LeafPathIndex::new(tree);
    let is_claimed = |kind: ClaimedSlotKind, i: u32| {
        claimed.iter().any(|c| c.node_key == node_key && c.kind == kind && c.index == i)
    };
    let nonzero = |path: &str| match index.leaf(path).and_then(effective_value) {
        Some(ConfigValue::Int { value }) => *value != 0,
        _ => true,
    };
    let masts = (1..)
        .take_while(|i| index.leaf(&mast_processing_path(*i)).is_some())
        .map(|i| {
            let event = |rule: u32| {
                let path = format!("Rule to Aspect/Mast[{i}]/Rule[{rule}]/set aspect");
                match index.leaf(&path).and_then(effective_value) {
                    Some(ConfigValue::EventId { bytes, .. }) => Some(*bytes),
                    _ => None,
                }
            };
            MastFact {
                index: i,
                in_use: nonzero(&mast_processing_path(i)) || is_claimed(ClaimedSlotKind::Mast, i),
                set_aspect_events: [event(1), event(2), event(3)],
            }
        })
        .collect();
    let logic = (1..)
        .take_while(|i| index.leaf(&logic_function_path(*i)).is_some())
        .map(|i| LogicFact {
            index: i,
            in_use: nonzero(&logic_function_path(i)) || is_claimed(ClaimedSlotKind::Logic, i),
        })
        .collect();
    NodeSlotFacts { node_key: node_key.to_string(), masts, logic }
}

/// Slots claimed by Automatic Block Signals facilities, excluding the facility
/// being composed.
pub fn claimed_slots_from_facilities(
    doc: &FacilitiesDocument,
    exclude_facility_id: Option<&str>,
) -> Vec<ClaimedSlot> {
    doc.facilities
        .iter()
        .filter(|f| Some(f.facility_id.as_str()) != exclude_facility_id)
        .filter_map(|f| f.block_signals.as_ref())
        .flat_map(|r| r.claimed_slots.iter().cloned())
        .collect()
}

/// Resolve compiled edits to memory locations. Unknown paths and edits whose
/// value kind does not match the element type are errors, never skipped.
pub fn resolve_edits(
    indexes: &HashMap<String, LeafPathIndex<'_>>,
    edits: &[NodeConfigEdit],
) -> Result<Vec<ResolvedEdit>, SlotsError> {
    edits
        .iter()
        .map(|edit| {
            let unknown = || SlotsError::UnknownPath {
                node_key: edit.node_key.clone(),
                path: edit.path.clone(),
            };
            let leaf = indexes
                .get(&edit.node_key)
                .and_then(|i| i.leaf(&edit.path))
                .ok_or_else(unknown)?;
            let (value, kind, expected) = match &edit.value {
                EditValue::Int { value } => (ConfigValue::Int { value: *value }, "integer", LeafType::Int),
                EditValue::String { value } => {
                    (ConfigValue::String { value: value.clone() }, "string", LeafType::String)
                }
                EditValue::EventId { bytes } => (
                    ConfigValue::EventId { bytes: *bytes, hex: lcc_rs::EventID::new(*bytes).to_canonical() },
                    "event ID",
                    LeafType::EventId,
                ),
            };
            if leaf.element_type != expected {
                return Err(SlotsError::TypeMismatch {
                    node_key: edit.node_key.clone(),
                    path: edit.path.clone(),
                    value_kind: kind,
                });
            }
            Ok(ResolvedEdit {
                node_key: edit.node_key.clone(),
                space: leaf.space,
                address: leaf.address,
                path: edit.path.clone(),
                value,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::facilities::{BlockSignalsRecord, Facility};
    use crate::node_tree::build_node_config_tree;
    use lcc_rs::cdi::parser::parse_cdi;

    const CDI: &str = include_str!("../../../docs/ref/RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml");
    const NODE: &str = "050101010700";

    fn tree() -> NodeConfigTree {
        build_node_config_tree(NODE, &parse_cdi(CDI).expect("CDI parses"))
    }

    fn set_leaf(nodes: &mut [ConfigNode], space: u8, address: u32, value: &ConfigValue) {
        for node in nodes {
            match node {
                ConfigNode::Leaf(l) if l.space == space && l.address == address => l.value = Some(value.clone()),
                ConfigNode::Leaf(_) => {}
                ConfigNode::Group(g) => set_leaf(&mut g.children, space, address, value),
            }
        }
    }

    fn set_value(tree: &mut NodeConfigTree, path: &str, value: ConfigValue) {
        let at = LeafPathIndex::new(tree).leaf(path).map(|l| (l.space, l.address)).expect(path);
        for seg in &mut tree.segments {
            set_leaf(&mut seg.children, at.0, at.1, &value);
        }
    }

    fn set_int(tree: &mut NodeConfigTree, path: &str, value: i64) {
        set_value(tree, path, ConfigValue::Int { value });
    }

    fn zeroed_tree() -> NodeConfigTree {
        let mut tree = tree();
        for i in 1..=8 {
            set_int(&mut tree, &mast_processing_path(i), 0);
        }
        for i in 1..=32 {
            set_int(&mut tree, &logic_function_path(i), 0);
        }
        tree
    }

    #[test]
    fn mast_with_processing_and_logic_with_function_are_in_use_others_free() {
        let mut tree = zeroed_tree();
        set_int(&mut tree, &mast_processing_path(1), 1);
        set_int(&mut tree, &logic_function_path(1), 2);
        let facts = node_slot_facts(NODE, &tree, &[]);
        assert_eq!(facts.masts.len(), 8);
        assert_eq!(facts.logic.len(), 32);
        assert!(facts.masts[0].in_use && !facts.masts[1].in_use);
        assert!(facts.logic[0].in_use && !facts.logic[1].in_use);
    }

    #[test]
    fn set_aspect_events_come_from_rules_one_to_three() {
        let mut tree = zeroed_tree();
        let bytes = [2, 0, 0, 0, 0, 0, 1, 3];
        set_value(
            &mut tree,
            "Rule to Aspect/Mast[1]/Rule[3]/set aspect",
            ConfigValue::EventId { bytes, hex: String::new() },
        );
        let facts = node_slot_facts(NODE, &tree, &[]);
        assert_eq!(facts.masts[0].set_aspect_events, [None, None, Some(bytes)]);
    }

    #[test]
    fn slot_claimed_by_another_facility_is_in_use_and_own_facility_is_excluded() {
        let tree = zeroed_tree();
        let record = |slots| BlockSignalsRecord { claimed_slots: slots, ..sample_record() };
        let claim = |kind, index| ClaimedSlot { node_key: NODE.into(), kind, index };
        let doc = FacilitiesDocument {
            facilities: vec![
                facility("other", record(vec![claim(ClaimedSlotKind::Mast, 2), claim(ClaimedSlotKind::Logic, 3)])),
                facility("mine", record(vec![claim(ClaimedSlotKind::Mast, 1)])),
            ],
            ..Default::default()
        };
        let claimed = claimed_slots_from_facilities(&doc, Some("mine"));
        let facts = node_slot_facts(NODE, &tree, &claimed);
        assert!(!facts.masts[0].in_use, "own facility's claim is excluded");
        assert!(facts.masts[1].in_use);
        assert!(facts.logic[2].in_use && !facts.logic[1].in_use);
    }

    #[test]
    fn recipe_paths_resolve_to_cdi_addresses() {
        let tree = tree();
        let indexes = HashMap::from([(NODE.to_string(), LeafPathIndex::new(&tree))]);
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
        let edit = |path: &str, value| NodeConfigEdit { node_key: NODE.into(), path: path.into(), value };
        let resolved = resolve_edits(
            &indexes,
            &[
                edit("Rule to Aspect/Mast[1]/Mast Processing", EditValue::Int { value: 1 }),
                edit("Conditionals/Logic[1]/Action#2[2]/Action Event", EditValue::EventId { bytes }),
                edit("Conditionals/Logic[1]/Variable #1/set true", EditValue::EventId { bytes }),
                edit("Conditionals/Logic[1]/Action#1/when true", EditValue::Int { value: 0 }),
            ],
        )
        .expect("resolves");
        assert_eq!((resolved[0].space, resolved[0].address), (253, 5296));
        // Logic 1 starts at 1648: Description(32) Function(1) then Variable #1: Trigger, Source, Track Speed, set true.
        assert_eq!(resolved[2].address, 1648 + 32 + 1 + 3);
        // Action#2[2] lies after Action#1 in the same Logic.
        assert!(resolved[1].address > resolved[3].address);
        assert_eq!(resolved[1].value, ConfigValue::EventId { bytes, hex: "0102030405060708".into() });
    }

    #[test]
    fn every_compiler_recipe_path_exists_in_the_signal_lcc_cdi() {
        let tree = tree();
        let index = LeafPathIndex::new(&tree);
        for path in [
            "Rule to Aspect/Mast[1]/Mast Description",
            "Rule to Aspect/Mast[1]/Lamp Fade",
            "Rule to Aspect/Mast[1]/Rule[3]/Appearance[4]/Lamp Phase (A-B) - Flash Rate",
            "Rule to Aspect/Mast[1]/Rule[2]/Appearance Effects",
            "Rule to Aspect/Mast[1]/Rule[2]/Effects Lamp",
            "Conditionals/Logic[1]/Description",
            "Conditionals/Logic[1]/Variable #1/Source",
            "Conditionals/Logic[1]/Variable #2/Trigger",
            "Conditionals/Logic[1]/Time Delay before action/Delay Time (1-60000)",
            "Conditionals/Logic[1]/Time Delay before action/Units",
            "Conditionals/Logic[1]/Action#2[4]/Condition",
            "Conditionals/Logic[1]/Logic Operation",
        ] {
            assert!(index.leaf(path).is_some(), "{path}");
        }
    }

    #[test]
    fn unknown_path_and_type_mismatch_are_errors() {
        let tree = tree();
        let indexes = HashMap::from([(NODE.to_string(), LeafPathIndex::new(&tree))]);
        let edit = |path: &str, value| NodeConfigEdit { node_key: NODE.into(), path: path.into(), value };
        let err = resolve_edits(&indexes, &[edit("Rule to Aspect/Mast[9]/Mast Processing", EditValue::Int { value: 1 })])
            .unwrap_err();
        assert!(matches!(err, SlotsError::UnknownPath { .. }));
        let err = resolve_edits(
            &indexes,
            &[edit("Rule to Aspect/Mast[1]/Mast Processing", EditValue::String { value: "x".into() })],
        )
        .unwrap_err();
        assert!(matches!(err, SlotsError::TypeMismatch { .. }));
    }

    #[test]
    fn every_edit_compiled_for_a_real_signal_lcc_tree_resolves_with_matching_kinds() {
        use crate::signaling::compiler::{
            compile_block_signals, BlockOccupancyEvents, CompileInput, SignalWiring, TWO_DRIVER_RG_STYLE,
        };
        use crate::signaling::{
            BlockSignalsRequest, SelectedBlock, SignalAppearanceSet, SignalDriver, SignalHardwareEvidence,
            SignalHardwareFacts, SignalHardwareHost, SignalOutput, SignalStyle,
        };
        let mut tree = zeroed_tree();
        for rule in 1..=3u8 {
            set_value(
                &mut tree,
                &format!("Rule to Aspect/Mast[1]/Rule[{rule}]/set aspect"),
                ConfigValue::EventId { bytes: [2, 0, 0, 0, 0, 0, 1, rule], hex: String::new() },
            );
        }
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
        let signal_id = crate::signaling::preview_block_signals(line.clone()).signals[0].signal_id.clone();
        let host = SignalHardwareHost {
            node_key: NODE.into(),
            node_name: "Signal LCC".into(),
            style: SignalStyle {
                style_id: TWO_DRIVER_RG_STYLE.into(),
                binding_kind: "signalLccRgOutputs".into(),
                appearances: SignalAppearanceSet {
                    stop: vec![SignalDriver::Red],
                    approach: vec![SignalDriver::Red, SignalDriver::Green],
                    clear: vec![SignalDriver::Green],
                },
            },
            evidence: SignalHardwareEvidence::Complete,
            evidence_detail: None,
            outputs: (1..=16)
                .map(|ordinal| SignalOutput { ordinal, label: format!("#{ordinal}"), owner: None, facility: None })
                .collect(),
        };
        let compiled = compile_block_signals(&CompileInput {
            line,
            wiring: vec![SignalWiring { signal_id, node_key: NODE.into(), red_output: 3, green_output: 1 }],
            hardware: SignalHardwareFacts { hosts: vec![host] },
            nodes: vec![node_slot_facts(NODE, &tree, &[])],
            occupancy: vec![BlockOccupancyEvents {
                block_id: "block:a".into(),
                occupied: Some([1, 1, 1, 1, 1, 1, 0, 1]),
                clear: Some([1, 1, 1, 1, 1, 1, 0, 2]),
            }],
            own_claims: vec![],
        });
        assert!(compiled.issues.is_empty(), "{:?}", compiled.issues);
        assert!(!compiled.edits.is_empty());
        assert_eq!(
            compiled.claimed_slots,
            vec![
                ClaimedSlot { node_key: NODE.into(), kind: ClaimedSlotKind::Mast, index: 1 },
                ClaimedSlot { node_key: NODE.into(), kind: ClaimedSlotKind::Logic, index: 1 },
            ]
        );
        let indexes = HashMap::from([(NODE.to_string(), LeafPathIndex::new(&tree))]);
        let resolved = resolve_edits(&indexes, &compiled.edits).expect("every compiled edit resolves");
        assert_eq!(resolved.len(), compiled.edits.len());
        let mut addresses: Vec<_> = resolved.iter().map(|e| (e.space, e.address)).collect();
        addresses.sort_unstable();
        addresses.dedup();
        assert_eq!(addresses.len(), resolved.len(), "no two edits target the same leaf");
    }

    fn sample_record() -> BlockSignalsRecord {
        serde_json::from_value(serde_json::json!({
            "direction": "eastward", "movements": "oneDirection",
            "entranceEnd": {"kind": "noPlant"}, "exitEnd": {"kind": "nothingBeyond"},
            "blocks": [],
            "settings": {"aspects": 3, "turnoutNotLined": "stop", "approachLighting": "alwaysLit"},
            "signals": [], "claimedSlots": []
        }))
        .expect("record")
    }

    fn facility(id: &str, record: BlockSignalsRecord) -> Facility {
        let mut f: Facility = serde_json::from_value(serde_json::json!({
            "facilityId": id, "name": id, "templateId": "automatic-block-signals", "slotBindings": {}
        }))
        .expect("facility");
        f.block_signals = Some(record);
        f
    }
}
