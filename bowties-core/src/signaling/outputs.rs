//! Signal-LCC physical output ownership, read from the effective
//! configuration. Pure over [`NodeConfigTree`]; no I/O.
//!
//! Two owners are documented for Signal-LCC outputs: Direct Lamp Control rows
//! and active Rule-to-Aspect masts. A mast is ONE owner of every lamp it uses
//! across all of its rules (a mast normally reuses a lamp, e.g. red in Stop
//! and Approach), so the same mast selecting an output twice is not a
//! conflict. The number of replicated Direct Lamp rows is the authoritative
//! output capacity.

use std::collections::HashMap;

use super::{OutputFacility, SignalHardwareHost, SignalOutput, SignalOutputOwner};
use crate::layout::facilities::{ClaimedSlotKind, FacilitiesDocument};
use crate::node_key::NodeKey;
use crate::node_tree::{
    replication_instances, ConfigNode, ConfigValue, NodeConfigTree,
};

/// Interpret the configuration into one [`SignalOutput`] per physical output.
pub fn signal_outputs_from_configuration(tree: &NodeConfigTree) -> Result<Vec<SignalOutput>, String> {
    let direct_lamp_segment = tree
        .segments
        .iter()
        .find(|segment| segment.name == "Direct Lamp Control")
        .ok_or_else(|| "Direct Lamp Control configuration is unavailable.".to_string())?;
    let lamps = replication_instances(&direct_lamp_segment.children, "Lamp");
    if lamps.is_empty() {
        return Err("Direct Lamp Control has no authoritative output rows.".to_string());
    }
    let output_count = lamps.len() as u32;
    let mut owners = HashMap::<u32, SignalOutputOwner>::new();
    for lamp in lamps {
        let selection = required_int_leaf(&lamp.children, "Lamp Selection")?;
        record_output_owner(
            &mut owners,
            selection,
            output_count,
            SignalOutputOwner::DirectLamp(lamp.instance),
        )?;
    }

    let mast_segment = tree
        .segments
        .iter()
        .find(|segment| segment.name == "Rule to Aspect")
        .ok_or_else(|| "Rule to Aspect configuration is unavailable.".to_string())?;
    for mast in replication_instances(&mast_segment.children, "Mast") {
        let processing = required_int_leaf(&mast.children, "Mast Processing")?;
        if processing == 0 {
            continue;
        }
        if !(1..=2).contains(&processing) {
            return Err(format!(
                "Mast {} has an unknown Mast Processing value {}.",
                mast.instance, processing
            ));
        }
        for rule in replication_instances(&mast.children, "Rule") {
            for appearance in replication_instances(&rule.children, "Appearance") {
                let selection = required_int_leaf(&appearance.children, "Lamp Selection")?;
                record_output_owner(
                    &mut owners,
                    selection,
                    output_count,
                    SignalOutputOwner::Mast(mast.instance),
                )?;
            }
        }
    }

    Ok((1..=output_count)
        .map(|ordinal| SignalOutput {
            ordinal,
            label: signal_lcc_output_label(ordinal),
            owner: owners.remove(&ordinal),
            facility: None,
        })
        .collect())
}

/// Name the Bowties facility behind each mast-owned output: the facility
/// whose claimed Mast slot on the host matches the output's owner. Node keys
/// are compared canonically. Direct lamps and unclaimed masts stay `None`.
pub fn attribute_output_facilities(hosts: &mut [SignalHardwareHost], facilities: &FacilitiesDocument) {
    for host in hosts {
        let Ok(host_key) = NodeKey::parse(&host.node_key) else { continue };
        for output in &mut host.outputs {
            let Some(SignalOutputOwner::Mast(mast)) = output.owner else { continue };
            output.facility = facilities
                .facilities
                .iter()
                .find(|facility| {
                    facility.block_signals.as_ref().is_some_and(|record| {
                        record.claimed_slots.iter().any(|slot| {
                            slot.kind == ClaimedSlotKind::Mast
                                && slot.index == mast
                                && NodeKey::parse(&slot.node_key).is_ok_and(|key| key == host_key)
                        })
                    })
                })
                .map(|facility| OutputFacility {
                    facility_id: facility.facility_id.clone(),
                    name: facility.name.clone(),
                });
        }
    }
}

fn required_int_leaf(children: &[ConfigNode], name: &str) -> Result<i64, String> {
    find_int_leaf(children, name)
        .unwrap_or_else(|| Err(format!("{name} is missing from the configuration evidence.")))
}

fn find_int_leaf(children: &[ConfigNode], name: &str) -> Option<Result<i64, String>> {
    for child in children {
        match child {
            ConfigNode::Leaf(leaf) if leaf.name == name => {
                return Some(match leaf.value.as_ref() {
                    Some(ConfigValue::Int { value }) => Ok(*value),
                    Some(_) => Err(format!("{name} has a non-integer configuration value.")),
                    None => Err(format!("{name} has no current configuration evidence.")),
                });
            }
            ConfigNode::Group(group) => {
                if let Some(value) = find_int_leaf(&group.children, name) {
                    return Some(value);
                }
            }
            ConfigNode::Leaf(_) => {}
        }
    }
    None
}

fn record_output_owner(
    owners: &mut HashMap<u32, SignalOutputOwner>,
    selection: i64,
    output_count: u32,
    owner: SignalOutputOwner,
) -> Result<(), String> {
    if selection == 0 {
        return Ok(());
    }
    let ordinal = u32::try_from(selection)
        .ok()
        .filter(|ordinal| (1..=output_count).contains(ordinal))
        .ok_or_else(|| format!("{owner} selects invalid output {selection}."))?;
    if let Some(previous_owner) = owners.insert(ordinal, owner) {
        if previous_owner == owner {
            return Ok(());
        }
        return Err(format!(
            "Output {} is claimed by both {} and {}.",
            ordinal, previous_owner, owner
        ));
    }
    Ok(())
}

fn signal_lcc_output_label(ordinal: u32) -> String {
    const POSITIONS: [&str; 4] = ["G", "Y", "R", "L"];
    let head = ((ordinal - 1) / 4) + 1;
    let position = POSITIONS[((ordinal - 1) % 4) as usize];
    format!("#{} H{}-{}", ordinal, head, position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_tree::{build_node_config_tree, LeafType};
    use crate::signaling::slots::LeafPathIndex;
    use lcc_rs::cdi::parser::parse_cdi;

    const CDI: &str = include_str!("../../../docs/ref/RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml");

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

    fn set_int(tree: &mut NodeConfigTree, path: &str, value: i64) {
        let at = LeafPathIndex::new(tree)
            .leaf(path)
            .map(|l| (l.space, l.address))
            .expect(path);
        for seg in &mut tree.segments {
            set_leaf(&mut seg.children, at.0, at.1, value);
        }
    }

    fn tree() -> NodeConfigTree {
        let mut tree = build_node_config_tree("050101010700", &parse_cdi(CDI).expect("CDI parses"));
        for seg in &mut tree.segments {
            zero_ints(&mut seg.children);
        }
        tree
    }

    fn mast_lamp(mast: u32, rule: u32, appearance: u32) -> String {
        format!("Rule to Aspect/Mast[{mast}]/Rule[{rule}]/Appearance[{appearance}]/Lamp Selection")
    }

    fn mast_processing(mast: u32) -> String {
        format!("Rule to Aspect/Mast[{mast}]/Mast Processing")
    }

    fn owner_of(outputs: &[SignalOutput], ordinal: u32) -> Option<String> {
        outputs[(ordinal - 1) as usize].owner.map(|o| o.to_string())
    }

    fn mast_facility_doc(node_key: &str, mast: u32) -> FacilitiesDocument {
        use crate::layout::facilities::{BlockSignalsRecord, ClaimedSlot, Facility};
        use crate::signaling::{
            EntranceEnd, ExitEnd, FacilitySignalSettings, Movements, TravelDirection,
        };
        let mut record = BlockSignalsRecord {
            direction: TravelDirection::default(),
            movements: Movements::default(),
            entrance_end: EntranceEnd::default(),
            exit_end: ExitEnd::default(),
            blocks: vec![],
            settings: FacilitySignalSettings::default(),
            signals: vec![],
            claimed_slots: vec![],
        };
        record.claimed_slots.push(ClaimedSlot {
            node_key: node_key.to_string(),
            kind: ClaimedSlotKind::Mast,
            index: mast,
        });
        FacilitiesDocument::new(vec![Facility {
            facility_id: "fac-1".into(),
            template_id: "automatic-block-signals".into(),
            name: "East Siding".into(),
            slot_bindings: Default::default(),
            block_signals: Some(record),
        }])
    }

    #[test]
    fn claimed_mast_outputs_name_their_facility_and_others_stay_unattributed() {
        let mut tree = tree();
        set_int(&mut tree, &mast_processing(1), 1);
        set_int(&mut tree, &mast_lamp(1, 1, 1), 3);
        set_int(&mut tree, &mast_processing(2), 1);
        set_int(&mut tree, &mast_lamp(2, 1, 1), 5);
        set_int(&mut tree, "Direct Lamp Control/Lamp[1]/Lamp Selection", 7);
        let outputs = signal_outputs_from_configuration(&tree).expect("no conflict");
        let mut hosts = vec![crate::signaling::SignalHardwareHost {
            node_key: "05.01.01.01.07.00".to_string(),
            node_name: "host".into(),
            style: crate::signaling::SignalStyle {
                style_id: "s".into(),
                binding_kind: "signalLccRgOutputs".into(),
                appearances: crate::signaling::SignalAppearanceSet {
                    stop: vec![],
                    approach: vec![],
                    clear: vec![],
                },
            },
            evidence: crate::signaling::SignalHardwareEvidence::Complete,
            evidence_detail: None,
            outputs,
        }];
        // Claimed slot uses the canonical (undotted) form of the same node key.
        attribute_output_facilities(&mut hosts, &mast_facility_doc("050101010700", 1));
        let facility = |ordinal: usize| hosts[0].outputs[ordinal - 1].facility.clone();
        assert_eq!(
            facility(3),
            Some(OutputFacility { facility_id: "fac-1".into(), name: "East Siding".into() })
        );
        assert_eq!(facility(5), None, "unclaimed mast stays unattributed");
        assert_eq!(facility(7), None, "direct lamp stays unattributed");
    }

    #[test]
    fn mast_reusing_a_lamp_across_rules_owns_it_once() {
        let mut tree = tree();
        set_int(&mut tree, &mast_processing(1), 1);
        set_int(&mut tree, &mast_lamp(1, 1, 1), 3);
        set_int(&mut tree, &mast_lamp(1, 2, 1), 3);
        set_int(&mut tree, &mast_lamp(1, 2, 2), 1);
        set_int(&mut tree, &mast_lamp(1, 3, 1), 1);
        let outputs = signal_outputs_from_configuration(&tree).expect("no conflict");
        assert_eq!(owner_of(&outputs, 3), Some("Mast 1".to_string()));
        assert_eq!(owner_of(&outputs, 1), Some("Mast 1".to_string()));
        assert_eq!(owner_of(&outputs, 2), None);
        assert_eq!(outputs[2].label, "#3 H1-R");
    }

    #[test]
    fn two_masts_on_one_output_conflict() {
        let mut tree = tree();
        set_int(&mut tree, &mast_processing(1), 1);
        set_int(&mut tree, &mast_processing(2), 1);
        set_int(&mut tree, &mast_lamp(1, 1, 1), 3);
        set_int(&mut tree, &mast_lamp(2, 1, 1), 3);
        let err = signal_outputs_from_configuration(&tree).unwrap_err();
        assert_eq!(err, "Output 3 is claimed by both Mast 1 and Mast 2.");
    }

    #[test]
    fn direct_lamp_and_mast_on_one_output_conflict() {
        let mut tree = tree();
        set_int(&mut tree, "Direct Lamp Control/Lamp[1]/Lamp Selection", 3);
        set_int(&mut tree, &mast_processing(1), 1);
        set_int(&mut tree, &mast_lamp(1, 1, 1), 3);
        let err = signal_outputs_from_configuration(&tree).unwrap_err();
        assert_eq!(err, "Output 3 is claimed by both Direct lamp 1 and Mast 1.");
    }

    #[test]
    fn two_direct_lamps_on_one_output_conflict() {
        let mut tree = tree();
        set_int(&mut tree, "Direct Lamp Control/Lamp[1]/Lamp Selection", 3);
        set_int(&mut tree, "Direct Lamp Control/Lamp[2]/Lamp Selection", 3);
        let err = signal_outputs_from_configuration(&tree).unwrap_err();
        assert_eq!(err, "Output 3 is claimed by both Direct lamp 1 and Direct lamp 2.");
    }

    #[test]
    fn unused_mast_selections_are_ignored() {
        let mut tree = tree();
        set_int(&mut tree, &mast_lamp(1, 1, 1), 3);
        let outputs = signal_outputs_from_configuration(&tree).expect("unused mast ignored");
        assert_eq!(owner_of(&outputs, 3), None);
        assert_eq!(outputs.len(), 16);
    }
}
