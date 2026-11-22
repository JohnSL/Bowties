use bowties_core::layout::facilities::{ClaimedSlot, ClaimedSlotKind};
use bowties_core::signaling::compiler::*;
use bowties_core::signaling::{
    BlockSignalsRequest, SelectedBlock, SignalAppearanceSet, SignalDriver, SignalHardwareEvidence,
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
            .map(|ordinal| SignalOutput { ordinal, label: format!("#{ordinal}"), owner: None })
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
                set_aspect_events: [
                    Some(set_aspect(i as u8, 1)),
                    Some(set_aspect(i as u8, 2)),
                    Some(set_aspect(i as u8, 3)),
                ],
            })
            .collect(),
        logic: (1..=32).map(|i| LogicFact { index: i, in_use: false }).collect(),
        pending_edits: vec![],
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
    c.hardware.hosts[0].outputs[9].owner = Some("Direct lamp 3".into());
    cases.push(("output owned", c, OutputInUse, true));

    let mut c = input();
    c.wiring[0].green_output = c.wiring[0].red_output;
    cases.push(("duplicate pin", c, OutputDuplicate, true));

    let mut c = input();
    for o in c.hardware.hosts[0].outputs.iter_mut().skip(1) {
        o.owner = Some("Mast 4 rule 1".into());
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

    // A slot that is free in the facts but taken by an unsaved edit.
    let mut c = input();
    c.nodes[0].masts.iter_mut().skip(1).for_each(|m| m.in_use = true);
    c.nodes[0].pending_edits.push(PendingEdit {
        path: "Rule to Aspect/Mast[1]/Mast Processing".into(),
        value: int(1),
    });
    cases.push(("mast taken by unsaved edit", c, NoFreeMast, true));

    let mut c = input();
    c.nodes[0].logic.iter_mut().skip(1).for_each(|l| l.in_use = true);
    c.nodes[0].pending_edits.push(PendingEdit {
        path: "Conditionals/Logic[1]/Function".into(),
        value: int(2),
    });
    cases.push(("logic taken by unsaved edit", c, NoFreeLogic, true));

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
