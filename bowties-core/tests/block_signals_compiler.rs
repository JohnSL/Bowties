use bowties_core::layout::facilities::{
    ClaimedSlot, ClaimedSlotKind, NodeLogicPlacement, PlacedLogicGroup, SignalResources,
};
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
                link_address: None,
            })
            .collect(),
        logic: (1..=32)
            .map(|i| LogicFact { index: i, in_use: false, function: Some(0), claimed: false, actions: vec![], sources: vec![] })
            .collect(),
        direct_lamps: (1..=16).collect(),
        track_circuits: vec![],
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
        saved_resources: vec![],
        logic_placement: vec![],
    }
}

/// A Logic configured with Function Last (Single).
fn use_logic(l: &mut LogicFact) {
    l.in_use = true;
    l.function = Some(2);
}

fn int(v: i64) -> EditValue {
    EditValue::Int { value: v }
}

fn expected_edits(mast: u32, logic: u32, red: i64, green: i64) -> Vec<(String, EditValue)> {
    expected_edits_named("Main signal", mast, logic, red, green)
}

fn expected_edits_named(name: &str, mast: u32, logic: u32, red: i64, green: i64) -> Vec<(String, EditValue)> {
    let m = format!("Rule to Aspect/Mast[{mast}]");
    let l = format!("Conditionals/Logic[{logic}]");
    let mut e: Vec<(String, EditValue)> = vec![
        (format!("{m}/Mast Description"), EditValue::String { value: name.into() }),
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
        (format!("{l}/Description"), EditValue::String { value: format!("{name} occupancy") }),
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
    use_logic(&mut taken.nodes[0].logic[0]);
    let first = compile_block_signals(&taken);
    assert_eq!(first.issues, vec![]);
    assert_eq!(pairs(&first), expected_edits(2, 2, 10, 9));
    assert!(first.edits.iter().all(|e| e.node_key == NODE));
    assert_eq!(first.resources, vec![resources_for(2)]);
    assert_eq!(first.placement, vec![placement_of(&[2])]);
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
    c.nodes[0].logic.iter_mut().for_each(use_logic);
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
    c.nodes[0].logic.iter_mut().for_each(use_logic);
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
        assert!(result.resources.is_empty() && result.placement.is_empty(), "{name}: records must be empty");
        let issue = result.issues.iter().find(|i| i.code == expected).unwrap();
        if signal_scoped {
            assert_eq!(issue.signal_id.as_deref(), Some(sid.as_str()), "{name}");
            assert_eq!(issue.node_key.as_deref(), Some(NODE), "{name}");
        }
    }
}

fn own_resources(mast: u32) -> SignalResources {
    SignalResources { node_key: NODE.into(), mast, track_circuits: vec![] }
}

fn resources_for(mast: u32) -> PlannedSignalResources {
    PlannedSignalResources { signal_id: signal_id(&line()), resources: own_resources(mast) }
}

/// The board's placement holding this facility's one signal group per `first`.
fn placement_of(firsts: &[u32]) -> NodeLogicPlacement {
    NodeLogicPlacement {
        node_key: NODE.into(),
        groups: firsts
            .iter()
            .map(|first| PlacedLogicGroup {
                facility_id: "abs:test".into(),
                signal_id: signal_id(&line()),
                first: *first,
                length: 1,
            })
            .collect(),
    }
}

fn claim(kind: ClaimedSlotKind, index: u32) -> ClaimedSlot {
    ClaimedSlot { node_key: NODE.into(), kind, index }
}

/// A saved facility's own configuration: Mast 2 and Logic 1 are active and
/// Mast 2 drives pins 9 and 10.
fn recompile_input() -> CompileInput {
    let mut c = input();
    c.saved_resources = vec![PlannedSignalResources { signal_id: signal_id(&c.line), resources: own_resources(2) }];
    c.logic_placement = vec![placement_of(&[1])];
    c.nodes[0].masts[1].in_use = true;
    use_logic(&mut c.nodes[0].logic[0]);
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
    assert_eq!(result.resources, vec![resources_for(2)]);
    assert_eq!(result.placement, vec![placement_of(&[1])]);
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
    other_node.saved_resources[0].resources.node_key = "050101010999".into();

    let mut missing_mast = recompile_input();
    missing_mast.nodes[0].masts.retain(|m| m.index != 2);

    let mut missing_logic = recompile_input();
    missing_logic.nodes[0].logic.retain(|l| l.index != 1);

    for (name, case) in [("other node", other_node), ("missing mast", missing_mast), ("missing logic", missing_logic)] {
        let result = compile_block_signals(&case);
        assert!(codes(&result).contains(&ClaimedSlotUnavailable), "{name}: {:?}", result.issues);
        assert!(result.edits.is_empty(), "{name}");
        assert!(result.resources.is_empty() && result.placement.is_empty(), "{name}");
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
                ReleaseSlot::TrackCircuit(i) => format!("Track Circuits/Circuit[{i}]/"),
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
        use_logic(logic);
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

#[test]
fn a_one_block_compile_beside_an_outside_group_yields_the_pinned_edits_resources_and_placement() {
    // Logic 1 is a terminated group configured outside Bowties; Logic 2+ are free.
    let mut c = input();
    use_logic(&mut c.nodes[0].logic[0]);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(pairs(&result), expected_edits(1, 2, 10, 9), "same edits as the flat-claims compiler");
    assert_eq!(result.resources, vec![resources_for(1)]);
    assert_eq!(result.placement, vec![placement_of(&[2])]);
}

#[test]
fn recompiling_with_recorded_placement_keeps_the_same_logic_even_when_a_lower_slot_is_free() {
    let mut c = recompile_input();
    // The recorded group sits at Logic 3; Logic 1 is free (Blocked) and lower.
    c.nodes[0].logic[0] = LogicFact { index: 1, in_use: false, function: Some(0), claimed: false, actions: vec![], sources: vec![] };
    use_logic(&mut c.nodes[0].logic[2]);
    c.logic_placement = vec![placement_of(&[3])];
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(pairs(&result), expected_edits(2, 3, 10, 9));
    assert_eq!(result.placement, vec![placement_of(&[3])]);
}

#[test]
fn outside_logic_with_an_undefined_function_blocks_the_board_and_names_the_logic() {
    let mut c = input();
    c.nodes[0].logic[1].in_use = true;
    c.nodes[0].logic[1].function = Some(3);
    let result = compile_block_signals(&c);
    let issue = result
        .issues
        .iter()
        .find(|i| i.code == CompileIssueCode::LogicUndefinedFunction)
        .unwrap_or_else(|| panic!("{:?}", result.issues));
    assert!(issue.detail.contains("Logic 2"), "{}", issue.detail);
    assert_eq!(issue.signal_id.as_deref(), Some(signal_id(&line()).as_str()));
    assert!(result.edits.is_empty() && result.resources.is_empty() && result.placement.is_empty());
}

const LINK_B: [u8; 8] = [3, 3, 3, 3, 3, 3, 0, 2];
const OCC_B: [u8; 8] = [1, 1, 1, 1, 1, 1, 0, 3];
const CLR_B: [u8; 8] = [1, 1, 1, 1, 1, 1, 0, 4];

fn two_block_line() -> BlockSignalsRequest {
    let mut line = line();
    line.blocks.push(SelectedBlock {
        block_id: "block:b".into(),
        name: "Block B".into(),
        joined_to_previous: false,
        turnouts: vec![],
    });
    line
}

fn two_block_ids(line: &BlockSignalsRequest) -> (String, String) {
    let signals = bowties_core::signaling::preview_block_signals(line.clone()).signals;
    (signals[0].signal_id.clone(), signals[1].signal_id.clone())
}

/// A two-block Line on one free board: the first signal has the second after it.
fn two_block_input() -> CompileInput {
    let mut line = two_block_line();
    let (first, second) = two_block_ids(&line);
    line.signal_names.insert(first.clone(), "First signal".into());
    line.signal_names.insert(second.clone(), "Second signal".into());
    let mut node = free_node();
    node.masts[1].link_address = Some(LINK_B);
    node.track_circuits = (1..=8)
        .map(|i| TrackCircuitFact { index: i, in_use: false, claimed: false, link_address: None })
        .collect();
    CompileInput {
        wiring: vec![
            SignalWiring { signal_id: first, node_key: NODE.into(), red_output: 10, green_output: 9 },
            SignalWiring { signal_id: second, node_key: NODE.into(), red_output: 12, green_output: 11 },
        ],
        line,
        hardware: SignalHardwareFacts { hosts: vec![host()] },
        nodes: vec![node],
        occupancy: vec![
            BlockOccupancyEvents { block_id: "block:a".into(), occupied: Some(OCC), clear: Some(CLR) },
            BlockOccupancyEvents { block_id: "block:b".into(), occupied: Some(OCC_B), clear: Some(CLR_B) },
        ],
        saved_resources: vec![],
        logic_placement: vec![],
    }
}

fn mast_part(edits: Vec<(String, EditValue)>) -> Vec<(String, EditValue)> {
    edits.into_iter().take_while(|(p, _)| p.starts_with("Rule to Aspect")).collect()
}

/// The internal signal's Track Circuit and its two Logic rungs (rung 2 first).
fn internal_edits(name: &str, next_name: &str, mast: u32, first_logic: u32, circuit: u32) -> Vec<(String, EditValue)> {
    let c = format!("Track Circuits/Circuit[{circuit}]");
    let (l1, l2) = (
        format!("Conditionals/Logic[{first_logic}]"),
        format!("Conditionals/Logic[{}]", first_logic + 1),
    );
    let ev = |rule: u8| EditValue::EventId { bytes: set_aspect(mast as u8, rule) };
    let s = |v: String| EditValue::String { value: v };
    vec![
        (format!("{c}/Remote Mast Description"), s(next_name.into())),
        (format!("{c}/Link Address"), EditValue::EventId { bytes: LINK_B }),
        (format!("{l2}/Description"), s(format!("{name} next signal"))),
        (format!("{l2}/Variable #1/Trigger"), int(0)),
        (format!("{l2}/Variable #1/Source"), int(circuit as i64)),
        (format!("{l2}/Variable #1/Track Speed"), int(0)),
        (format!("{l2}/Logic Operation"), int(7)),
        (format!("{l2}/Variable #2/Trigger"), int(2)),
        (format!("{l2}/Action#1/when true"), int(0)),
        (format!("{l2}/Action#1/when false"), int(0)),
        (format!("{l2}/Time Delay before action/Delay Time (1-60000)"), int(0)),
        (format!("{l2}/Time Delay before action/Units"), int(0)),
        (format!("{l2}/Action#2[1]/Condition"), int(3)),
        (format!("{l2}/Action#2[1]/Action Event"), ev(2)),
        (format!("{l2}/Action#2[2]/Condition"), int(4)),
        (format!("{l2}/Action#2[2]/Action Event"), ev(3)),
        (format!("{l2}/Action#2[3]/Condition"), int(0)),
        (format!("{l2}/Action#2[4]/Condition"), int(0)),
        (format!("{l2}/Function"), int(2)),
        (format!("{l1}/Description"), s(format!("{name} occupancy"))),
        (format!("{l1}/Variable #1/Trigger"), int(1)),
        (format!("{l1}/Variable #1/Source"), int(0)),
        (format!("{l1}/Variable #1/set true"), EditValue::EventId { bytes: OCC }),
        (format!("{l1}/Variable #1/set false"), EditValue::EventId { bytes: CLR }),
        (format!("{l1}/Logic Operation"), int(7)),
        (format!("{l1}/Variable #2/Trigger"), int(2)),
        (format!("{l1}/Action#1/when true"), int(0)),
        (format!("{l1}/Action#1/when false"), int(4)),
        (format!("{l1}/Time Delay before action/Delay Time (1-60000)"), int(0)),
        (format!("{l1}/Time Delay before action/Units"), int(0)),
        (format!("{l1}/Action#2[1]/Condition"), int(3)),
        (format!("{l1}/Action#2[1]/Action Event"), ev(1)),
        (format!("{l1}/Action#2[2]/Condition"), int(0)),
        (format!("{l1}/Action#2[3]/Condition"), int(0)),
        (format!("{l1}/Action#2[4]/Condition"), int(0)),
        (format!("{l1}/Function"), int(1)),
    ]
}

fn held(signal_id: &str, mast: u32, circuits: Vec<u32>) -> PlannedSignalResources {
    PlannedSignalResources {
        signal_id: signal_id.into(),
        resources: SignalResources { node_key: NODE.into(), mast, track_circuits: circuits },
    }
}

fn group(signal_id: &str, first: u32, length: u32) -> PlacedLogicGroup {
    PlacedLogicGroup { facility_id: "abs:test".into(), signal_id: signal_id.into(), first, length }
}

/// The last signal's terminal recipe on its own mast and Logic, with its block's events.
fn last_edits(mast: u32, logic: u32) -> Vec<(String, EditValue)> {
    let mut last = expected_edits_named("Second signal", mast, logic, 12, 11);
    for (path, value) in last.iter_mut() {
        if path.ends_with("Variable #1/set true") {
            *value = EditValue::EventId { bytes: OCC_B };
        } else if path.ends_with("Variable #1/set false") {
            *value = EditValue::EventId { bytes: CLR_B };
        }
    }
    last
}

#[test]
fn two_blocks_give_the_first_signal_a_two_rung_group_and_a_track_circuit() {
    let input = two_block_input();
    let (first, second) = two_block_ids(&input.line);
    let result = compile_block_signals(&input);
    assert_eq!(result.issues, vec![]);

    let mut expected = mast_part(expected_edits_named("First signal", 1, 1, 10, 9));
    expected.extend(internal_edits("First signal", "Second signal", 1, 1, 1));
    expected.extend(last_edits(2, 3));
    assert_eq!(pairs(&result), expected);

    assert_eq!(result.resources, vec![held(&first, 1, vec![1]), held(&second, 2, vec![])]);
    assert_eq!(
        result.placement,
        vec![NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&first, 1, 2), group(&second, 3, 1)] }]
    );
}

fn scoped<'a>(c: &'a CompiledBlockSignals, code: CompileIssueCode) -> &'a CompileIssue {
    c.issues.iter().find(|i| i.code == code).unwrap_or_else(|| panic!("{code:?} in {:?}", c.issues))
}

fn assert_blocked(c: &CompiledBlockSignals) {
    assert!(c.edits.is_empty() && c.resources.is_empty() && c.placement.is_empty());
}

/// Mark Mast 1 and pins 9 and 10 as the saved first signal's own.
fn own_first_mast(c: &mut CompileInput, mast: u32) {
    c.nodes[0].masts[mast as usize - 1].in_use = true;
    c.nodes[0].masts[mast as usize - 1].claimed = true;
    for o in c.hardware.hosts[0].outputs.iter_mut().filter(|o| o.ordinal == 9 || o.ordinal == 10) {
        o.owner = Some(SignalOutputOwner::Mast(mast));
    }
}

const NODE_B: &str = "050101010799";

/// The two-block Line with its second signal wired on a second free board whose
/// first mast carries `LINK_B` as its Link Address.
fn two_board_input() -> CompileInput {
    let mut c = two_block_input();
    c.wiring[1].node_key = NODE_B.into();
    let mut board_b = free_node();
    board_b.node_key = NODE_B.into();
    board_b.masts[0].link_address = Some(LINK_B);
    c.nodes.push(board_b);
    let mut host_b = host();
    host_b.node_key = NODE_B.into();
    c.hardware.hosts.push(host_b);
    c
}

#[test]
fn signals_on_two_boards_each_get_their_own_mast_logic_and_the_link_address_comes_from_the_next_board() {
    let c = two_board_input();
    let (first, second) = two_block_ids(&c.line);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);

    let mut expected = mast_part(expected_edits_named("First signal", 1, 1, 10, 9));
    expected.extend(internal_edits("First signal", "Second signal", 1, 1, 1));
    expected.extend(last_edits(1, 1));
    assert_eq!(pairs(&result), expected);
    let first_edit_count = expected.len() - last_edits(1, 1).len();
    assert!(result.edits[..first_edit_count].iter().all(|e| e.node_key == NODE));
    assert!(result.edits[first_edit_count..].iter().all(|e| e.node_key == NODE_B));

    let held_on = |signal: &str, node: &str, mast: u32, circuits: Vec<u32>| PlannedSignalResources {
        signal_id: signal.into(),
        resources: SignalResources { node_key: node.into(), mast, track_circuits: circuits },
    };
    assert_eq!(result.resources, vec![held_on(&first, NODE, 1, vec![1]), held_on(&second, NODE_B, 1, vec![])]);
    assert_eq!(
        result.placement,
        vec![
            NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&first, 1, 2)] },
            NodeLogicPlacement { node_key: NODE_B.into(), groups: vec![group(&second, 1, 1)] },
        ]
    );
}

#[test]
fn a_second_signal_without_wiring_names_that_signal() {
    let mut c = two_block_input();
    let (_, second) = two_block_ids(&c.line);
    c.wiring.pop();
    let result = compile_block_signals(&c);
    assert_eq!(scoped(&result, CompileIssueCode::WiringMissing).signal_id.as_deref(), Some(second.as_str()));
    assert_blocked(&result);
}

#[test]
fn a_new_signal_takes_the_lowest_free_mast_another_signal_does_not_hold() {
    let mut c = two_block_input();
    let (first, second) = two_block_ids(&c.line);
    // The saved first signal keeps Mast 2; the new second signal takes Mast 1.
    c.nodes[0].masts[0].link_address = Some(LINK_B);
    own_first_mast(&mut c, 2);
    c.saved_resources = vec![held(&first, 2, vec![])];
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(result.resources, vec![held(&first, 2, vec![1]), held(&second, 1, vec![])]);
}

#[test]
fn a_saved_signal_keeps_its_circuit_and_a_new_need_takes_the_lowest_untaken() {
    let mut c = two_block_input();
    let (first, _) = two_block_ids(&c.line);
    c.nodes[0].track_circuits[0].in_use = true;
    c.nodes[0].track_circuits[1].in_use = true;
    c.nodes[0].track_circuits[1].claimed = true;
    c.saved_resources = vec![held(&first, 1, vec![2])];
    own_first_mast(&mut c, 1);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(result.resources[0].resources.track_circuits, vec![2]);

    let mut fresh = two_block_input();
    fresh.nodes[0].track_circuits[0].in_use = true;
    let result = compile_block_signals(&fresh);
    assert_eq!(result.resources[0].resources.track_circuits, vec![2]);
}

#[test]
fn no_free_track_circuit_and_unread_link_address_block_the_first_signal() {
    let (first, _) = two_block_ids(&two_block_line());

    let mut c = two_block_input();
    c.nodes[0].track_circuits.iter_mut().for_each(|t| t.in_use = true);
    let result = compile_block_signals(&c);
    assert_eq!(scoped(&result, CompileIssueCode::NoFreeTrackCircuit).signal_id.as_deref(), Some(first.as_str()));
    assert_blocked(&result);

    for link in [None, Some([0u8; 8])] {
        let mut c = two_block_input();
        c.nodes[0].masts[1].link_address = link;
        let result = compile_block_signals(&c);
        let issue = scoped(&result, CompileIssueCode::LinkAddressMissing);
        assert_eq!(issue.signal_id.as_deref(), Some(first.as_str()));
        assert!(issue.detail.contains("Second signal"), "{}", issue.detail);
        assert_blocked(&result);
    }

    let mut c = two_block_input();
    c.nodes[0].masts[0].set_aspect_events[1] = None;
    let result = compile_block_signals(&c);
    assert_eq!(scoped(&result, CompileIssueCode::AspectEventsMissing).signal_id.as_deref(), Some(first.as_str()));
}

/// Function, Description and the eight event leaves of a Logic reset.
const LOGIC_RESET_EDITS: usize = 10;

#[test]
fn a_saved_one_block_facility_gaining_a_block_moves_its_group_and_resets_the_old_logic_last() {
    // The saved one-block group sits at Logic 1 and another facility holds Logic 2, so the
    // group moves to the lowest free run.
    let mut c = two_block_input();
    let (first, second) = two_block_ids(&c.line);
    own_first_mast(&mut c, 1);
    c.nodes[0].masts[0].link_address = Some(LINK_B);
    c.saved_resources = vec![held(&first, 1, vec![])];
    for l in c.nodes[0].logic.iter_mut().take(2) {
        use_logic(l);
        l.claimed = true;
    }
    c.logic_placement = vec![NodeLogicPlacement {
        node_key: NODE.into(),
        groups: vec![
            group(&first, 1, 1),
            PlacedLogicGroup { facility_id: "abs:other".into(), signal_id: "signal:other".into(), first: 2, length: 1 },
        ],
    }];
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    let groups = &result.placement[0].groups;
    assert!(groups.contains(&group(&first, 3, 2)), "{groups:?}");
    assert!(groups.contains(&group(&second, 5, 1)), "{groups:?}");
    let p = pairs(&result);
    let function_at = |logic: u32, value: i64| {
        p.iter().position(|(path, v)| path == &format!("Conditionals/Logic[{logic}]/Function") && *v == int(value))
    };
    let terminator = function_at(4, 2).expect("terminator written");
    let new_group = function_at(3, 1).expect("new group written");
    let old_reset = function_at(1, 0).expect("old Logic reset");
    assert!(terminator < new_group && new_group < old_reset);
    assert_eq!(old_reset, p.len() - LOGIC_RESET_EDITS, "the old Logic reset is the last edit set");
    assert!(p.iter().all(|(path, _)| !path.starts_with("Conditionals/Logic[2]")), "another facility is untouched");
}

#[test]
fn a_saved_two_block_facility_recompiled_as_one_block_shrinks_and_releases_after_the_recipe() {
    let mut c = input();
    let sid = signal_id(&c.line);
    c.saved_resources = vec![held(&sid, 1, vec![3])];
    c.logic_placement = vec![NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&sid, 1, 2)] }];
    c.nodes[0].masts[0].in_use = true;
    c.nodes[0].masts[0].claimed = true;
    for l in c.nodes[0].logic.iter_mut().take(2) {
        use_logic(l);
        l.claimed = true;
    }
    c.nodes[0].track_circuits = (1..=8)
        .map(|i| TrackCircuitFact { index: i, in_use: i == 3, claimed: i == 3, link_address: None })
        .collect();
    for o in c.hardware.hosts[0].outputs.iter_mut().filter(|o| o.ordinal == 9 || o.ordinal == 10) {
        o.owner = Some(SignalOutputOwner::Mast(1));
    }
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(result.resources, vec![held(&sid, 1, vec![])]);
    assert_eq!(result.placement, vec![placement_of(&[1])]);
    let p = pairs(&result);
    let recipe = expected_edits(1, 1, 10, 9);
    assert_eq!(&p[..recipe.len()], &recipe[..]);
    let rest: Vec<&str> = p[recipe.len()..].iter().map(|(path, _)| path.as_str()).collect();
    assert!(rest[0].starts_with("Conditionals/Logic[2]/"), "{rest:?}");
    assert_eq!(rest.len(), LOGIC_RESET_EDITS + 2);
    assert!(rest[rest.len() - 2].starts_with("Track Circuits/Circuit[3]/Remote Mast Description"));
    assert_eq!(p.last().unwrap().1, EditValue::FreshEventId);
}

#[test]
fn no_free_logic_names_the_signal_that_does_not_fit() {
    // Only Logic 1 and 2 are free: the first signal's two-rung group takes both and the
    // second signal's one Logic has nowhere to go.
    let mut c = two_block_input();
    let (first, second) = two_block_ids(&c.line);
    c.nodes[0].logic.iter_mut().skip(2).for_each(use_logic);
    let result = compile_block_signals(&c);
    let issue = scoped(&result, CompileIssueCode::NoFreeLogic);
    assert_eq!(issue.signal_id.as_deref(), Some(second.as_str()));
    assert_ne!(issue.signal_id.as_deref(), Some(first.as_str()));
    assert_blocked(&result);
}

#[test]
fn an_unread_link_address_on_the_next_board_names_the_next_signal_and_that_board() {
    let (first, _) = two_block_ids(&two_block_line());
    let mut c = two_board_input();
    c.nodes[1].masts[0].link_address = None;
    // The first board's own mast 2 Link Address must not stand in for the next board's.
    c.nodes[0].masts[1].link_address = Some(LINK_B);
    let result = compile_block_signals(&c);
    let issue = scoped(&result, CompileIssueCode::LinkAddressMissing);
    assert_eq!(issue.signal_id.as_deref(), Some(first.as_str()));
    assert!(issue.detail.contains("Second signal") && issue.detail.contains(NODE_B), "{}", issue.detail);
    assert_blocked(&result);
}

#[test]
fn track_circuits_and_masts_are_numbered_per_board() {
    // Board A's circuit 1 and mast 1 are the saved first signal's; the second signal on board B still takes B's mast 1.
    let mut c = two_board_input();
    let (first, second) = two_block_ids(&c.line);
    own_first_mast(&mut c, 1);
    c.saved_resources = vec![held(&first, 1, vec![1])];
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(result.resources[0].resources.track_circuits, vec![1]);
    assert_eq!(result.resources[1].resources.mast, 1);
    assert_eq!(result.resources[1].signal_id, second);
}

#[test]
fn a_resize_on_one_board_resets_vacated_logic_there_and_emits_nothing_extra_on_the_other() {
    let mut c = two_board_input();
    let (first, second) = two_block_ids(&c.line);
    own_first_mast(&mut c, 1);
    c.saved_resources = vec![held(&first, 1, vec![]), PlannedSignalResources {
        signal_id: second.clone(),
        resources: SignalResources { node_key: NODE_B.into(), mast: 1, track_circuits: vec![] },
    }];
    // The first signal was a lone one-rung group at Logic 3; it now needs two rungs.
    c.logic_placement = vec![
        NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&first, 3, 1)] },
        NodeLogicPlacement { node_key: NODE_B.into(), groups: vec![group(&second, 1, 1)] },
    ];
    c.nodes[1].masts[0].in_use = true;
    c.nodes[1].masts[0].claimed = true;
    // Logic 4 is configured outside Bowties, so the group cannot grow in place.
    use_logic(&mut c.nodes[0].logic[3]);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    let on_b: Vec<&NodeConfigEdit> = result.edits.iter().filter(|e| e.node_key == NODE_B).collect();
    assert!(
        on_b.iter().all(|e| e.path.starts_with("Rule to Aspect/Mast[1]") || e.path.starts_with("Conditionals/Logic[1]")),
        "{:?}",
        on_b.iter().map(|e| &e.path).collect::<Vec<_>>()
    );
    let reset_on_a = result
        .edits
        .iter()
        .filter(|e| e.node_key == NODE && e.path.starts_with("Conditionals/Logic[3]"))
        .count();
    assert_eq!(result.placement[0].groups, vec![group(&first, 1, 2)]);
    assert_eq!(reset_on_a, LOGIC_RESET_EDITS);
    assert_eq!(result.placement[1].groups, vec![group(&second, 1, 1)]);
}

#[test]
fn a_claimed_mast_missing_on_its_own_board_blames_that_signal_and_board() {
    let mut c = two_board_input();
    let (first, second) = two_block_ids(&c.line);
    own_first_mast(&mut c, 1);
    // The second signal's saved mast 5 exists on board A but not on board B.
    c.nodes[1].masts.retain(|m| m.index != 5);
    c.saved_resources = vec![held(&first, 1, vec![]), PlannedSignalResources {
        signal_id: second.clone(),
        resources: SignalResources { node_key: NODE_B.into(), mast: 5, track_circuits: vec![] },
    }];
    let result = compile_block_signals(&c);
    let issue = scoped(&result, CompileIssueCode::ClaimedSlotUnavailable);
    assert_eq!(issue.signal_id.as_deref(), Some(second.as_str()));
    assert_eq!(issue.node_key.as_deref(), Some(NODE_B));
    assert_blocked(&result);
}

fn blocks_line(ids: &[&str]) -> BlockSignalsRequest {
    let mut line = line();
    line.blocks = ids
        .iter()
        .map(|id| SelectedBlock {
            block_id: format!("block:{id}"),
            name: format!("Block {}", id.to_uppercase()),
            joined_to_previous: false,
            turnouts: vec![],
        })
        .collect();
    line
}

fn ids_of(line: &BlockSignalsRequest) -> Vec<String> {
    bowties_core::signaling::preview_block_signals(line.clone()).signals.into_iter().map(|s| s.signal_id).collect()
}

fn own_pins(c: &mut CompileInput, board: &str, pins: [u32; 2], mast: u32) {
    let host = c.hardware.hosts.iter_mut().find(|h| h.node_key == board).unwrap();
    for o in host.outputs.iter_mut().filter(|o| pins.contains(&o.ordinal)) {
        o.owner = Some(SignalOutputOwner::Mast(mast));
    }
}

fn occupancy_of(id: &str, n: u8) -> BlockOccupancyEvents {
    BlockOccupancyEvents {
        block_id: format!("block:{id}"),
        occupied: Some([1, 1, 1, 1, 1, 1, n, 1]),
        clear: Some([1, 1, 1, 1, 1, 1, n, 2]),
    }
}

/// A saved facility of blocks A, B, C (masts 1-3, pins 9-14, circuits 1 and 2,
/// Logic 1-2, 3-4, 5) edited to blocks D, A, C: the middle signal is dropped and
/// the new first signal D is wired to the dropped signal's pins 11 and 12.
fn dropped_middle_input() -> CompileInput {
    let saved = ids_of(&blocks_line(&["a", "b", "c"]));
    let line = blocks_line(&["d", "a", "c"]);
    let new = ids_of(&line);
    let mut node = free_node();
    for (i, m) in node.masts.iter_mut().enumerate() {
        m.link_address = Some([3, 3, 3, 3, 3, 3, 0, i as u8 + 1]);
        m.in_use = i < 3;
        m.claimed = i < 3;
    }
    for l in node.logic.iter_mut().take(5) {
        use_logic(l);
        l.claimed = true;
    }
    node.track_circuits = (1..=8)
        .map(|i| TrackCircuitFact { index: i, in_use: i <= 2, claimed: i <= 2, link_address: None })
        .collect();
    let wire = |id: &String, red, green| SignalWiring {
        signal_id: id.clone(),
        node_key: NODE.into(),
        red_output: red,
        green_output: green,
    };
    let mut c = CompileInput {
        wiring: vec![wire(&new[0], 12, 11), wire(&new[1], 10, 9), wire(&new[2], 14, 13)],
        line,
        hardware: SignalHardwareFacts { hosts: vec![host()] },
        nodes: vec![node],
        occupancy: vec![occupancy_of("d", 4), occupancy_of("a", 1), occupancy_of("c", 3)],
        saved_resources: vec![held(&saved[0], 1, vec![1]), held(&saved[1], 2, vec![2]), held(&saved[2], 3, vec![])],
        logic_placement: vec![NodeLogicPlacement {
            node_key: NODE.into(),
            groups: vec![group(&saved[0], 1, 2), group(&saved[1], 3, 2), group(&saved[2], 5, 1)],
        }],
    };
    own_pins(&mut c, NODE, [9, 10], 1);
    own_pins(&mut c, NODE, [11, 12], 2);
    own_pins(&mut c, NODE, [13, 14], 3);
    c
}

#[test]
fn a_dropped_signals_pins_are_free_and_its_mast_circuit_and_logic_are_reset() {
    let c = dropped_middle_input();
    let saved_b = c.saved_resources[1].signal_id.clone();
    let new = ids_of(&c.line);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);

    let resource_ids: Vec<&str> = result.resources.iter().map(|r| r.signal_id.as_str()).collect();
    assert_eq!(resource_ids, vec![new[0].as_str(), new[1].as_str(), new[2].as_str()]);
    assert!(!resource_ids.contains(&saved_b.as_str()));
    let mast_of = |id: &String| result.resources.iter().find(|r| &r.signal_id == id).unwrap().resources.mast;
    assert_eq!((mast_of(&new[0]), mast_of(&new[1]), mast_of(&new[2])), (4, 1, 3));

    let p = pairs(&result);
    let at = |path: &str, value: EditValue| p.iter().position(|(q, v)| q == path && *v == value);
    let mast_reset = at("Rule to Aspect/Mast[2]/Mast Processing", int(0)).expect("dropped mast reset");
    let circuit_reset =
        at("Track Circuits/Circuit[2]/Link Address", EditValue::FreshEventId).expect("dropped circuit reset");
    let last_recipe = p.iter().rposition(|(q, _)| q.starts_with("Rule to Aspect/Mast[4]")).unwrap();
    assert!(mast_reset > last_recipe && circuit_reset > last_recipe, "resets follow the recipe");

    // Each Logic of the dropped group is either reused by the new placement or reset.
    let placed = &result.placement[0].groups;
    for logic in [3u32, 4] {
        let reused = placed.iter().any(|g| (g.first..g.first + g.length).contains(&logic));
        let reset = at(&format!("Conditionals/Logic[{logic}]/Function"), int(0)).is_some();
        assert!(reused != reset, "Logic {logic}: reused {reused}, reset {reset}");
    }
    assert!(placed.iter().all(|g| g.signal_id != saved_b));
}

#[test]
fn surviving_signals_may_swap_pins_and_keep_their_masts() {
    let mut c = two_block_input();
    let (first, second) = two_block_ids(&c.line);
    c.nodes[0].masts[0].link_address = Some(LINK_B);
    for m in c.nodes[0].masts.iter_mut().take(2) {
        m.in_use = true;
        m.claimed = true;
    }
    c.nodes[0].track_circuits[0].in_use = true;
    c.nodes[0].track_circuits[0].claimed = true;
    for l in c.nodes[0].logic.iter_mut().take(3) {
        use_logic(l);
        l.claimed = true;
    }
    c.saved_resources = vec![held(&first, 1, vec![1]), held(&second, 2, vec![])];
    c.logic_placement =
        vec![NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&first, 1, 2), group(&second, 3, 1)] }];
    own_pins(&mut c, NODE, [9, 10], 1);
    own_pins(&mut c, NODE, [11, 12], 2);
    c.wiring[0].red_output = 12;
    c.wiring[0].green_output = 11;
    c.wiring[1].red_output = 10;
    c.wiring[1].green_output = 9;
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(result.resources, vec![held(&first, 1, vec![1]), held(&second, 2, vec![])]);

    // A pin held by another facility's mast still blocks.
    let mut other = c.clone();
    own_pins(&mut other, NODE, [11, 12], 7);
    let blocked = compile_block_signals(&other);
    assert!(codes(&blocked).contains(&CompileIssueCode::OutputInUse), "{:?}", blocked.issues);
    assert!(blocked.edits.is_empty());
}

#[test]
fn a_dropped_signal_alone_on_a_board_vacates_that_boards_group_and_mast() {
    let mut c = two_board_input();
    let (first, second) = two_block_ids(&c.line);
    own_first_mast(&mut c, 1);
    c.nodes[0].track_circuits[0].in_use = true;
    c.nodes[0].track_circuits[0].claimed = true;
    for l in c.nodes[0].logic.iter_mut().take(2) {
        use_logic(l);
        l.claimed = true;
    }
    c.nodes[1].masts[0].in_use = true;
    c.nodes[1].masts[0].claimed = true;
    use_logic(&mut c.nodes[1].logic[0]);
    c.nodes[1].logic[0].claimed = true;
    c.saved_resources = vec![
        held(&first, 1, vec![1]),
        PlannedSignalResources {
            signal_id: second.clone(),
            resources: SignalResources { node_key: NODE_B.into(), mast: 1, track_circuits: vec![] },
        },
    ];
    c.logic_placement = vec![
        NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&first, 1, 2)] },
        NodeLogicPlacement { node_key: NODE_B.into(), groups: vec![group(&second, 1, 1)] },
    ];
    // The edited facility keeps only block A.
    c.line = blocks_line(&["a"]);
    c.line.signal_names.insert(first.clone(), "First signal".into());
    c.wiring.truncate(1);
    c.occupancy.truncate(1);

    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    assert_eq!(result.resources, vec![held(&first, 1, vec![])]);
    let on_b: Vec<&NodeConfigEdit> = result.edits.iter().filter(|e| e.node_key == NODE_B).collect();
    assert!(on_b.iter().any(|e| e.path == "Conditionals/Logic[1]/Function" && e.value == int(0)), "Logic reset on board 2");
    assert!(
        on_b.iter().any(|e| e.path == "Rule to Aspect/Mast[1]/Mast Processing" && e.value == int(0)),
        "mast reset on board 2"
    );
    assert_eq!(
        result.placement,
        vec![
            NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&first, 1, 1)] },
            NodeLogicPlacement { node_key: NODE_B.into(), groups: vec![] },
        ]
    );
}

const OCC_J: [u8; 8] = [1, 1, 1, 1, 1, 1, 0, 5];
const CLR_J: [u8; 8] = [1, 1, 1, 1, 1, 1, 0, 6];

/// Blocks A, B (joined to A), C: the first signal protects A and B.
fn joined_line() -> BlockSignalsRequest {
    let mut line = blocks_line(&["a", "b", "c"]);
    line.blocks[1].joined_to_previous = true;
    line
}

fn joined_input() -> CompileInput {
    let mut c = two_block_input();
    c.line = joined_line();
    let ids = ids_of(&c.line);
    assert_eq!(ids.len(), 2);
    c.line.signal_names.insert(ids[0].clone(), "First signal".into());
    c.line.signal_names.insert(ids[1].clone(), "Second signal".into());
    c.wiring[0].signal_id = ids[0].clone();
    c.wiring[1].signal_id = ids[1].clone();
    c.occupancy = vec![
        BlockOccupancyEvents { block_id: "block:a".into(), occupied: Some(OCC), clear: Some(CLR) },
        BlockOccupancyEvents { block_id: "block:b".into(), occupied: Some(OCC_J), clear: Some(CLR_J) },
        BlockOccupancyEvents { block_id: "block:c".into(), occupied: Some(OCC_B), clear: Some(CLR_B) },
    ];
    c
}

/// One Group rung that sends Stop (mast 1) when `occupied`/`clear` say so.
fn group_stop_rung(logic: u32, description: &str, occupied: [u8; 8], clear: [u8; 8]) -> Vec<(String, EditValue)> {
    let mut rung = internal_edits("First signal", "Second signal", 1, logic, 1).split_off(19);
    for (path, value) in rung.iter_mut() {
        if path.ends_with("/Description") {
            *value = EditValue::String { value: description.into() };
        } else if path.ends_with("Variable #1/set true") {
            *value = EditValue::EventId { bytes: occupied };
        } else if path.ends_with("Variable #1/set false") {
            *value = EditValue::EventId { bytes: clear };
        }
    }
    rung
}

#[test]
fn an_internal_signal_protecting_two_joined_blocks_gets_a_group_rung_per_block_then_the_circuit_rung() {
    let input = joined_input();
    let ids = ids_of(&input.line);
    let result = compile_block_signals(&input);
    assert_eq!(result.issues, vec![]);

    let mut expected = mast_part(expected_edits_named("First signal", 1, 1, 10, 9));
    // Circuit and the Last (Single) rung (Logic 3) first, then Group rungs from the highest down.
    expected.extend(internal_edits("First signal", "Second signal", 1, 2, 1).into_iter().take(19));
    expected.extend(group_stop_rung(2, "First signal occupancy 2", OCC_J, CLR_J));
    expected.extend(group_stop_rung(1, "First signal occupancy 1", OCC, CLR));
    expected.extend(last_edits(2, 4));
    assert_eq!(pairs(&result), expected);
    assert_eq!(
        result.placement,
        vec![NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&ids[0], 1, 3), group(&ids[1], 4, 1)] }]
    );
}

#[test]
fn a_terminal_signal_protecting_two_joined_blocks_gets_a_group_rung_then_a_last_rung_on_the_last_block() {
    let mut line = blocks_line(&["a", "b"]);
    line.blocks[1].joined_to_previous = true;
    let id = ids_of(&line).remove(0);
    let mut c = input();
    c.line = line;
    c.line.signal_names.insert(id.clone(), "Main signal".into());
    c.wiring[0].signal_id = id.clone();
    c.occupancy = vec![
        BlockOccupancyEvents { block_id: "block:a".into(), occupied: Some(OCC), clear: Some(CLR) },
        BlockOccupancyEvents { block_id: "block:b".into(), occupied: Some(OCC_J), clear: Some(CLR_J) },
    ];
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);

    let full = expected_edits_named("Main signal", 1, 2, 10, 9);
    let mast_len = mast_part(full.clone()).len();
    let mut expected = full[..mast_len].to_vec();
    let mut last: Vec<_> = full[mast_len..].to_vec();
    for (path, value) in last.iter_mut() {
        if path.ends_with("Variable #1/set true") {
            *value = EditValue::EventId { bytes: OCC_J };
        } else if path.ends_with("Variable #1/set false") {
            *value = EditValue::EventId { bytes: CLR_J };
        } else if path.ends_with("/Description") {
            *value = EditValue::String { value: "Main signal occupancy 2".into() };
        }
    }
    expected.extend(last);
    let mut rung = group_stop_rung(1, "Main signal occupancy 1", OCC, CLR);
    for (path, value) in rung.iter_mut() {
        if path.ends_with("Action Event") {
            *value = EditValue::EventId { bytes: set_aspect(1, 1) };
        }
    }
    expected.extend(rung);
    assert_eq!(pairs(&result), expected);
    assert_eq!(
        result.placement,
        vec![NodeLogicPlacement { node_key: NODE.into(), groups: vec![group(&id, 1, 2)] }]
    );
}

#[test]
fn a_joined_block_without_occupancy_events_blocks_the_signal_that_protects_it() {
    let mut c = joined_input();
    let ids = ids_of(&c.line);
    c.occupancy[1] = BlockOccupancyEvents { block_id: "block:b".into(), occupied: None, clear: None };
    let result = compile_block_signals(&c);
    let issue = scoped(&result, CompileIssueCode::OccupancyEventsMissing);
    assert_eq!(issue.signal_id.as_deref(), Some(ids[0].as_str()));
    assert_blocked(&result);
}

// ---- Editing a saved facility: reorder, prepend, middle removal, growing group ----

fn mast_link(mast: u32) -> EditValue {
    EditValue::EventId { bytes: [3, 3, 3, 3, 3, 3, 0, mast as u8] }
}

fn value_at(c: &CompiledBlockSignals, path: &str) -> Option<EditValue> {
    c.edits.iter().rev().find(|e| e.path == path).map(|e| e.value.clone())
}

/// A saved facility over `saved` blocks (one signal each, pins 9-10, 11-12, ...,
/// masts 1.., circuits 1.. for all but the last, consecutive Logic groups) edited
/// to `line`, whose signals enter at `entrances`. A block new to the facility is
/// wired to pins 15 and 16.
fn saved_edit(saved: &[&str], mut line: BlockSignalsRequest, entrances: &[&str]) -> (CompileInput, Vec<String>) {
    let saved_ids = ids_of(&blocks_line(saved));
    line.saved_signals = saved_ids
        .iter()
        .zip(saved)
        .map(|(id, b)| bowties_core::signaling::SavedSignalIdentity {
            signal_id: id.clone(),
            protected_block: format!("block:{b}"),
        })
        .collect();
    let new_ids = ids_of(&line);
    let n = saved.len() as u32;
    let mut node = free_node();
    for (i, m) in node.masts.iter_mut().enumerate() {
        m.link_address = Some([3, 3, 3, 3, 3, 3, 0, i as u8 + 1]);
        m.in_use = (i as u32) < n;
        m.claimed = m.in_use;
    }
    let logic_used = 2 * (n - 1) + 1;
    for l in node.logic.iter_mut().take(logic_used as usize) {
        use_logic(l);
        l.claimed = true;
    }
    node.track_circuits = (1..=8)
        .map(|i| TrackCircuitFact { index: i, in_use: i < n, claimed: i < n, link_address: None })
        .collect();
    let pins = |block: &str| match saved.iter().position(|b| *b == block) {
        Some(k) => (10 + 2 * k as u32, 9 + 2 * k as u32),
        None => (16, 15),
    };
    let wiring = new_ids
        .iter()
        .zip(entrances)
        .map(|(id, b)| {
            let (red, green) = pins(b);
            SignalWiring { signal_id: id.clone(), node_key: NODE.into(), red_output: red, green_output: green }
        })
        .collect();
    let occupancy = line
        .blocks
        .iter()
        .map(|b| occupancy_of(&b.block_id[6..], b.block_id.as_bytes()[6] - b'a' + 1))
        .collect();
    let mut groups = vec![];
    let mut next = 1;
    for (k, id) in saved_ids.iter().enumerate() {
        let length = if (k as u32) < n - 1 { 2 } else { 1 };
        groups.push(group(id, next, length));
        next += length;
    }
    let mut c = CompileInput {
        wiring,
        line,
        hardware: SignalHardwareFacts { hosts: vec![host()] },
        nodes: vec![node],
        occupancy,
        saved_resources: saved_ids
            .iter()
            .enumerate()
            .map(|(k, id)| held(id, k as u32 + 1, if (k as u32) < n - 1 { vec![k as u32 + 1] } else { vec![] }))
            .collect(),
        logic_placement: vec![NodeLogicPlacement { node_key: NODE.into(), groups }],
    };
    for k in 0..n {
        own_pins(&mut c, NODE, [9 + 2 * k, 10 + 2 * k], k + 1);
    }
    (c, saved_ids)
}

fn resources_of<'a>(c: &'a CompiledBlockSignals, id: &str) -> &'a SignalResources {
    &c.resources.iter().find(|r| r.signal_id == id).unwrap_or_else(|| panic!("{id} in {:?}", c.resources)).resources
}

#[test]
fn reordering_a_saved_facility_keeps_ids_masts_and_circuits_and_relinks_to_the_new_next_signal() {
    // Passed on first run (bench-proven).
    let (c, saved) = saved_edit(&["a", "b", "c"], blocks_line(&["c", "a", "b"]), &["c", "a", "b"]);
    let (a, b, cc) = (&saved[0], &saved[1], &saved[2]);
    assert_eq!(ids_of(&c.line), vec![cc.clone(), a.clone(), b.clone()]);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);

    assert_eq!(resources_of(&result, a).mast, 1);
    assert_eq!(resources_of(&result, b).mast, 2);
    assert_eq!(resources_of(&result, cc).mast, 3);
    // A keeps circuit 1 and now links to B's mast; B is terminal and releases circuit 2.
    assert_eq!(resources_of(&result, a).track_circuits, vec![1]);
    assert_eq!(value_at(&result, "Track Circuits/Circuit[1]/Link Address"), Some(mast_link(2)));
    assert_eq!(resources_of(&result, b).track_circuits, Vec::<u32>::new());
    assert_eq!(value_at(&result, "Track Circuits/Circuit[2]/Link Address"), Some(EditValue::FreshEventId));
    // C was terminal and is now internal: it takes the lowest free circuit and links to A's mast.
    assert_eq!(resources_of(&result, cc).track_circuits, vec![3]);
    assert_eq!(value_at(&result, "Track Circuits/Circuit[3]/Link Address"), Some(mast_link(1)));
}

#[test]
fn prepending_a_block_to_a_saved_facility_leaves_the_saved_signals_untouched() {
    // Passed on first run (bench-proven).
    let (c, saved) = saved_edit(&["a", "b"], blocks_line(&["x", "a", "b"]), &["x", "a", "b"]);
    let (a, b) = (&saved[0], &saved[1]);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);
    let x = &ids_of(&c.line)[0];

    assert_eq!(resources_of(&result, a), &SignalResources { node_key: NODE.into(), mast: 1, track_circuits: vec![1] });
    assert_eq!(resources_of(&result, b), &SignalResources { node_key: NODE.into(), mast: 2, track_circuits: vec![] });
    let new = resources_of(&result, x);
    assert_eq!((new.mast, new.track_circuits.clone()), (3, vec![2]));
    assert_eq!(value_at(&result, "Track Circuits/Circuit[2]/Link Address"), Some(mast_link(1)));

    let groups = &result.placement[0].groups;
    assert!(groups.contains(&group(a, 1, 2)) && groups.contains(&group(b, 3, 1)), "{groups:?}");
    let xg = groups.iter().find(|g| &g.signal_id == x).unwrap();
    assert!(xg.first >= 4 && xg.length == 2, "{groups:?}");
    // A's circuit still links to B's mast, and nothing is reset.
    assert_eq!(value_at(&result, "Track Circuits/Circuit[1]/Link Address"), Some(mast_link(2)));
    assert!(!result.edits.iter().any(|e| e.value == EditValue::FreshEventId && !e.path.contains("Circuit[2]")));
    assert!(!result.edits.iter().any(|e| e.path.ends_with("/Function") && e.value == int(0)), "no Logic is reset");
}

#[test]
fn removing_the_middle_block_relinks_the_first_signal_and_resets_the_dropped_one() {
    // Passed on first run (bench-proven).
    let (c, saved) = saved_edit(&["a", "b", "c"], blocks_line(&["a", "c"]), &["a", "c"]);
    let (a, b, cc) = (&saved[0], &saved[1], &saved[2]);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);

    assert_eq!(resources_of(&result, a), &SignalResources { node_key: NODE.into(), mast: 1, track_circuits: vec![1] });
    assert_eq!(value_at(&result, "Track Circuits/Circuit[1]/Link Address"), Some(mast_link(3)));
    assert_eq!(resources_of(&result, cc), &SignalResources { node_key: NODE.into(), mast: 3, track_circuits: vec![] });
    assert!(result.resources.iter().all(|r| &r.signal_id != b));

    assert_eq!(value_at(&result, "Rule to Aspect/Mast[2]/Mast Processing"), Some(int(0)));
    assert_eq!(value_at(&result, "Track Circuits/Circuit[2]/Link Address"), Some(EditValue::FreshEventId));
    let placed = &result.placement[0].groups;
    assert!(placed.iter().all(|g| &g.signal_id != b));
    assert!(placed.contains(&group(a, 1, 2)), "{placed:?}");
    for logic in [3u32, 4] {
        let reused = placed.iter().any(|g| (g.first..g.first + g.length).contains(&logic));
        let reset = value_at(&result, &format!("Conditionals/Logic[{logic}]/Function")) == Some(int(0));
        assert!(reused != reset, "Logic {logic}: reused {reused}, reset {reset}");
    }
}

#[test]
fn a_saved_group_grows_from_two_rungs_to_three_when_the_next_block_is_joined() {
    // Passed on first run (bench-proven).
    let mut line = blocks_line(&["a", "b", "c"]);
    line.blocks[1].joined_to_previous = true;
    let (c, saved) = saved_edit(&["a", "b", "c"], line, &["a", "c"]);
    let (a, b, cc) = (&saved[0], &saved[1], &saved[2]);
    assert_eq!(ids_of(&c.line), vec![a.clone(), cc.clone()]);
    let result = compile_block_signals(&c);
    assert_eq!(result.issues, vec![]);

    let placed = &result.placement[0].groups;
    let ga = placed.iter().find(|g| &g.signal_id == a).unwrap_or_else(|| panic!("{placed:?}"));
    assert_eq!(ga.length, 3, "{placed:?}");
    for rung in 0..3 {
        let logic = ga.first + rung;
        assert!(
            result.edits.iter().any(|e| e.path == format!("Conditionals/Logic[{logic}]/Function") && e.value != int(0)),
            "Logic {logic} written"
        );
    }
    assert_eq!(resources_of(&result, a), &SignalResources { node_key: NODE.into(), mast: 1, track_circuits: vec![1] });
    assert_eq!(value_at(&result, "Track Circuits/Circuit[1]/Link Address"), Some(mast_link(3)));
    assert_eq!(resources_of(&result, cc).mast, 3);

    // B's dropped signal releases its mast and circuit.
    assert!(result.resources.iter().all(|r| &r.signal_id != b));
    assert_eq!(value_at(&result, "Rule to Aspect/Mast[2]/Mast Processing"), Some(int(0)));
    assert_eq!(value_at(&result, "Track Circuits/Circuit[2]/Link Address"), Some(EditValue::FreshEventId));
    assert!(placed.iter().all(|g| &g.signal_id != b));
    // Every old Logic is either inside a placed group or reset, never both.
    for logic in 1u32..=5 {
        let reused = placed.iter().any(|g| (g.first..g.first + g.length).contains(&logic));
        let reset = value_at(&result, &format!("Conditionals/Logic[{logic}]/Function")) == Some(int(0));
        assert!(!(reused && reset), "Logic {logic} both reused and reset");
        assert!(reused || reset, "Logic {logic} neither reused nor reset: {placed:?}");
    }
}

#[test]
fn two_signals_choosing_the_same_output_on_one_board_each_name_the_other_and_block() {
    let mut c = two_block_input();
    let (first, second) = two_block_ids(&c.line);
    c.wiring[1].green_output = 9;
    let result = compile_block_signals(&c);
    let in_use: Vec<(Option<&str>, &str)> = result
        .issues
        .iter()
        .filter(|i| i.code == CompileIssueCode::OutputInUse)
        .map(|i| (i.signal_id.as_deref(), i.detail.as_str()))
        .collect();
    assert_eq!(
        in_use,
        vec![
            (Some(first.as_str()), "Output 9 is already used by Second signal."),
            (Some(second.as_str()), "Output 9 is already used by First signal."),
        ]
    );
    let held: Vec<_> = result
        .issues
        .iter()
        .filter(|i| i.code == CompileIssueCode::OutputInUse)
        .map(|i| i.binding_issue.clone())
        .collect();
    assert_eq!(
        held,
        vec![
            Some(bowties_core::signaling::ProposalIssue::SignalOutputHeldBySignal {
                signal_id: first.clone(),
                node_key: c.wiring[0].node_key.clone(),
                output: 9,
                holder_signal_id: second.clone(),
                holder_name: "Second signal".to_string(),
            }),
            Some(bowties_core::signaling::ProposalIssue::SignalOutputHeldBySignal {
                signal_id: second.clone(),
                node_key: c.wiring[1].node_key.clone(),
                output: 9,
                holder_signal_id: first.clone(),
                holder_name: "First signal".to_string(),
            }),
        ]
    );
    assert_blocked(&result);

    // The same output number on different boards is fine.
    let mut other_boards = two_board_input();
    other_boards.wiring[1].red_output = 10;
    other_boards.wiring[1].green_output = 9;
    assert_eq!(compile_block_signals(&other_boards).issues, vec![]);
}
