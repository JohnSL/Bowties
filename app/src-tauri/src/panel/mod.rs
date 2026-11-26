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
    panel::{panel_saved_rows, compiler::{
        compile_panel, PanelBoardFacts, PanelCompilation, PanelCompileInput, PanelIssue, PanelIssueCode,
        PanelLampRequest, PanelRowAssignment,
    }},
    profile::StructureProfile,
    signaling::{
        compiler::{merge_release_edits, release_slots, ReleaseSlot, ReleaseTarget},
        slots::{claimed_slots_from_facilities, node_slot_facts, resolve_edits, LeafPathIndex, ResolvedEdit},
        ReleaseRequest, SignalHardwareEvidence, SignalHardwareHost, SignalOutput, SignalOutputOwner,
    },
};
use serde::{Deserialize, Serialize};

use crate::signaling::{index_effective_trees, occupancy_events, proposal_facts_from_layout};
use crate::state::AppState;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelWiringFactsRequest {
    /// Restrict to these boards; every board with Direct Lamp Control when absent.
    #[serde(default)]
    pub node_keys: Option<Vec<String>>,
    /// Outside owners the caller proposes to release; they read as free in the facts.
    #[serde(default)]
    pub release: Vec<ReleaseRequest>,
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
    /// Named releases that cannot be honoured (the owner is claimed or absent).
    pub release_issues: Vec<WiringIssue>,
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
    let facts = proposal_facts_from_layout(layout, profiles, &request.release);
    let wanted: Option<Vec<NodeKey>> = request
        .node_keys
        .as_ref()
        .map(|keys| {
            keys.iter()
                .map(|k| NodeKey::parse(k).map_err(|e| format!("Invalid node key '{k}': {e}")))
                .collect::<Result<_, _>>()
        })
        .transpose()?;
    let mut response = PanelWiringFactsResponse {
        release_issues: facts
            .release_issues
            .iter()
            .map(|i| WiringIssue { node_key: i.node_key.clone().unwrap_or_default(), detail: i.detail.clone() })
            .collect(),
        ..Default::default()
    };
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
    /// Outside owners the caller proposes to release so their pins can be chosen.
    #[serde(default)]
    pub release: Vec<ReleaseRequest>,
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
    let facts = proposal_facts_from_layout(layout, profiles, &request.release);
    if !facts.release_issues.is_empty() {
        return Ok(ComposePanelResponse {
            edits: Vec::new(),
            assignments: Vec::new(),
            issues: facts
                .release_issues
                .iter()
                .map(|issue| PanelIssue {
                    code: PanelIssueCode::ReleaseInvalid,
                    entry_id: None,
                    node_key: issue.node_key.clone(),
                    output: None,
                    detail: issue.detail.clone(),
                })
                .collect(),
        });
    }
    let claimed = claimed_slots_from_facilities(
        layout.effective_facilities(),
        layout.effective_channels(),
        Some(&request.facility_id),
    );
    let saved_rows = layout
        .effective_facilities()
        .facilities
        .iter()
        .find(|f| f.facility_id == request.facility_id)
        .map(|f| panel_saved_rows(f, layout.effective_channels()))
        .unwrap_or_default();
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

    let PanelCompilation { edits, assignments, issues, released } = compile_panel(&PanelCompileInput {
        facility_id: request.facility_id,
        record: request.record,
        lamps: request.lamps,
        saved_rows,
        occupancy,
        boards,
    });
    let mut edits = edits;
    if !released.is_empty() {
        // An edit releases the rows it drops (ADR-0020): reset them through the
        // shared `release_slots`, on boards that may hold no lamp of the new record.
        trees.extend(index_effective_trees(&facts, released.iter().map(|r| &r.node_key))?);
        let mut released_boards: Vec<&str> = released.iter().map(|r| r.node_key.as_str()).collect();
        released_boards.sort_unstable();
        released_boards.dedup();
        let nodes: Vec<_> = released_boards
            .into_iter()
            .filter_map(|board| trees.get(board).map(|tree| node_slot_facts(board, tree, &[])))
            .collect();
        let targets: Vec<ReleaseTarget> = released
            .iter()
            .map(|row| ReleaseTarget { node_key: row.node_key.clone(), slot: ReleaseSlot::DirectLamp(row.row) })
            .collect();
        let reset = release_slots(&targets, &nodes);
        if let Some(issue) = reset.issues.first() {
            return Ok(ComposePanelResponse {
                edits: Vec::new(),
                assignments: Vec::new(),
                issues: vec![PanelIssue {
                    code: PanelIssueCode::ClaimedSlotUnavailable,
                    entry_id: None,
                    node_key: issue.node_key.clone(),
                    output: None,
                    detail: issue.detail.clone(),
                }],
            });
        }
        edits.extend(reset.edits);
    }
    // The request's named releases ride along; the recipe wins on a shared leaf.
    let edits = merge_release_edits(facts.release_edits.clone(), edits);
    for edit in &edits {
        if trees.contains_key(&edit.node_key) {
            continue;
        }
        let key = NodeKey::parse(&edit.node_key).map_err(|e| format!("Invalid node key '{}': {e}", edit.node_key))?;
        if let Some(tree) = facts.trees.get(&key) {
            trees.insert(edit.node_key.clone(), tree);
        }
    }
    let indexes: HashMap<String, LeafPathIndex<'_>> =
        trees.iter().map(|(k, t)| (k.clone(), LeafPathIndex::new(t))).collect();
    let edits = resolve_edits(&indexes, &edits).map_err(|e| e.to_string())?;
    Ok(ComposePanelResponse { edits, assignments, issues })
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use bowties_core::signaling::slots::ResolvedValue;

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

    #[test]
    fn wiring_facts_read_a_released_outside_pin_as_free_and_report_a_claimed_owner_release() {
        let layout = layout(
            board_tree(),
            vec![lamp_channel("lamp-2", 2)],
            vec![panel_facility("pnl-1", "Yard Panel", &["lamp-2"])],
        );
        let release = |row: u32| ReleaseRequest { node_key: BOARD.into(), owner: SignalOutputOwner::DirectLamp(row) };

        let request = PanelWiringFactsRequest { release: vec![release(4)], ..Default::default() };
        let out = wiring_facts_from_layout(&layout, &profiles(), &request).unwrap();
        assert!(out.release_issues.is_empty(), "{:?}", out.release_issues);
        assert_eq!(out.boards[0].pins[6].owner, PinOwner::Free, "pin 7's outside row 4 is released");

        let request = PanelWiringFactsRequest { release: vec![release(2)], ..Default::default() };
        let out = wiring_facts_from_layout(&layout, &profiles(), &request).unwrap();
        assert_eq!(out.release_issues.len(), 1);
        assert_eq!(out.release_issues[0].node_key, BOARD);
        assert_eq!(
            out.boards[0].pins[2].owner,
            PinOwner::Panel { facility_id: "pnl-1".into(), name: "Yard Panel".into(), row: 2 },
            "a claimed owner is not freed"
        );
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["releaseIssues"][0]["nodeKey"], BOARD);
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
            name: format!("Lamp {id}"),
        };
        ComposePanelRequest {
            facility_id: "pnl-new".into(),
            record: PanelRecord {
                lit_when: LitWhen::Occupied,
                entries: vec![entry("e1", "b1"), entry("e2", "b2"), entry("e3", "b3")],
            },
            lamps: vec![lamp("e1", pins[0]), lamp("e2", pins[1]), lamp("e3", pins[2])],
            release: Vec::new(),
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

    fn release_direct_lamp(row: u32) -> ReleaseRequest {
        ReleaseRequest { node_key: BOARD.into(), owner: SignalOutputOwner::DirectLamp(row) }
    }

    #[test]
    fn compose_seats_a_lamp_on_an_outside_pin_only_when_that_owner_is_released() {
        let layout = layout(board_tree(), vec![], vec![]);
        // Pin 7 is driven by outside Direct Lamp row 4 ("Depot lamp").
        let blocked = compose_from_layout(&layout, &profiles(), compose_request([7, 9, 14])).unwrap();
        assert!(blocked.edits.is_empty());
        assert_eq!(blocked.issues.len(), 1);
        assert_eq!(blocked.issues[0].code, PanelIssueCode::PinOutsideNotReleased);
        assert_eq!(blocked.issues[0].output, Some(7));

        let mut request = compose_request([7, 9, 14]);
        request.release = vec![release_direct_lamp(4)];
        let out = compose_from_layout(&layout, &profiles(), request).unwrap();
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        let row = out.assignments[0].row;
        let on = |r: u32| edit_for(&out, BOARD, &format!("Direct Lamp Control/Lamp[{r}]/Lamp On")).is_some();
        assert!(on(row), "the recipe seats the lamp");
        if row != 4 {
            let sel = edit_for(&out, BOARD, "Direct Lamp Control/Lamp[4]/Lamp Selection").expect("row 4 reset");
            assert_eq!(sel.value, ResolvedValue::Config(ConfigValue::Int { value: 0 }));
        }
        let desc = edit_for(&out, BOARD, &format!("Direct Lamp Control/Lamp[{row}]/Lamp Description")).unwrap();
        assert_eq!(desc.value, ResolvedValue::Config(ConfigValue::String { value: "Lamp e1".into() }));
        let mut leaves: Vec<_> = out.edits.iter().map(|e| (e.node_key.clone(), e.path.clone())).collect();
        let total = leaves.len();
        leaves.sort();
        leaves.dedup();
        assert_eq!(leaves.len(), total, "no leaf is edited twice");
    }

    #[test]
    fn compose_refuses_a_release_of_a_row_another_panel_claims() {
        let layout = layout(
            board_tree(),
            vec![lamp_channel("lamp-2", 2)],
            vec![panel_facility("pnl-1", "Yard Panel", &["lamp-2"])],
        );
        let mut request = compose_request([3, 9, 14]);
        request.release = vec![release_direct_lamp(2)];
        let out = compose_from_layout(&layout, &profiles(), request).unwrap();
        assert!(out.edits.is_empty() && out.assignments.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].code, PanelIssueCode::ReleaseInvalid);
        assert_eq!(out.issues[0].node_key.as_deref(), Some(BOARD));
    }

    const OTHER_BOARD: &str = "05010101FF03";

    /// A saved panel `pnl-1` whose entries e1..eN sit on `(board, row)` lamp channels.
    fn saved_panel_layout(lamps: &[(&str, u32)], with_other_board: bool) -> LayoutState {
        use bowties_core::layout::channels::ChannelBinding;
        use bowties_core::layout::facilities::{LitWhen, PanelEntry};
        let mut channels = Vec::new();
        let mut entries = Vec::new();
        for (i, (board, row)) in lamps.iter().enumerate() {
            let mut ch = lamp_channel(&format!("lamp-{i}"), *row);
            ch.binding = ChannelBinding::LampRow { node_key: (*board).into(), row_ordinal: *row };
            channels.push(ch);
            entries.push(PanelEntry {
                entry_id: format!("e{}", i + 1),
                block_id: format!("b{}", i + 1),
                lamp_channel_id: Some(format!("lamp-{i}")),
                lit_when: None,
            });
        }
        let mut facility = panel_facility("pnl-1", "Yard Panel", &[]);
        facility.panel = Some(PanelRecord { lit_when: LitWhen::Occupied, entries });
        let mut layout = layout(signal_lcc_tree(), channels, vec![facility]);
        if with_other_board {
            layout.record_captured(
                NodeKey::parse(OTHER_BOARD).unwrap(),
                bowties_core::layout::state::CapturedNode { tree: Some(signal_lcc_tree()), ..Default::default() },
            );
        }
        layout
    }

    fn recompose(
        layout: &LayoutState,
        lamps: &[(&str, &str, u32)], // (entry id, board, pin)
    ) -> ComposePanelResponse {
        use bowties_core::layout::facilities::{LitWhen, PanelEntry};
        let request = ComposePanelRequest {
            facility_id: "pnl-1".into(),
            record: PanelRecord {
                lit_when: LitWhen::Occupied,
                entries: lamps
                    .iter()
                    .enumerate()
                    .map(|(i, (id, _, _))| PanelEntry {
                        entry_id: (*id).into(),
                        block_id: format!("b{}", i + 1),
                        lamp_channel_id: None,
                        lit_when: None,
                    })
                    .collect(),
            },
            lamps: lamps
                .iter()
                .map(|(id, board, pin)| PanelLampRequest {
                    entry_id: (*id).into(),
                    node_key: (*board).into(),
                    output: *pin,
                    name: format!("Lamp {id}"),
                })
                .collect(),
            release: Vec::new(),
        };
        compose_from_layout(layout, &profiles(), request).unwrap()
    }

    fn edit_for<'a>(out: &'a ComposePanelResponse, board: &str, path: &str) -> Option<&'a ResolvedEdit> {
        out.edits.iter().find(|e| e.node_key == board && e.path == path)
    }

    #[test]
    fn recompose_with_an_entry_removed_resets_its_row_and_recipes_only_the_kept_row() {
        let layout = saved_panel_layout(&[(BOARD, 2), (BOARD, 6)], false);
        let out = recompose(&layout, &[("e1", BOARD, 2)]);
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        assert_eq!(out.assignments.len(), 1);
        assert_eq!(out.assignments[0].row, 2);
        let sel = edit_for(&out, BOARD, "Direct Lamp Control/Lamp[6]/Lamp Selection").expect("row 6 reset");
        assert_eq!(sel.value, ResolvedValue::Config(ConfigValue::Int { value: 0 }));
        assert!(edit_for(&out, BOARD, "Direct Lamp Control/Lamp[6]/Lamp On").is_some());
        assert!(edit_for(&out, BOARD, "Direct Lamp Control/Lamp[2]/Lamp On").is_some());
        assert_eq!(out.edits.len(), 8, "4 recipe edits on row 2 + 4 reset edits on row 6");
    }

    #[test]
    fn recompose_with_a_lamp_moved_to_another_board_resets_the_old_row_and_allocates_on_the_new_board() {
        let layout = saved_panel_layout(&[(BOARD, 2)], true);
        let out = recompose(&layout, &[("e1", OTHER_BOARD, 5)]);
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        assert_eq!(out.assignments[0].node_key, OTHER_BOARD);
        assert!(edit_for(&out, BOARD, "Direct Lamp Control/Lamp[2]/Lamp Description").is_some());
        let row = out.assignments[0].row;
        assert!(edit_for(&out, OTHER_BOARD, &format!("Direct Lamp Control/Lamp[{row}]/Lamp On")).is_some());
        assert_eq!(out.edits.len(), 8);
    }

    #[test]
    fn recompose_with_a_same_board_pin_change_resets_nothing() {
        let layout = saved_panel_layout(&[(BOARD, 2)], false);
        let out = recompose(&layout, &[("e1", BOARD, 9)]);
        assert!(out.issues.is_empty(), "{:?}", out.issues);
        assert_eq!(out.assignments[0].row, 2);
        let sel = edit_for(&out, BOARD, "Direct Lamp Control/Lamp[2]/Lamp Selection").unwrap();
        assert_eq!(sel.value, ResolvedValue::Config(ConfigValue::Int { value: 9 }));
        assert_eq!(out.edits.len(), 4, "only the kept row's recipe");
    }

    #[test]
    fn recompose_blocks_when_a_dropped_rows_board_is_not_loaded() {
        let layout = saved_panel_layout(&[(BOARD, 2), (OTHER_BOARD, 3)], false);
        let out = recompose(&layout, &[("e1", BOARD, 2)]);
        assert!(out.edits.is_empty() && out.assignments.is_empty());
        assert_eq!(out.issues.len(), 1);
        assert_eq!(out.issues[0].code, PanelIssueCode::ClaimedSlotUnavailable);
        assert_eq!(out.issues[0].node_key.as_deref(), Some(OTHER_BOARD));
    }
}
