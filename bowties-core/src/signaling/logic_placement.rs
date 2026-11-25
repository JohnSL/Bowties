//! Pure planner for where an Automatic Block Signals facility's Logic groups
//! live on one Signal-LCC board (Spec 020 / E3a).
//!
//! # The invariant this protects
//!
//! Logic placement is **board-wide**, not per facility
//! (`product/hardware/signal-lcc/conditionals.md`, "node-wide structural
//! invariant"): every active group must use Function values the current CDI
//! defines, and every run of `Group` entries must reach a valid `Last (Single)`
//! terminator. An undefined Function value or an unterminated run anywhere on
//! the board can silence a later, otherwise valid group. The planner therefore
//! refuses to place anything next to Logic it cannot prove well-formed, and it
//! never moves or overwrites groups that belong to another facility.
//!
//! The planner is deterministic and does no I/O. Its caller supplies the board's
//! Logic Function facts, the board's whole recorded placement, and this
//! facility's demand; it gets back the board's whole new placement or the
//! reasons it cannot give one.
//!
//! A recorded group whose demanded length changed is resized: it grows in
//! place when the Logic after it is free, otherwise it moves to the lowest free
//! run (its own old slots count as free), and it shrinks keeping its first
//! Logic. Another facility's group never moves. `vacated` lists the Logic this
//! facility held before and no longer holds, so the caller can release it after
//! the new groups are written.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::layout::facilities::PlacedLogicGroup;

/// Signal-LCC C7c CDI Logic Function value: Blocked (the slot is unused).
pub const FUNCTION_BLOCKED: i64 = 0;
/// Signal-LCC C7c CDI Logic Function value: Group (continues to the next Logic).
pub const FUNCTION_GROUP: i64 = 1;
/// Signal-LCC C7c CDI Logic Function value: Last (Single), which ends a group.
pub const FUNCTION_LAST: i64 = 2;

/// One Logic slot of the board and its current Function (`None` = not read).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicSlotFact {
    /// 1-based Logic number.
    pub index: u32,
    pub function: Option<i64>,
}

/// A group this facility wants on the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicGroupDemand {
    pub signal_id: String,
    pub length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogicPlacementIssueCode {
    /// A Logic Bowties does not own has a Function the CDI does not define.
    UndefinedFunction,
    /// A run of `Group` Logic outside Bowties' groups is not ended by `Last (Single)`.
    UnterminatedGroup,
    /// No run of consecutive free Logic is long enough.
    NoFreeLogic,
    /// A Logic's Function has not been read.
    LogicUnread,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicPlacementIssue {
    pub code: LogicPlacementIssueCode,
    /// The Logic numbers the issue names (empty when none applies).
    pub logic: Vec<u32>,
    /// The signal a `NoFreeLogic` issue is about; `None` for board-wide issues.
    pub signal_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedLogic {
    /// The board's whole new placement, ordered by `first`. Empty when `issues` is not.
    pub placement: Vec<PlacedLogicGroup>,
    pub issues: Vec<LogicPlacementIssue>,
    /// Logic this facility's groups held before and no longer hold (moves,
    /// shrinks and dropped groups), ascending. Empty when `issues` is not.
    pub vacated: Vec<u32>,
}

fn slots_of(group: &PlacedLogicGroup) -> impl Iterator<Item = u32> {
    group.first..group.first + group.length
}

fn issue(code: LogicPlacementIssueCode, logic: Vec<u32>, detail: String) -> LogicPlacementIssue {
    LogicPlacementIssue { code, logic, signal_id: None, detail }
}

fn no_free_logic(length: u32, signal_id: &str) -> LogicPlacementIssue {
    LogicPlacementIssue {
        code: LogicPlacementIssueCode::NoFreeLogic,
        logic: Vec::new(),
        signal_id: Some(signal_id.to_string()),
        detail: format!("No run of {length} free Logic for signal {signal_id}."),
    }
}

/// Plan this facility's Logic groups on one board.
pub fn plan_logic_placement(
    logic: &[LogicSlotFact],
    recorded: &[PlacedLogicGroup],
    facility_id: &str,
    demand: &[LogicGroupDemand],
) -> PlannedLogic {
    let function: BTreeMap<u32, Option<i64>> = logic.iter().map(|s| (s.index, s.function)).collect();
    let mut issues = Vec::new();

    // Slots owned by any recorded group are Bowties' own; everything else is
    // outside Logic whose structure must be proven well formed.
    let owned: BTreeSet<u32> = recorded.iter().flat_map(slots_of).collect();

    let unread: Vec<u32> = function.iter().filter(|(_, f)| f.is_none()).map(|(i, _)| *i).collect();
    if !unread.is_empty() {
        issues.push(issue(
            LogicPlacementIssueCode::LogicUnread,
            unread,
            "Read the board's Logic before placing groups.".to_string(),
        ));
    }

    let mut run: Vec<u32> = Vec::new();
    let flush = |run: &mut Vec<u32>, issues: &mut Vec<LogicPlacementIssue>| {
        if !run.is_empty() {
            issues.push(issue(
                LogicPlacementIssueCode::UnterminatedGroup,
                std::mem::take(run),
                "A run of Group Logic is not ended by Last (Single).".to_string(),
            ));
        }
    };
    for (&index, &f) in &function {
        if owned.contains(&index) {
            flush(&mut run, &mut issues);
            continue;
        }
        match f {
            None | Some(FUNCTION_BLOCKED) => flush(&mut run, &mut issues),
            Some(FUNCTION_GROUP) => run.push(index),
            Some(FUNCTION_LAST) => run.clear(),
            Some(other) => {
                run.clear();
                issues.push(issue(
                    LogicPlacementIssueCode::UndefinedFunction,
                    vec![index],
                    format!("Logic {index} has Function value {other}, which the CDI does not define."),
                ));
            }
        }
    }
    flush(&mut run, &mut issues);

    // Occupied: other facilities' groups and this facility's groups that keep
    // their length. This facility's groups for undemanded signals are dropped
    // and their slots become free; a group whose length changed is resized below.
    let mut occupied: BTreeSet<u32> = BTreeSet::new();
    let mut dropped: BTreeSet<u32> = BTreeSet::new();
    let mut placement: Vec<PlacedLogicGroup> = Vec::new();
    let mut handled: BTreeSet<&str> = BTreeSet::new();
    let mut resizing: Vec<(&PlacedLogicGroup, u32)> = Vec::new();
    let mut previously_held: BTreeSet<u32> = BTreeSet::new();
    for group in recorded {
        if group.facility_id != facility_id {
            occupied.extend(slots_of(group));
            placement.push(group.clone());
            continue;
        }
        previously_held.extend(slots_of(group));
        match demand.iter().find(|d| d.signal_id == group.signal_id && d.length > 0) {
            Some(d) if d.length == group.length => {
                occupied.extend(slots_of(group));
                placement.push(group.clone());
                handled.insert(d.signal_id.as_str());
            }
            Some(d) => {
                handled.insert(d.signal_id.as_str());
                resizing.push((group, d.length));
            }
            None => dropped.extend(slots_of(group)),
        }
    }

    let top = function.keys().next_back().copied().unwrap_or(0);
    let is_free = |index: u32, occupied: &BTreeSet<u32>, own: &BTreeSet<u32>| {
        !occupied.contains(&index)
            && (own.contains(&index)
                || dropped.contains(&index)
                || function.get(&index) == Some(&Some(FUNCTION_BLOCKED)))
    };
    let run_free = |first: u32, length: u32, occupied: &BTreeSet<u32>, own: &BTreeSet<u32>| {
        (first..first + length).all(|i| is_free(i, occupied, own))
    };

    // A resized group grows in place when its slots are free, otherwise moves
    // (its own old slots count as free); a shrink keeps its first slot.
    let mut resized: Vec<PlacedLogicGroup> = Vec::new();
    for (group, length) in resizing {
        let own: BTreeSet<u32> = slots_of(group).collect();
        let first = if length < group.length || run_free(group.first, length, &occupied, &own) {
            Some(group.first)
        } else {
            (1..=top).find(|&first| run_free(first, length, &occupied, &own))
        };
        match first {
            Some(first) => {
                let moved = PlacedLogicGroup { first, length, ..group.clone() };
                occupied.extend(slots_of(&moved));
                resized.push(moved);
            }
            None => issues.push(no_free_logic(length, &group.signal_id)),
        }
    }
    placement.extend(resized);

    let none = BTreeSet::new();
    for d in demand {
        if !handled.insert(d.signal_id.as_str()) || d.length == 0 {
            continue;
        }
        let start = (1..=top).find(|&first| run_free(first, d.length, &occupied, &none));
        match start {
            Some(first) => {
                let group = PlacedLogicGroup {
                    facility_id: facility_id.to_string(),
                    signal_id: d.signal_id.clone(),
                    first,
                    length: d.length,
                };
                occupied.extend(slots_of(&group));
                placement.push(group);
            }
            None => issues.push(no_free_logic(d.length, &d.signal_id)),
        }
    }

    if !issues.is_empty() {
        return PlannedLogic { placement: Vec::new(), issues, vacated: Vec::new() };
    }
    let held_now: BTreeSet<u32> =
        placement.iter().filter(|g| g.facility_id == facility_id).flat_map(slots_of).collect();
    let vacated = previously_held.difference(&held_now).copied().collect();
    placement.sort_by_key(|g| g.first);
    PlannedLogic { placement, issues, vacated }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots(functions: &[Option<i64>]) -> Vec<LogicSlotFact> {
        functions
            .iter()
            .enumerate()
            .map(|(i, f)| LogicSlotFact { index: i as u32 + 1, function: *f })
            .collect()
    }

    fn blocked(n: usize) -> Vec<LogicSlotFact> {
        slots(&vec![Some(FUNCTION_BLOCKED); n])
    }

    fn group(facility: &str, signal: &str, first: u32, length: u32) -> PlacedLogicGroup {
        PlacedLogicGroup {
            facility_id: facility.to_string(),
            signal_id: signal.to_string(),
            first,
            length,
        }
    }

    fn want(signal: &str, length: u32) -> LogicGroupDemand {
        LogicGroupDemand { signal_id: signal.to_string(), length }
    }

    fn codes(planned: &PlannedLogic) -> Vec<LogicPlacementIssueCode> {
        planned.issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn a_demanded_signal_takes_the_lowest_free_run() {
        let mut logic = blocked(8);
        logic[0].function = Some(FUNCTION_GROUP);
        logic[1].function = Some(FUNCTION_LAST); // outside, terminated group at 1-2
        let planned = plan_logic_placement(&logic, &[], "f1", &[want("s0", 2), want("s1", 3)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("f1", "s0", 3, 2), group("f1", "s1", 5, 3)]);
    }

    #[test]
    fn a_recorded_group_stays_put_on_recompile() {
        let mut logic = blocked(6);
        logic[3].function = Some(FUNCTION_GROUP);
        logic[4].function = Some(FUNCTION_LAST);
        let recorded = [group("f1", "s0", 4, 2)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("s0", 2), want("s1", 1)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("f1", "s1", 1, 1), group("f1", "s0", 4, 2)]);
    }

    #[test]
    fn other_facilities_groups_are_kept_unchanged_and_not_reused() {
        let mut logic = blocked(6);
        logic[0].function = Some(FUNCTION_GROUP);
        logic[1].function = Some(FUNCTION_LAST);
        let recorded = [group("other", "x", 1, 2)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("s0", 2)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("other", "x", 1, 2), group("f1", "s0", 3, 2)]);
    }

    #[test]
    fn recorded_groups_for_undemanded_signals_are_dropped_and_their_slots_reused() {
        let mut logic = blocked(4);
        logic[0].function = Some(FUNCTION_GROUP);
        logic[1].function = Some(FUNCTION_LAST);
        let recorded = [group("f1", "old", 1, 2)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("new", 2)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("f1", "new", 1, 2)]);

        let none = plan_logic_placement(&logic, &recorded, "f1", &[]);
        assert!(none.issues.is_empty());
        assert!(none.placement.is_empty());
    }

    #[test]
    fn an_undefined_function_blocks_and_names_the_slot() {
        let mut logic = blocked(5);
        logic[1].function = Some(3);
        let planned = plan_logic_placement(&logic, &[], "f1", &[want("s0", 1)]);
        assert!(planned.placement.is_empty());
        assert_eq!(codes(&planned), vec![LogicPlacementIssueCode::UndefinedFunction]);
        assert_eq!(planned.issues[0].logic, vec![2]);
    }

    #[test]
    fn an_unterminated_group_run_blocks_and_names_its_slots() {
        let mut logic = blocked(6);
        logic[1].function = Some(FUNCTION_GROUP);
        logic[2].function = Some(FUNCTION_GROUP); // then Blocked at 4
        let planned = plan_logic_placement(&logic, &[], "f1", &[want("s0", 1)]);
        assert!(planned.placement.is_empty());
        assert_eq!(codes(&planned), vec![LogicPlacementIssueCode::UnterminatedGroup]);
        assert_eq!(planned.issues[0].logic, vec![2, 3]);
    }

    #[test]
    fn a_group_run_ended_by_a_bowties_slot_or_the_end_is_unterminated() {
        let mut logic = blocked(4);
        logic[0].function = Some(FUNCTION_GROUP);
        logic[2].function = Some(FUNCTION_GROUP);
        logic[3].function = Some(FUNCTION_GROUP);
        logic[1].function = Some(FUNCTION_LAST);
        let recorded = [group("other", "x", 2, 1)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[]);
        assert_eq!(
            codes(&planned),
            vec![
                LogicPlacementIssueCode::UnterminatedGroup,
                LogicPlacementIssueCode::UnterminatedGroup,
            ]
        );
        assert_eq!(planned.issues[0].logic, vec![1]);
        assert_eq!(planned.issues[1].logic, vec![3, 4]);
    }

    #[test]
    fn a_terminated_outside_group_does_not_block() {
        let mut logic = blocked(4);
        logic[0].function = Some(FUNCTION_GROUP);
        logic[1].function = Some(FUNCTION_LAST);
        let planned = plan_logic_placement(&logic, &[], "f1", &[want("s0", 1)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("f1", "s0", 3, 1)]);
    }

    #[test]
    fn no_run_long_enough_is_no_free_logic() {
        let mut logic = blocked(5);
        logic[2].function = Some(FUNCTION_LAST); // outside single at 3 splits 2 + 2
        let planned = plan_logic_placement(&logic, &[], "f1", &[want("s0", 3)]);
        assert!(planned.placement.is_empty());
        assert_eq!(codes(&planned), vec![LogicPlacementIssueCode::NoFreeLogic]);
    }

    #[test]
    fn unread_logic_blocks_placement() {
        let mut logic = blocked(3);
        logic[1].function = None;
        let planned = plan_logic_placement(&logic, &[], "f1", &[want("s0", 1)]);
        assert!(planned.placement.is_empty());
        assert_eq!(codes(&planned), vec![LogicPlacementIssueCode::LogicUnread]);
        assert_eq!(planned.issues[0].logic, vec![2]);
    }

    #[test]
    fn a_group_grows_in_place_when_the_next_logic_is_free() {
        let mut logic = blocked(6);
        logic[1].function = Some(FUNCTION_LAST);
        let recorded = [group("f1", "s0", 2, 1)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("s0", 2)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("f1", "s0", 2, 2)]);
        assert!(planned.vacated.is_empty());
    }

    #[test]
    fn a_group_moves_to_the_lowest_free_run_when_the_next_logic_is_taken() {
        let mut logic = blocked(7);
        logic[0].function = Some(FUNCTION_LAST); // outside single at 1
        logic[1].function = Some(FUNCTION_LAST); // own single at 2
        logic[2].function = Some(FUNCTION_LAST); // outside single at 3 blocks growth
        let recorded = [group("f1", "s0", 2, 1), group("other", "x", 3, 1)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("s0", 2)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("other", "x", 3, 1), group("f1", "s0", 4, 2)]);
        assert_eq!(planned.vacated, vec![2]);
    }

    #[test]
    fn a_move_may_reuse_the_groups_own_old_slots() {
        let mut logic = blocked(5);
        logic[1].function = Some(FUNCTION_GROUP);
        logic[2].function = Some(FUNCTION_LAST);
        logic[3].function = Some(FUNCTION_LAST);
        let recorded = [group("f1", "s0", 2, 2), group("other", "x", 4, 1)];
        // Growing to 3 in place is blocked by 4; slots 1-3 are the lowest run (2-3 are its own).
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("s0", 3)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("f1", "s0", 1, 3), group("other", "x", 4, 1)]);
        assert!(planned.vacated.is_empty());
    }

    #[test]
    fn a_shrunk_group_keeps_its_first_logic_and_vacates_the_rest() {
        let mut logic = blocked(5);
        logic[1].function = Some(FUNCTION_GROUP);
        logic[2].function = Some(FUNCTION_LAST);
        let recorded = [group("f1", "s0", 2, 2)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("s0", 1)]);
        assert!(planned.issues.is_empty(), "{:?}", planned.issues);
        assert_eq!(planned.placement, vec![group("f1", "s0", 2, 1)]);
        assert_eq!(planned.vacated, vec![3]);
    }

    #[test]
    fn a_dropped_group_vacates_all_its_logic() {
        let mut logic = blocked(4);
        logic[0].function = Some(FUNCTION_GROUP);
        logic[1].function = Some(FUNCTION_LAST);
        let planned = plan_logic_placement(&logic, &[group("f1", "old", 1, 2)], "f1", &[]);
        assert_eq!(planned.vacated, vec![1, 2]);
    }

    #[test]
    fn a_resize_that_fits_nowhere_is_no_free_logic_naming_the_signal() {
        let mut logic = blocked(4);
        logic[0].function = Some(FUNCTION_LAST);
        logic[2].function = Some(FUNCTION_LAST);
        let recorded = [group("f1", "s0", 2, 1), group("other", "x", 3, 1)];
        let planned = plan_logic_placement(&logic, &recorded, "f1", &[want("s0", 3)]);
        assert!(planned.placement.is_empty());
        assert_eq!(codes(&planned), vec![LogicPlacementIssueCode::NoFreeLogic]);
        assert!(planned.issues[0].detail.contains("s0"));
        assert_eq!(planned.issues[0].signal_id.as_deref(), Some("s0"));
    }

    #[test]
    fn board_wide_issues_name_no_signal() {
        let mut logic = blocked(3);
        logic[1].function = None;
        let planned = plan_logic_placement(&logic, &[], "f1", &[want("s0", 1)]);
        assert_eq!(planned.issues[0].signal_id, None);
    }
}
