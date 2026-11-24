use bowties_core::layout::facilities::{ClaimedSlot, ClaimedSlotKind};
use bowties_core::signaling::compiler::*;
use bowties_core::signaling::{
    BlockSignalsRequest, SignalOutputOwner, SelectedBlock, SignalAppearanceSet, SignalDriver, SignalHardwareEvidence,
    SignalHardwareFacts, SignalHardwareHost, SignalOutput, SignalStyle,
};

const NODE: &str = "050101010700";
const OCC: [u8; 8] = [1, 1, 1, 1, 1, 1, 0, 1];
const CLR: [u8; 8] = [1, 1, 1, 1, 1, 1, 0, 2];

fn set_aspect(mast: u8, rule: u8) -> [u8; 8] {
    [2, 0, 0, 0, 0, 0, mast, rule]
}

fn line() -> BlockSignalsRequest {
    BlockSignalsRequest {
        facility_id: "abs:test".into(),
        facility_name: "Main".into(),
        blocks: vec![SelectedBlock {
            block_id: "block:a".into(),
            name: "Block A".into(),
            joined_to_previous: false,
            turnouts: vec![],
        }],
        ..Default::default()
    }
}

fn host() -> SignalHardwareHost {
    SignalHardwareHost {
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
    }
}

fn free_node() -> NodeSlotFacts {
    NodeSlotFacts {
        node_key: NODE.into(),
        masts: (1..=8)
            .map(|i| MastFact {
                index: i,
                in_use: false,
                claimed: false,
                set_aspect_events: vec![
                    Some(set_aspect(i as u8, 1)),
                    Some(set_aspect(i as u8, 2)),
                    Some(set_aspect(i as u8, 3)),
                ],
            })
            .collect(),
        logic: (1..=32).map(|i| LogicFact { index: i, in_use: false, claimed: false, actions: vec![] }).collect(),
        direct_lamps: (1..=16).collect(),
    }
}

fn signal_id(line: &BlockSignalsRequest) -> String {
    bowties_core::signaling::preview_block_signals(line.clone()).signals[0]
        .signal_id
        .clone()
}

fn input() -> CompileInput {
    let mut line = line();
    let id = signal_id(&line);
    line.signal_names.insert(id, "Main signal".into());
    CompileInput {
        wiring: vec![SignalWiring {
            signal_id: signal_id(&line),
            node_key: NODE.into(),
            red_output: 10,
            green_output: 9,
        }],
        line,
        hardware: SignalHardwareFacts { hosts: vec![host()] },
        nodes: vec![free_node()],
        occupancy: vec![BlockOccupancyEvents {
            block_id: "block:a".into(),
            occupied: Some(OCC),
            clear: Some(CLR),
        }],
        own_claims: vec![],
    }
}

fn int(v: i64) -> EditValue {
    EditValue::Int { value: v }
}

fn expected_edits(mast: u32, logic: u32, red: i64, green: i64) -> Vec<(String, EditValue)> {
    let m = format!("Rule to Aspect/Mast[{mast}]");
    let l = format!("Conditionals/Logic[{logic}]");
    let mut e: Vec<(String, EditValue)> = vec![
        (format!("{m}/Mast Description"), EditValue::String { value: "Main signal".into() }),
        (format!("{m}/Lamp Fade"), int(0)),
    ];
    // (rule name, track speed, lamps)
    let rules: [(i64, i64, Vec<i64>); 3] =
        [(0, 0, vec![red]), (21, 5, vec![red, green]), (29, 7, vec![green])];
    for (r, (name, speed, lamps)) in rules.iter().enumerate() {
        let r = r + 1;
        e.push((format!("{m}/Rule[{r}]/Rule Name"), int(*name)));
        e.push((format!("{m}/Rule[{r}]/Track Speed"), int(*speed)));
        for a in 1..=4usize {
            let lamp = lamps.get(a - 1).copied().unwrap_or(0);
            e.push((format!("{m}/Rule[{r}]/Appearance[{a}]/Lamp Selection"), int(lamp)));
            e.push((
                format!("{m}/Rule[{r}]/Appearance[{a}]/Lamp Phase (A-B) - Flash Rate"),
                int(0),
            ));
        }
        e.push((format!("{m}/Rule[{r}]/Appearance Effects"), int(0)));
        e.push((format!("{m}/Rule[{r}]/Effects Lamp"), int(0)));
    }
    e.push((format!("{m}/Mast Processing"), int(1)));
    e.extend([
        (format!("{l}/Description"), EditValue::String { value: "Main signal occupancy".into() }),
        (format!("{l}/Variable #1/Trigger"), int(1)),
        (format!("{l}/Variable #1/Source"), int(0)),
        (format!("{l}/Variable #1/set true"), EditValue::EventId { bytes: OCC }),
        (format!("{l}/Variable #1/set false"), EditValue::EventId { bytes: CLR }),
        (format!("{l}/Logic Operation"), int(7)),
        (format!("{l}/Variable #2/Trigger"), int(2)),
        (format!("{l}/Action#1/when true"), int(0)),
        (format!("{l}/Action#1/when false"), int(0)),
        (format!("{l}/Time Delay before action/Delay Time (1-60000)"), int(0)),
        (format!("{l}/Time Delay before action/Units"), int(0)),
        (format!("{l}/Action#2[1]/Condition"), int(3)),
        (
            format!("{l}/Action#2[1]/Action Event"),
            EditValue::EventId { bytes: set_aspect(mast as u8, 1) },
        ),
        (format!("{l}/Action#2[2]/Condition"), int(4)),
        (
            format!("{l}/Action#2[2]/Action Event"),
            EditValue::EventId { bytes: set_aspect(mast as u8, 3) },
        ),
        (format!("{l}/Action#2[3]/Condition"), int(0)),
        (format!("{l}/Action#2[4]/Condition"), int(0)),
        (format!("{l}/Function"), int(2)),
    ]);
    e
}

fn pairs(c: &CompiledBlockSignals) -> Vec<(String, EditValue)> {
    c.edits.iter().map(|e| (e.path.clone(), e.value.clone())).collect()
}

#[test]
fn terminal_recipe_uses_lowest_free_slots_and_is_deterministic() {
    let free = compile_block_signals(&input());
    assert_eq!(free.issues, vec![]);
    assert_eq!(pairs(&free), expected_edits(1, 1, 10, 9));

    // Mast 1 and Logic 1 are taken, so the lowest free slots are 2 and 2.
    let mut taken = input();
    taken.nodes[0].masts[0].in_use = true;
    taken.nodes[0].logic[0].in_use = true;
    let first = compile_block_signals(&taken);
    assert_eq!(first.issues, vec![]);
    assert_eq!(pairs(&first), expected_edits(2, 2, 10, 9));
    assert!(first.edits.iter().all(|e| e.node_key == NODE));
    assert_eq!(
        first.claimed_slots,
        vec![
            ClaimedSlot { node_key: NODE.into(), kind: ClaimedSlotKind::Mast, index: 2 },
            ClaimedSlot { node_key: NODE.into(), kind: ClaimedSlotKind::Logic, index: 2 },
        ]
    );
    assert_eq!(first, compile_block_signals(&taken));
}

fn codes(c: &CompiledBlockSignals) -> Vec<CompileIssueCode> {
    c.issues.iter().map(|i| i.code).collect()
}

#[test]
fn blocking_issues_are_coded_scoped_and_suppress_all_edits() {
    use bowties_core::signaling::{ApproachLighting, Movements};
    use CompileIssueCode::*;

    let sid = signal_id(&line());
    let mut cases: Vec<(&str, CompileInput, CompileIssueCode, bool)> = Vec::new();

    let mut c = input();
    c.hardware.hosts[0].outputs[9].owner = Some(SignalOutputOwner::DirectLamp(3));
    cases.push(("output owned", c, OutputInUse, true));

    let mut c = input();
    c.wiring[0].green_output = c.wiring[0].red_output;
    cases.push(("duplicate pin", c, OutputDuplicate, true));

    let mut c = input();
    for o in c.hardware.hosts[0].outputs.iter_mut().skip(1) {
        o.owner = Some(SignalOutputOwner::Mast(4));
    }
    cases.push(("capacity", c, OutputCapacity, true));

    let mut c = input();
    c.nodes[0].masts.iter_mut().for_each(|m| m.in_use = true);
    cases.push(("no free mast", c, NoFreeMast, true));

    let mut c = input();
    c.nodes[0].logic.iter_mut().for_each(|l| l.in_use = true);
    cases.push(("no free logic", c, NoFreeLogic, true));

    let mut c = input();
    c.occupancy[0].clear = None;
    cases.push(("no clear event", c, OccupancyEventsMissing, false));

    let mut c = input();
    c.nodes[0].masts[0].set_aspect_events[2] = None;
    cases.push(("no aspect event", c, AspectEventsMissing, true));

    // No slot is free.
    let mut c = input();
    c.nodes[0].masts.iter_mut().for_each(|m| m.in_use = true);
    cases.push(("every mast in use", c, NoFreeMast, true));

    let mut c = input();
    c.nodes[0].logic.iter_mut().for_each(|l| l.in_use = true);
    cases.push(("every logic in use", c, NoFreeLogic, true));

    let mut c = input();
    c.line.settings.aspects = 2;
    cases.push(("two aspects", c, UnsupportedAspects, false));

    let mut c = input();
    c.line.settings.aspects = 4;
    cases.push(("four aspects", c, UnsupportedAspects, false));

    let mut c = input();
    c.line.settings.approach_lighting = ApproachLighting::WhenApproached;
    cases.push(("approach lighting", c, UnsupportedApproachLighting, false));

    let mut c = input();
    c.line.movements = Movements::BothDirections;
    cases.push(("both directions", c, UnsupportedMovements, false));

    for (name, case, expected, signal_scoped) in cases {
        let result = compile_block_signals(&case);
        assert!(codes(&result).contains(&expected), "{name}: {:?}", result.issues);
        assert!(result.edits.is_empty(), "{name}: edits must be empty");
        assert!(result.claimed_slots.is_empty(), "{name}: claims must be empty");
        let issue = result.issues.iter().find(|i| i.code == expected).unwrap();
        if signal_scoped {
            assert_eq!(issue.signal_id.as_deref(), Some(sid.as_str()), "{name}");
            assert_eq!(issue.node_key.as_deref(), Some(NODE), "{name}");
        }
    }
}

fn claim(kind: ClaimedSlotKind, index: u32) -> ClaimedSlot {
    ClaimedSlot { node_key: NODE.into(), kind, index }
}

/// A saved facility's own configuration: Mast 2 and Logic 1 are active and
/// Mast 2 drives pins 9 and 10.
fn recompile_input() -> CompileInput {
    let mut c = input();
    c.own_claims = vec![claim(ClaimedSlotKind::Mast, 2), claim(ClaimedSlotKind::Logic, 1)];
    c.nodes[0].masts[1].in_use = true;
    c.nodes[0].logic[0].in_use = true;
    for o in c.hardware.hosts[0].outputs.iter_mut().filter(|o| o.ordinal == 9 || o.ordinal == 10) {
        o.owner = Some(SignalOutputOwner::Mast(2));
    }
    c
}

#[test]
fn recompile_reuses_own_claimed_slots_and_own_pins() {
    let result = compile_block_signals(&recompile_input());
    assert_eq!(result.issues, vec![]);
    assert_eq!(pairs(&result), expected_edits(2, 1, 10, 9));
    assert_eq!(
        result.claimed_slots,
        vec![claim(ClaimedSlotKind::Mast, 2), claim(ClaimedSlotKind::Logic, 1)]
    );
}

#[test]
fn recompile_still_conflicts_with_pins_of_another_mast() {
    let mut c = recompile_input();
    c.hardware.hosts[0].outputs[9].owner = Some(SignalOutputOwner::Mast(3));
    let result = compile_block_signals(&c);
    assert!(codes(&result).contains(&CompileIssueCode::OutputInUse), "{:?}", result.issues);
    assert!(result.edits.is_empty());
}

#[test]
fn recompile_never_repicks_when_a_claimed_slot_is_unavailable() {
    use CompileIssueCode::ClaimedSlotUnavailable;

    let mut other_node = recompile_input();
    other_node.own_claims[0].node_key = "050101010999".into();

    let mut missing_mast = recompile_input();
    missing_mast.nodes[0].masts.retain(|m| m.index != 2);

    let mut missing_logic = recompile_input();
    missing_logic.nodes[0].logic.retain(|l| l.index != 1);

    for (name, case) in [("other node", other_node), ("missing mast", missing_mast), ("missing logic", missing_logic)] {
        let result = compile_block_signals(&case);
        assert!(codes(&result).contains(&ClaimedSlotUnavailable), "{name}: {:?}", result.issues);
        assert!(result.edits.is_empty(), "{name}");
        assert!(result.claimed_slots.is_empty(), "{name}");
    }
}

fn release_claims() -> Vec<ClaimedSlot> {
    vec![claim(ClaimedSlotKind::Mast, 2), claim(ClaimedSlotKind::Logic, 1)]
}

fn release_targets() -> Vec<ReleaseTarget> {
    release_claims().iter().map(ReleaseTarget::from).collect()
}

fn release_edit(path: &str, value: EditValue) -> NodeConfigEdit {
    NodeConfigEdit { node_key: NODE.into(), path: path.into(), value }
}

#[test]
fn release_resets_every_rule_of_a_mast_every_event_of_a_logic_and_the_lamp_row() {
    let mut targets = release_targets();
    targets.push(ReleaseTarget { node_key: NODE.into(), slot: ReleaseSlot::DirectLamp(5) });
    let result = release_slots(&targets, &[free_node()]);
    assert_eq!(result.issues, vec![]);
    assert!(result.claimed_slots.is_empty());
    let int0 = || EditValue::Int { value: 0 };
    let empty = || EditValue::String { value: String::new() };
    let mut expected = vec![
        release_edit("Rule to Aspect/Mast[2]/Mast Processing", int0()),
        release_edit("Rule to Aspect/Mast[2]/Mast Description", empty()),
    ];
    // free_node's masts carry 3 set-aspect events; the reset covers that many rules.
    for rule in 1..=3 {
        for appearance in 1..=4 {
            expected.push(release_edit(
                &format!("Rule to Aspect/Mast[2]/Rule[{rule}]/Appearance[{appearance}]/Lamp Selection"),
                int0(),
            ));
        }
        expected.push(release_edit(&format!("Rule to Aspect/Mast[2]/Rule[{rule}]/Effects Lamp"), int0()));
    }
    expected.push(release_edit("Conditionals/Logic[1]/Function", int0()));
    expected.push(release_edit("Conditionals/Logic[1]/Description", empty()));
    for leaf in [
        "Variable #1/set true",
        "Variable #1/set false",
        "Variable #2/set true",
        "Variable #2/set false",
        "Action#2[1]/Action Event",
        "Action#2[2]/Action Event",
        "Action#2[3]/Action Event",
        "Action#2[4]/Action Event",
    ] {
        expected.push(release_edit(&format!("Conditionals/Logic[1]/{leaf}"), EditValue::FreshEventId));
    }
    expected.push(release_edit("Direct Lamp Control/Lamp[5]/Lamp Selection", int0()));
    expected.push(release_edit("Direct Lamp Control/Lamp[5]/Lamp Description", empty()));
    expected.push(release_edit("Direct Lamp Control/Lamp[5]/Lamp On", EditValue::FreshEventId));
    expected.push(release_edit("Direct Lamp Control/Lamp[5]/Lamp Off", EditValue::FreshEventId));
    assert_eq!(result.edits, expected);
    for edit in &result.edits {
        let inside = targets.iter().any(|t| {
            let prefix = match &t.slot {
                ReleaseSlot::Mast(i) => format!("Rule to Aspect/Mast[{i}]/"),
                ReleaseSlot::Logic(i) => format!("Conditionals/Logic[{i}]/"),
                ReleaseSlot::DirectLamp(i) => format!("Direct Lamp Control/Lamp[{i}]/"),
                ReleaseSlot::LogicActions { index, .. } => format!("Conditionals/Logic[{index}]/"),
            };
            t.node_key == edit.node_key && edit.path.starts_with(&prefix)
        });
        assert!(inside, "edit outside the released slots: {edit:?}");
    }
}

#[test]
fn release_with_unavailable_claims_is_blocked_with_no_edits() {
    let mut missing_index = free_node();
    missing_index.masts.retain(|m| m.index != 2);
    let no_facts = NodeSlotFacts { node_key: "050101010999".into(), ..free_node() };
    for (name, nodes) in [("missing index", vec![missing_index]), ("node without facts", vec![no_facts])] {
        let result = release_slots(&release_targets(), &nodes);
        assert!(codes(&result).contains(&CompileIssueCode::ClaimedSlotUnavailable), "{name}: {:?}", result.issues);
        assert!(result.edits.is_empty(), "{name}");
        assert!(result.claimed_slots.is_empty(), "{name}");
    }
}

#[test]
fn release_freshens_at_least_the_event_leaves_the_recipe_wires_into_the_logic() {
    let recipe = compile_block_signals(&recompile_input());
    assert_eq!(recipe.issues, vec![]);
    let recipe_event_paths: std::collections::BTreeSet<_> = recipe
        .edits
        .iter()
        .filter(|e| e.path.starts_with("Conditionals/") && matches!(e.value, EditValue::EventId { .. }))
        .map(|e| e.path.clone())
        .collect();
    let fresh: std::collections::BTreeSet<_> = release_slots(&release_targets(), &[free_node()])
        .edits
        .iter()
        .filter(|e| e.value == EditValue::FreshEventId)
        .map(|e| e.path.clone())
        .collect();
    assert!(!fresh.is_empty());
    assert!(fresh.is_superset(&recipe_event_paths), "release must cover every event the recipe wrote");
}

#[test]
fn no_free_slot_issues_say_how_many_slots_are_configured_outside_bowties() {
    let mut c = input();
    for (n, mast) in c.nodes[0].masts.iter_mut().enumerate() {
        mast.in_use = true;
        mast.claimed = n >= 3; // Masts 1-3 are configured outside Bowties.
    }
    for (n, logic) in c.nodes[0].logic.iter_mut().enumerate() {
        logic.in_use = true;
        logic.claimed = n >= 1; // Only Logic 1 is configured outside Bowties.
    }
    let result = compile_block_signals(&c);
    let detail = |code| result.issues.iter().find(|i| i.code == code).map(|i| i.detail.clone());
    assert_eq!(
        detail(CompileIssueCode::NoFreeMast).as_deref(),
        Some("The board has no free mast; 3 are configured outside Bowties.")
    );
    assert_eq!(
        detail(CompileIssueCode::NoFreeLogic).as_deref(),
        Some("The board has no free Logic conditional; 1 is configured outside Bowties.")
    );
}

fn logic_sending(node: &mut NodeSlotFacts, index: u32, actions: &[(i64, [u8; 8])]) {
    let logic = &mut node.logic[index as usize - 1];
    logic.in_use = true;
    logic.actions = (0..4)
        .map(|n| match actions.get(n) {
            Some((condition, event)) => LogicAction { condition: Some(*condition), event: Some(*event) },
            None => LogicAction { condition: Some(0), event: None },
        })
        .collect();
}

#[test]
fn untracked_mast_release_matches_same_node_logic_by_aspect_event_and_keeps_logic_with_other_actions() {
    let other: [u8; 8] = [9, 9, 9, 9, 9, 9, 9, 9];
    let mut node = free_node();
    // Logic 1: only sets the mast's aspects -> matched, fully released.
    logic_sending(&mut node, 1, &[(3, set_aspect(2, 1)), (4, set_aspect(2, 3))]);
    // Logic 2: sets one aspect and also sends another event -> matched and kept.
    logic_sending(&mut node, 2, &[(3, set_aspect(2, 2)), (1, other)]);
    // Logic 3: matching event but Function is 0 (not in use) -> not matched.
    logic_sending(&mut node, 3, &[(3, set_aspect(2, 1))]);
    node.logic[2].in_use = false;
    // Logic 4: sends events for a different mast -> not matched.
    logic_sending(&mut node, 4, &[(3, set_aspect(5, 1))]);
    // Logic 5: matching event but claimed by a facility -> not matched.
    logic_sending(&mut node, 5, &[(3, set_aspect(2, 1))]);
    node.logic[4].claimed = true;
    // Logic 6: a matching action whose condition is 0 still counts as a match; a
    // different event under condition 0 sends nothing, so the Logic is not kept.
    logic_sending(&mut node, 6, &[(0, set_aspect(2, 3)), (0, other)]);

    let owners = vec![
        (NODE.to_string(), SignalOutputOwner::Mast(2)),
        (NODE.to_string(), SignalOutputOwner::DirectLamp(7)),
    ];
    let result = untracked_release_targets(&owners, &[node.clone()]);

    assert_eq!(
        result.owners,
        vec![
            UntrackedOwnerRelease {
                node_key: NODE.into(),
                owner: SignalOutputOwner::Mast(2),
                matched_logic: vec![1, 2, 6],
                kept_logic: vec![2],
            },
            UntrackedOwnerRelease {
                node_key: NODE.into(),
                owner: SignalOutputOwner::DirectLamp(7),
                matched_logic: vec![],
                kept_logic: vec![],
            },
        ]
    );
    let target = |slot| ReleaseTarget { node_key: NODE.into(), slot };
    assert_eq!(
        result.targets,
        vec![
            target(ReleaseSlot::Mast(2)),
            target(ReleaseSlot::Logic(1)),
            target(ReleaseSlot::LogicActions { index: 2, actions: vec![1] }),
            target(ReleaseSlot::Logic(6)),
            target(ReleaseSlot::DirectLamp(7)),
        ]
    );

    // The kept Logic gets fresh IDs on only its mast-matching action; edits stay inside the targets.
    let released = release_slots(&result.targets, &[node]);
    assert_eq!(released.issues, vec![]);
    let logic2: Vec<_> = released.edits.iter().filter(|e| e.path.starts_with("Conditionals/Logic[2]/")).collect();
    assert_eq!(
        logic2,
        vec![&release_edit("Conditionals/Logic[2]/Action#2[1]/Action Event", EditValue::FreshEventId)]
    );
    assert!(released.edits.iter().any(|e| e.path == "Conditionals/Logic[1]/Function"));
    assert!(released.edits.iter().all(|e| {
        ["Rule to Aspect/Mast[2]/", "Conditionals/Logic[1]/", "Conditionals/Logic[2]/Action#2[1]/",
         "Conditionals/Logic[6]/", "Direct Lamp Control/Lamp[7]/"]
            .iter()
            .any(|prefix| e.path.starts_with(prefix))
    }));
}

#[test]
fn untracked_release_targets_never_name_a_slot_or_leaf_twice() {
    let mut node = free_node();
    // Mast 3 also answers to Mast 2's Stop-rule event and to one extra event.
    node.masts[2].set_aspect_events[0] = Some(set_aspect(2, 1));
    node.masts[2].set_aspect_events[1] = Some(set_aspect(2, 2));
    node.masts[2].set_aspect_events[2] = Some([9; 8]);
    // Logic 1 matches both masts and sends nothing else for either -> released once.
    logic_sending(&mut node, 1, &[(3, set_aspect(2, 1))]);
    // Logic 2 is kept for Mast 2 (it also sends [9; 8]) but is all Mast 3's -> full release wins.
    logic_sending(&mut node, 2, &[(3, set_aspect(2, 2)), (1, [9; 8])]);
    // Logic 3 is kept for both masts, each matching a different action -> actions are unioned.
    logic_sending(&mut node, 3, &[(3, set_aspect(2, 3)), (3, [9; 8]), (1, [8; 8])]);

    let owners = vec![
        (NODE.to_string(), SignalOutputOwner::Mast(2)),
        (NODE.to_string(), SignalOutputOwner::Mast(3)),
    ];
    let result = untracked_release_targets(&owners, &[node.clone()]);
    let target = |slot| ReleaseTarget { node_key: NODE.into(), slot };
    let logic_targets: Vec<_> =
        result.targets.iter().filter(|t| !matches!(t.slot, ReleaseSlot::Mast(_))).cloned().collect();
    assert_eq!(
        logic_targets,
        vec![
            target(ReleaseSlot::Logic(1)),
            target(ReleaseSlot::Logic(2)),
            target(ReleaseSlot::LogicActions { index: 3, actions: vec![1, 2] }),
        ]
    );

    let released = release_slots(&result.targets, &[node]);
    assert_eq!(released.issues, vec![]);
    let mut leaves: Vec<_> = released.edits.iter().map(|e| (e.node_key.clone(), e.path.clone())).collect();
    let total = leaves.len();
    leaves.sort();
    leaves.dedup();
    assert_eq!(leaves.len(), total, "no two release edits target the same leaf");
}
