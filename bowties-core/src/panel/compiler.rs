//! Pure compiler for a panel facility: turns its entries, lamp requests,
//! block occupancy events and board facts into Direct Lamp Control edits
//! (Spec 021). No I/O and no CDI addresses; edits are addressed by CDI path
//! like the Automatic Block Signals compiler's.

use serde::{Deserialize, Serialize};

use crate::layout::facilities::{canonical_node_key, LitWhen, PanelRecord};
use crate::signaling::compiler::{direct_lamp_path, BlockOccupancyEvents, EditValue, NodeConfigEdit};
use crate::signaling::{SignalOutput, SignalOutputOwner};

/// A lamp the panel wants lit for one entry: the board pin it drives and the
/// Direct Lamp row it already holds, if any.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelLampRequest {
    pub entry_id: String,
    pub node_key: String,
    /// 1-based physical output pin.
    pub output: u32,
    /// The row the lamp channel is bound to; kept when present.
    pub current_row: Option<u32>,
    pub name: String,
}

/// What the compiler needs to know about one Signal LCC.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelBoardFacts {
    pub node_key: String,
    /// Outputs with owners attributed to facilities.
    pub outputs: Vec<SignalOutput>,
    /// 1-based Direct Lamp rows present on the node.
    pub direct_lamp_rows: Vec<u32>,
    /// Rows claimed by facilities other than this panel.
    #[serde(default)]
    pub claimed_rows: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelCompileInput {
    pub facility_id: String,
    pub record: PanelRecord,
    pub lamps: Vec<PanelLampRequest>,
    pub occupancy: Vec<BlockOccupancyEvents>,
    pub boards: Vec<PanelBoardFacts>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelRowAssignment {
    pub entry_id: String,
    pub node_key: String,
    pub row: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PanelIssueCode {
    PinOwnedElsewhere,
    PinOutsideNotReleased,
    DuplicatePin,
    BlockEventsUnresolved,
    BoardUnavailable,
    NoFreeLampRow,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelIssue {
    pub code: PanelIssueCode,
    pub entry_id: Option<String>,
    pub node_key: Option<String>,
    pub output: Option<u32>,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelCompilation {
    pub edits: Vec<NodeConfigEdit>,
    pub assignments: Vec<PanelRowAssignment>,
    pub issues: Vec<PanelIssue>,
}

/// Compile a panel's lamps. With any issue, `edits` and `assignments` are empty.
pub fn compile_panel(input: &PanelCompileInput) -> PanelCompilation {
    let mut out = PanelCompilation::default();
    let mut rows = RowAllocator::new(input);
    let mut assigned: Vec<Option<u32>> = vec![None; input.lamps.len()];
    let valid: Vec<bool> = (0..input.lamps.len()).map(|i| check_lamp(input, i, &mut out.issues)).collect();
    // Lamps that already hold a row keep it; only then do others pick one.
    for (i, lamp) in input.lamps.iter().enumerate().filter(|(i, _)| valid[*i]) {
        if let Some(row) = lamp.current_row {
            rows.reserve(&lamp.node_key, row);
            assigned[i] = Some(row);
        }
    }
    for (i, lamp) in input.lamps.iter().enumerate().filter(|(i, _)| valid[*i]) {
        if assigned[i].is_some() {
            continue;
        }
        match rows.allocate(&lamp.node_key, lamp.output) {
            Some(row) => assigned[i] = Some(row),
            None => out.issues.push(lamp_issue(
                PanelIssueCode::NoFreeLampRow,
                lamp,
                "The Signal LCC has no free Direct Lamp row for this lamp.",
            )),
        }
    }
    if !out.issues.is_empty() {
        return out;
    }
    for (lamp, row) in input.lamps.iter().zip(assigned) {
        let Some(row) = row else { continue };
        let (on, off) = lamp_events(input, lamp).expect("validated lamp has events");
        let path = direct_lamp_path(row);
        let mut push = |leaf: &str, value: EditValue| {
            out.edits.push(NodeConfigEdit { node_key: lamp.node_key.clone(), path: format!("{path}/{leaf}"), value });
        };
        push("Lamp Selection", EditValue::Int { value: i64::from(lamp.output) });
        push("Lamp On", EditValue::EventId { bytes: on });
        push("Lamp Off", EditValue::EventId { bytes: off });
        push("Lamp Description", EditValue::String { value: lamp.name.clone() });
        out.assignments.push(PanelRowAssignment { entry_id: lamp.entry_id.clone(), node_key: lamp.node_key.clone(), row });
    }
    group_edits_by_node(&mut out.edits);
    out
}

fn lamp_issue(code: PanelIssueCode, lamp: &PanelLampRequest, detail: &str) -> PanelIssue {
    PanelIssue {
        code,
        entry_id: Some(lamp.entry_id.clone()),
        node_key: Some(lamp.node_key.clone()),
        output: Some(lamp.output),
        detail: detail.into(),
    }
}

/// The lamp's (On, Off) events: its block's occupied/clear events, swapped
/// when the entry's effective lit-when is Clear. `None` when either event of
/// the block, or the entry itself, is missing.
fn lamp_events(input: &PanelCompileInput, lamp: &PanelLampRequest) -> Option<([u8; 8], [u8; 8])> {
    let entry = input.record.entries.iter().find(|e| e.entry_id == lamp.entry_id)?;
    let events = input.occupancy.iter().find(|o| o.block_id == entry.block_id)?;
    let (occupied, clear) = (events.occupied?, events.clear?);
    Some(match entry.lit_when.unwrap_or(input.record.lit_when) {
        LitWhen::Occupied => (occupied, clear),
        LitWhen::Clear => (clear, occupied),
    })
}

/// Report every blocking problem of lamp `index`; true when it has none.
fn check_lamp(input: &PanelCompileInput, index: usize, issues: &mut Vec<PanelIssue>) -> bool {
    let lamp = &input.lamps[index];
    let before = issues.len();
    if lamp_events(input, lamp).is_none() {
        issues.push(lamp_issue(
            PanelIssueCode::BlockEventsUnresolved,
            lamp,
            "The block's occupied and clear events are not both known.",
        ));
    }
    let key = canonical_node_key(&lamp.node_key);
    let Some(board) = input.boards.iter().find(|b| canonical_node_key(&b.node_key) == key) else {
        issues.push(lamp_issue(PanelIssueCode::BoardUnavailable, lamp, "The Signal LCC is not available."));
        return false;
    };
    let repeats = input.lamps[..index]
        .iter()
        .any(|other| canonical_node_key(&other.node_key) == key && other.output == lamp.output);
    if repeats {
        issues.push(lamp_issue(PanelIssueCode::DuplicatePin, lamp, "Another lamp of this panel uses the same output."));
    }
    if let Some(output) = board.outputs.iter().find(|o| o.ordinal == lamp.output) {
        let own_row = matches!(output.owner, Some(SignalOutputOwner::DirectLamp(row)) if lamp.current_row == Some(row));
        let own_facility = output.facility.as_ref().is_some_and(|f| f.facility_id == input.facility_id);
        if output.owner.is_some() && !own_row && !own_facility {
            let (code, detail) = match (output.owner, &output.facility) {
                (Some(SignalOutputOwner::DirectLamp(_)), None) => (
                    PanelIssueCode::PinOutsideNotReleased,
                    "A Direct Lamp row configured outside Bowties uses this output; release it first.",
                ),
                _ => (PanelIssueCode::PinOwnedElsewhere, "Another facility or mast uses this output."),
            };
            issues.push(lamp_issue(code, lamp, detail));
        }
    }
    issues.len() == before
}

/// Stable-group edits so each node's edits are contiguous, nodes in order of first appearance.
fn group_edits_by_node(edits: &mut [NodeConfigEdit]) {
    let mut order: Vec<String> = Vec::new();
    for edit in edits.iter() {
        if !order.contains(&edit.node_key) {
            order.push(edit.node_key.clone());
        }
    }
    edits.sort_by_key(|e| order.iter().position(|k| *k == e.node_key));
}

/// Tracks which Direct Lamp rows are taken on each board while rows are chosen.
struct RowAllocator<'a> {
    boards: &'a [PanelBoardFacts],
    /// `(canonical node key, row)` taken by an owner, another facility or this compile.
    taken: Vec<(String, u32)>,
}

impl<'a> RowAllocator<'a> {
    fn new(input: &'a PanelCompileInput) -> Self {
        let mut taken = Vec::new();
        for board in &input.boards {
            let key = canonical_node_key(&board.node_key);
            let owned = board.outputs.iter().filter_map(|o| match o.owner {
                Some(SignalOutputOwner::DirectLamp(row)) => Some(row),
                _ => None,
            });
            taken.extend(owned.chain(board.claimed_rows.iter().copied()).map(|row| (key.clone(), row)));
        }
        Self { boards: &input.boards, taken }
    }

    fn board(&self, node_key: &str) -> Option<&'a PanelBoardFacts> {
        let key = canonical_node_key(node_key);
        self.boards.iter().find(|b| canonical_node_key(&b.node_key) == key)
    }

    fn is_taken(&self, node_key: &str, row: u32) -> bool {
        let key = canonical_node_key(node_key);
        self.taken.iter().any(|(k, r)| *k == key && *r == row)
    }

    fn reserve(&mut self, node_key: &str, row: u32) {
        self.taken.push((canonical_node_key(node_key), row));
    }

    /// The row equal to `pin` when free, else the lowest free row.
    fn allocate(&mut self, node_key: &str, pin: u32) -> Option<u32> {
        let board = self.board(node_key)?;
        let free = |row: u32| !self.is_taken(node_key, row);
        let row = if board.direct_lamp_rows.contains(&pin) && free(pin) {
            Some(pin)
        } else {
            board.direct_lamp_rows.iter().copied().filter(|r| free(*r)).min()
        }?;
        self.reserve(node_key, row);
        Some(row)
    }
}
