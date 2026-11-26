use std::collections::HashMap;

use bowties_core::layout::facilities::{LitWhen, PanelEntry, PanelRecord};
use bowties_core::node_tree::build_node_config_tree;
use bowties_core::panel::compiler::*;
use bowties_core::signaling::compiler::{BlockOccupancyEvents, EditValue, NodeConfigEdit};
use bowties_core::signaling::slots::{resolve_edits, LeafPathIndex};
use bowties_core::signaling::{OutputFacility, SignalOutput, SignalOutputOwner};
use lcc_rs::cdi::parser::parse_cdi;

const CDI: &str = include_str!("../../docs/ref/RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml");
const NODE: &str = "050101010700";
const PANEL: &str = "panel:main";

fn occ(n: u8) -> [u8; 8] {
    [1, 1, 1, 1, 1, 1, n, 1]
}
fn clr(n: u8) -> [u8; 8] {
    [1, 1, 1, 1, 1, 1, n, 2]
}

fn free_outputs() -> Vec<SignalOutput> {
    (1..=16).map(|ordinal| SignalOutput { ordinal, label: format!("#{ordinal}"), owner: None, facility: None }).collect()
}

fn board(node: &str) -> PanelBoardFacts {
    PanelBoardFacts { node_key: node.into(), outputs: free_outputs(), direct_lamp_rows: (1..=16).collect(), claimed_rows: vec![] }
}

fn entry(id: &str, block: &str) -> PanelEntry {
    PanelEntry { entry_id: id.into(), block_id: block.into(), lamp_channel_id: None, lit_when: None }
}

fn lamp(entry_id: &str, pin: u32, current_row: Option<u32>) -> PanelLampRequest {
    PanelLampRequest {
        entry_id: entry_id.into(),
        node_key: NODE.into(),
        output: pin,
        current_row,
        name: format!("Lamp {entry_id}"),
    }
}

fn events(block: &str, n: u8) -> BlockOccupancyEvents {
    BlockOccupancyEvents { block_id: block.into(), occupied: Some(occ(n)), clear: Some(clr(n)) }
}

fn input(lamps: Vec<PanelLampRequest>) -> PanelCompileInput {
    PanelCompileInput {
        facility_id: PANEL.into(),
        record: PanelRecord {
            lit_when: LitWhen::Occupied,
            entries: vec![entry("e1", "b1"), entry("e2", "b2"), entry("e3", "b3")],
        },
        lamps,
        occupancy: vec![events("b1", 1), events("b2", 2), events("b3", 3)],
        boards: vec![board(NODE)],
    }
}

fn edit<'a>(edits: &'a [NodeConfigEdit], path: &str) -> &'a EditValue {
    &edits.iter().find(|e| e.path == path).unwrap_or_else(|| panic!("no edit for {path}")).value
}

fn assert_resolves(edits: &[NodeConfigEdit]) {
    let tree = build_node_config_tree(NODE, &parse_cdi(CDI).expect("CDI parses"));
    let mut indexes = HashMap::new();
    indexes.insert(NODE.to_string(), LeafPathIndex::new(&tree));
    resolve_edits(&indexes, edits).expect("every edit resolves against the real CDI");
}

#[test]
fn three_blocks_wired_to_pins_edit_only_their_rows() {
    let out = compile_panel(&input(vec![lamp("e1", 9, None), lamp("e2", 3, None), lamp("e3", 14, None)]));
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    let rows: Vec<u32> = out.assignments.iter().map(|a| a.row).collect();
    assert_eq!(rows, vec![9, 3, 14]);
    assert_eq!(out.edits.len(), 12);
    for (row, pin, n) in [(9u32, 9i64, 1u8), (3, 3, 2), (14, 14, 3)] {
        let p = format!("Direct Lamp Control/Lamp[{row}]");
        assert_eq!(edit(&out.edits, &format!("{p}/Lamp Selection")), &EditValue::Int { value: pin });
        assert_eq!(edit(&out.edits, &format!("{p}/Lamp On")), &EditValue::EventId { bytes: occ(n) });
        assert_eq!(edit(&out.edits, &format!("{p}/Lamp Off")), &EditValue::EventId { bytes: clr(n) });
    }
    assert_eq!(
        edit(&out.edits, "Direct Lamp Control/Lamp[9]/Lamp Description"),
        &EditValue::String { value: "Lamp e1".into() }
    );
    assert_resolves(&out.edits);
}

#[test]
fn clear_lit_when_swaps_on_and_off_and_entry_override_wins() {
    let mut i = input(vec![lamp("e1", 9, None), lamp("e2", 3, None)]);
    i.record.lit_when = LitWhen::Clear;
    i.record.entries[1].lit_when = Some(LitWhen::Occupied);
    let out = compile_panel(&i);
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    assert_eq!(edit(&out.edits, "Direct Lamp Control/Lamp[9]/Lamp On"), &EditValue::EventId { bytes: clr(1) });
    assert_eq!(edit(&out.edits, "Direct Lamp Control/Lamp[9]/Lamp Off"), &EditValue::EventId { bytes: occ(1) });
    assert_eq!(edit(&out.edits, "Direct Lamp Control/Lamp[3]/Lamp On"), &EditValue::EventId { bytes: occ(2) });
}

fn owned_by_outside_row(b: &mut PanelBoardFacts, row: u32, pin: u32) {
    b.outputs[(pin - 1) as usize].owner = Some(SignalOutputOwner::DirectLamp(row));
}

#[test]
fn lamp_keeps_its_current_row_even_when_row_differs_from_pin() {
    let mut i = input(vec![lamp("e1", 5, Some(2))]);
    owned_by_outside_row(&mut i.boards[0], 2, 5);
    i.boards[0].outputs[4].facility = Some(OutputFacility { facility_id: PANEL.into(), name: "Panel".into() });
    let out = compile_panel(&i);
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    assert_eq!(out.assignments[0].row, 2);
    assert_eq!(edit(&out.edits, "Direct Lamp Control/Lamp[2]/Lamp Selection"), &EditValue::Int { value: 5 });
}

#[test]
fn row_matching_pin_taken_falls_back_to_lowest_free_row() {
    let mut i = input(vec![lamp("e1", 9, None)]);
    // Row 9 selects pin 4 outside Bowties; row 1 is the lowest free row.
    owned_by_outside_row(&mut i.boards[0], 9, 4);
    let out = compile_panel(&i);
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    assert_eq!(out.assignments[0].row, 1);
}

#[test]
fn rows_all_in_use_reports_no_free_lamp_row() {
    let mut i = input(vec![lamp("e1", 9, None)]);
    i.boards[0].direct_lamp_rows = vec![1, 2];
    i.boards[0].claimed_rows = vec![1];
    owned_by_outside_row(&mut i.boards[0], 2, 4);
    let out = compile_panel(&i);
    assert_eq!(out.issues.len(), 1, "{:?}", out.issues);
    assert_eq!(out.issues[0].code, PanelIssueCode::NoFreeLampRow);
    assert!(out.edits.is_empty() && out.assignments.is_empty());
}

fn codes(out: &PanelCompilation) -> Vec<PanelIssueCode> {
    out.issues.iter().map(|i| i.code).collect()
}

fn assert_nothing_compiled(out: &PanelCompilation) {
    assert!(out.edits.is_empty() && out.assignments.is_empty(), "{out:?}");
}

#[test]
fn pin_owned_by_another_facility_blocks() {
    let other = Some(OutputFacility { facility_id: "abs:other".into(), name: "Main".into() });
    for owner in [SignalOutputOwner::Mast(1), SignalOutputOwner::DirectLamp(7)] {
        let mut i = input(vec![lamp("e1", 9, None)]);
        i.boards[0].outputs[8].owner = Some(owner);
        i.boards[0].outputs[8].facility = other.clone();
        let out = compile_panel(&i);
        assert_eq!(codes(&out), vec![PanelIssueCode::PinOwnedElsewhere]);
        assert_eq!(out.issues[0].output, Some(9));
        assert_nothing_compiled(&out);
    }
}

#[test]
fn pin_owned_by_unclaimed_lamp_row_needs_release() {
    let mut i = input(vec![lamp("e1", 9, None)]);
    owned_by_outside_row(&mut i.boards[0], 4, 9);
    let out = compile_panel(&i);
    assert_eq!(codes(&out), vec![PanelIssueCode::PinOutsideNotReleased]);
    assert_nothing_compiled(&out);
}

#[test]
fn pin_owned_by_this_panels_own_row_is_not_elsewhere() {
    let mut i = input(vec![lamp("e1", 9, None)]);
    owned_by_outside_row(&mut i.boards[0], 4, 9);
    i.boards[0].outputs[8].facility = Some(OutputFacility { facility_id: PANEL.into(), name: "Panel".into() });
    let out = compile_panel(&i);
    assert!(out.issues.is_empty(), "{:?}", out.issues);
}

#[test]
fn same_pin_twice_on_a_node_is_duplicate() {
    let out = compile_panel(&input(vec![lamp("e1", 9, None), lamp("e2", 9, None)]));
    assert_eq!(codes(&out), vec![PanelIssueCode::DuplicatePin]);
    assert_eq!(out.issues[0].entry_id.as_deref(), Some("e2"));
    assert_nothing_compiled(&out);
}

#[test]
fn block_without_both_events_is_unresolved() {
    let mut i = input(vec![lamp("e1", 9, None)]);
    i.occupancy[0].clear = None;
    let out = compile_panel(&i);
    assert_eq!(codes(&out), vec![PanelIssueCode::BlockEventsUnresolved]);
    assert_nothing_compiled(&out);
}

#[test]
fn lamp_on_a_node_without_facts_is_board_unavailable() {
    let mut i = input(vec![lamp("e1", 9, None)]);
    i.boards.clear();
    let out = compile_panel(&i);
    assert_eq!(codes(&out), vec![PanelIssueCode::BoardUnavailable]);
    assert_nothing_compiled(&out);
}

#[test]
fn edits_for_two_boards_are_grouped_per_node() {
    const OTHER: &str = "050101010701";
    let mut i = input(vec![lamp("e1", 9, None), lamp("e2", 3, None), lamp("e3", 5, None)]);
    i.lamps[1].node_key = OTHER.into();
    i.boards.push(board(OTHER));
    let out = compile_panel(&i);
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    let nodes: Vec<&str> = out.edits.iter().map(|e| e.node_key.as_str()).collect();
    assert_eq!(nodes.iter().filter(|n| **n == NODE).count(), 8);
    let switches = nodes.windows(2).filter(|w| w[0] != w[1]).count();
    assert_eq!(switches, 1, "edits are contiguous per node: {nodes:?}");
}
