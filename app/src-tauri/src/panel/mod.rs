//! Panel facility IPC adapter (Spec 021): read-only wiring facts and compose,
//! both derived from the effective layout (saved + drafts). No drafts, claims,
//! configuration writes or bus activity. The compile policy lives in
//! `bowties_core::panel::compiler`; this module only gathers facts and
//! resolves the generated edits to memory locations.

#[cfg(test)]
pub(crate) mod fixtures;

use std::collections::HashMap;

use bowties_core::{
    layout::{facilities::{ClaimedSlotKind, FacilitiesDocument, PanelRecord}, state::LayoutState},
    node_key::NodeKey,
    node_tree::{ConfigValue, NodeConfigTree},
    panel::compiler::{
        compile_panel, PanelBoardFacts, PanelCompilation, PanelCompileInput, PanelIssue, PanelLampRequest,
        PanelRowAssignment,
    },
    profile::StructureProfile,
    signaling::{
        slots::{claimed_slots_from_facilities, node_slot_facts, resolve_edits, LeafPathIndex, ResolvedEdit},
        SignalHardwareEvidence, SignalHardwareHost, SignalOutput, SignalOutputOwner,
    },
};
use serde::{Deserialize, Serialize};

use crate::signaling::{occupancy_events, proposal_facts_from_layout};
use crate::state::AppState;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelWiringFactsRequest {
    /// Restrict to these boards; every board with Direct Lamp Control when absent.
    #[serde(default)]
    pub node_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PinOwner {
    Free,
    #[serde(rename_all = "camelCase")]
    Panel { facility_id: String, name: String, row: u32 },
    #[serde(rename_all = "camelCase")]
    Facility { facility_id: String, name: String },
    #[serde(rename_all = "camelCase")]
    Outside { row: Option<u32>, description: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinFact {
    pub output: u32,
    pub label: String,
    pub group: String,
    pub owner: PinOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardWiring {
    pub node_key: String,
    pub name: String,
    pub pins: Vec<PinFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WiringIssue {
    pub node_key: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelWiringFactsResponse {
    pub boards: Vec<BoardWiring>,
    pub issues: Vec<WiringIssue>,
}

pub async fn wiring_facts(
    state: &AppState,
    request: PanelWiringFactsRequest,
) -> Result<PanelWiringFactsResponse, String> {
    let layout = state.layout_state.read().await;
    let Some(layout) = layout.as_ref() else {
        return Err("No layout is open.".to_string());
    };
    let profiles = state.profiles.read().await;
    wiring_facts_from_layout(layout, &profiles, &request)
}

fn wiring_facts_from_layout(
    layout: &LayoutState,
    profiles: &HashMap<String, Option<StructureProfile>>,
    request: &PanelWiringFactsRequest,
) -> Result<PanelWiringFactsResponse, String> {
    let facts = proposal_facts_from_layout(layout, profiles, &[]);
    let wanted: Option<Vec<NodeKey>> = request
        .node_keys
        .as_ref()
        .map(|keys| {
            keys.iter()
                .map(|k| NodeKey::parse(k).map_err(|e| format!("Invalid node key '{k}': {e}")))
                .collect::<Result<_, _>>()
        })
        .transpose()?;
    let mut response = PanelWiringFactsResponse::default();
    let mut hosts: Vec<&SignalHardwareHost> = facts
        .hardware
        .hosts
        .iter()
        .filter(|h| {
            wanted.as_ref().is_none_or(|w| NodeKey::parse(&h.node_key).is_ok_and(|key| w.contains(&key)))
        })
        .collect();
    hosts.sort_by(|a, b| a.node_key.cmp(&b.node_key));
    for host in hosts {
        if host.evidence != SignalHardwareEvidence::Complete {
            response.issues.push(WiringIssue {
                node_key: host.node_key.clone(),
                detail: host
                    .evidence_detail
                    .clone()
                    .unwrap_or_else(|| "The board's outputs are unavailable.".to_string()),
            });
            continue;
        }
        let tree = NodeKey::parse(&host.node_key).ok().and_then(|key| facts.trees.get(&key));
        let facilities = layout.effective_facilities();
        let pins = host
            .outputs
            .iter()
            .map(|output| PinFact {
                output: output.ordinal,
                label: output.label.clone(),
                group: output.label.split([' ', '-']).nth(1).unwrap_or_default().to_string(),
                owner: pin_owner(output, facilities, tree),
            })
            .collect();
        response.boards.push(BoardWiring { node_key: host.node_key.clone(), name: host.node_name.clone(), pins });
    }
    for key in wanted.iter().flatten() {
        if !response.boards.iter().any(|b| NodeKey::parse(&b.node_key).is_ok_and(|k| k == *key))
            && !response.issues.iter().any(|i| NodeKey::parse(&i.node_key).is_ok_and(|k| k == *key))
        {
            response.issues.push(WiringIssue {
                node_key: key.to_string(),
                detail: "The board has no Direct Lamp Control configuration available.".to_string(),
            });
        }
    }
    Ok(response)
}

fn pin_owner(output: &SignalOutput, facilities: &FacilitiesDocument, tree: Option<&NodeConfigTree>) -> PinOwner {
    let Some(owner) = output.owner else { return PinOwner::Free };
    if let Some(claimant) = &output.facility {
        let is_panel = facilities
            .facilities
            .iter()
            .any(|f| f.facility_id == claimant.facility_id && f.template_id == "panel");
        return match owner {
            SignalOutputOwner::DirectLamp(row) if is_panel => {
                PinOwner::Panel { facility_id: claimant.facility_id.clone(), name: claimant.name.clone(), row }
            }
            _ => PinOwner::Facility { facility_id: claimant.facility_id.clone(), name: claimant.name.clone() },
        };
    }
    match owner {
        SignalOutputOwner::DirectLamp(row) => {
            let description = tree
                .and_then(|tree| {
                    LeafPathIndex::new(tree)
                        .leaf(&format!("Direct Lamp Control/Lamp[{row}]/Lamp Description"))
                        .and_then(|leaf| match &leaf.value {
                            Some(ConfigValue::String { value }) if !value.trim().is_empty() => Some(value.clone()),
                            _ => None,
                        })
                })
                .unwrap_or_else(|| owner.to_string());
            PinOwner::Outside { row: Some(row), description }
        }
        SignalOutputOwner::Mast(_) => PinOwner::Outside { row: None, description: owner.to_string() },
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposePanelRequest {
    /// The panel being composed; its own claimed rows do not count as in use.
    pub facility_id: String,
    pub record: PanelRecord,
    pub lamps: Vec<PanelLampRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposePanelResponse {
    pub edits: Vec<ResolvedEdit>,
    pub assignments: Vec<PanelRowAssignment>,
    pub issues: Vec<PanelIssue>,
}

pub async fn compose(state: &AppState, request: ComposePanelRequest) -> Result<ComposePanelResponse, String> {
    let layout = state.layout_state.read().await;
    let Some(layout) = layout.as_ref() else {
        return Err("No layout is open.".to_string());
    };
    let profiles = state.profiles.read().await;
    compose_from_layout(layout, &profiles, request)
}

fn compose_from_layout(
    layout: &LayoutState,
    profiles: &HashMap<String, Option<StructureProfile>>,
    request: ComposePanelRequest,
) -> Result<ComposePanelResponse, String> {
    let facts = proposal_facts_from_layout(layout, profiles, &[]);
    let claimed = claimed_slots_from_facilities(
        layout.effective_facilities(),
        layout.effective_channels(),
        Some(&request.facility_id),
    );
    let occupancy = request.record.entries.iter().map(|e| occupancy_events(&facts, &e.block_id)).collect();

    let mut trees: HashMap<String, &NodeConfigTree> = HashMap::new();
    let mut boards: Vec<PanelBoardFacts> = Vec::new();
    for lamp in &request.lamps {
        if trees.contains_key(&lamp.node_key) {
            continue;
        }
        let key = NodeKey::parse(&lamp.node_key).map_err(|e| format!("Invalid node key '{}': {e}", lamp.node_key))?;
        let host = facts
            .hardware
            .hosts
            .iter()
            .find(|h| NodeKey::parse(&h.node_key).is_ok_and(|k| k == key))
            .filter(|h| h.evidence == SignalHardwareEvidence::Complete);
        let (Some(host), Some(tree)) = (host, facts.trees.get(&key)) else { continue };
        trees.insert(lamp.node_key.clone(), tree);
        boards.push(PanelBoardFacts {
            node_key: lamp.node_key.clone(),
            outputs: host.outputs.clone(),
            direct_lamp_rows: node_slot_facts(&lamp.node_key, tree, &[]).direct_lamps,
            claimed_rows: claimed
                .iter()
                .filter(|c| {
                    c.kind == ClaimedSlotKind::DirectLamp && NodeKey::parse(&c.node_key).is_ok_and(|k| k == key)
                })
                .map(|c| c.index)
                .collect(),
        });
    }
    let indexes: HashMap<String, LeafPathIndex<'_>> =
        trees.iter().map(|(k, t)| (k.clone(), LeafPathIndex::new(t))).collect();

    let PanelCompilation { edits, assignments, issues } = compile_panel(&PanelCompileInput {
        facility_id: request.facility_id,
        record: request.record,
        lamps: request.lamps,
        occupancy,
        boards,
    });
    let edits = resolve_edits(&indexes, &edits).map_err(|e| e.to_string())?;
    Ok(ComposePanelResponse { edits, assignments, issues })
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use bowties_core::panel::compiler::PanelIssueCode;

    fn board_tree() -> bowties_core::node_tree::NodeConfigTree {
        let mut tree = signal_lcc_tree();
        // A panel lamp: row 2 drives pin 3.
        set_int(&mut tree, "Direct Lamp Control/Lamp[2]/Lamp Selection", 3);
        // ABS mast 1 drives pin 5.
        set_int(&mut tree, &mast_processing(1), 1);
        set_int(&mut tree, &mast_lamp(1), 5);
        // An unclaimed Direct Lamp row 4 drives pin 7.
        set_int(&mut tree, "Direct Lamp Control/Lamp[4]/Lamp Selection", 7);
        set_value(
            &mut tree,
            "Direct Lamp Control/Lamp[4]/Lamp Description",
            ConfigValue::String { value: "Depot lamp".into() },
        );
        // An unclaimed mast 2 drives pin 12.
        set_int(&mut tree, &mast_processing(2), 1);
        set_int(&mut tree, &mast_lamp(2), 12);
        tree
    }

    #[test]
    fn wiring_facts_label_every_pin_and_attribute_free_panel_facility_and_outside_owners() {
        let layout = layout(
            board_tree(),
            vec![lamp_channel("lamp-2", 2)],
            vec![panel_facility("pnl-1", "Yard Panel", &["lamp-2"]), abs_facility("abs-1", "East Siding", 1)],
        );
        let out = wiring_facts_from_layout(&layout, &profiles(), &PanelWiringFactsRequest::default()).unwrap();
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        assert_eq!(out.boards.len(), 1);
        let pins = &out.boards[0].pins;
        assert_eq!(pins.len(), 16);
        assert_eq!((pins[0].label.as_str(), pins[0].group.as_str()), ("#1 H1-G", "H1"));
        assert_eq!((pins[15].label.as_str(), pins[15].group.as_str()), ("#16 H4-L", "H4"));
        let owner = |pin: usize| pins[pin - 1].owner.clone();
        assert_eq!(owner(1), PinOwner::Free);
        assert_eq!(owner(3), PinOwner::Panel { facility_id: "pnl-1".into(), name: "Yard Panel".into(), row: 2 });
        assert_eq!(owner(5), PinOwner::Facility { facility_id: "abs-1".into(), name: "East Siding".into() });
        assert_eq!(owner(7), PinOwner::Outside { row: Some(4), description: "Depot lamp".into() });
        assert_eq!(owner(12), PinOwner::Outside { row: None, description: "Mast 2".into() });
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["boards"][0]["pins"][2]["owner"]["kind"], "panel");
        assert_eq!(json["boards"][0]["pins"][2]["owner"]["facilityId"], "pnl-1");
    }

    fn compose_request(pins: [u32; 3]) -> ComposePanelRequest {
        use bowties_core::layout::facilities::{LitWhen, PanelEntry};
        let entry = |id: &str, block: &str| PanelEntry {
            entry_id: id.into(),
            block_id: block.into(),
            lamp_channel_id: None,
            lit_when: None,
        };
        let lamp = |id: &str, pin: u32| PanelLampRequest {
            entry_id: id.into(),
            node_key: BOARD.into(),
            output: pin,
            current_row: None,
            name: format!("Lamp {id}"),
        };
        ComposePanelRequest {
            facility_id: "pnl-new".into(),
            record: PanelRecord {
                lit_when: LitWhen::Occupied,
                entries: vec![entry("e1", "b1"), entry("e2", "b2"), entry("e3", "b3")],
            },
            lamps: vec![lamp("e1", pins[0]), lamp("e2", pins[1]), lamp("e3", pins[2])],
        }
    }

    #[test]
    fn compose_resolves_edits_on_the_rows_of_the_chosen_pins_without_touching_the_layout() {
        let layout = layout(signal_lcc_tree(), vec![], vec![]);
        let before = (
            serde_json::to_value(layout.effective_facilities()).unwrap(),
            serde_json::to_value(layout.effective_channels()).unwrap(),
            layout.pending_changes().len(),
        );
        let out = compose_from_layout(&layout, &profiles(), compose_request([9, 3, 14])).unwrap();
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        let rows: Vec<u32> = out.assignments.iter().map(|a| a.row).collect();
        assert_eq!(rows, vec![9, 3, 14]);
        assert_eq!(out.edits.len(), 12);
        for row in [9, 3, 14] {
            assert!(out.edits.iter().any(|e| e.path == format!("Direct Lamp Control/Lamp[{row}]/Lamp On")));
        }
        assert!(out.edits.iter().all(|e| e.node_key == BOARD && e.space == 253));
        let after = (
            serde_json::to_value(layout.effective_facilities()).unwrap(),
            serde_json::to_value(layout.effective_channels()).unwrap(),
            layout.pending_changes().len(),
        );
        assert_eq!(before, after, "compose is read-only");
    }

    #[test]
    fn compose_reports_a_pin_held_by_an_abs_mast_and_yields_no_edits() {
        let mut tree = signal_lcc_tree();
        set_int(&mut tree, &mast_processing(1), 1);
        set_int(&mut tree, &mast_lamp(1), 9);
        let layout = layout(tree, vec![], vec![abs_facility("abs-1", "East Siding", 1)]);
        let out = compose_from_layout(&layout, &profiles(), compose_request([9, 3, 14])).unwrap();
        assert!(out.edits.is_empty() && out.assignments.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].code, PanelIssueCode::PinOwnedElsewhere);
        assert_eq!(out.issues[0].output, Some(9));
    }
}
