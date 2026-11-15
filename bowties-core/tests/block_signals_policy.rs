use bowties_core::layout::channels::{ChannelBinding, ChannelRole};
use bowties_core::signaling::{
    preview_block_signals, preview_block_signals_with_hardware, ApproachLighting, BlockSignalsPreview,
    BlockSignalsRequest, EntranceEnd, ExitEnd, Movements, NextSignal, Observation,
    PlaceholderTurnout, ProposalIssue, RuleCondition, SelectedBlock, SignalAppearance,
    SignalAppearanceSet, SignalAspectRule, SignalBindingSelection, SignalDriver,
    SignalHardwareEvidence, SignalHardwareFacts, SignalHardwareHost, SignalIndication,
    SignalOutput, SignalOverride, SignalRun, SignalStyle, TravelDirection, TurnoutNotLined,
};
use serde_json::json;

const FACILITY: &str = "abs:westbound-main";
const WEST_YARD_EAST: &str = "signal:abs:westbound-main:block:west-yard:eastward";

fn block(block_id: &str, name: &str) -> SelectedBlock {
    SelectedBlock {
        block_id: block_id.to_string(),
        name: name.to_string(),
        joined_to_previous: false,
        turnouts: vec![],
    }
}

fn joined(block_id: &str, name: &str) -> SelectedBlock {
    SelectedBlock {
        joined_to_previous: true,
        ..block(block_id, name)
    }
}

fn with_turnout(mut selected: SelectedBlock, turnout_id: &str, name: &str) -> SelectedBlock {
    selected.turnouts.push(PlaceholderTurnout {
        turnout_id: turnout_id.to_string(),
        name: name.to_string(),
    });
    selected
}

fn request(blocks: Vec<SelectedBlock>) -> BlockSignalsRequest {
    BlockSignalsRequest {
        facility_id: FACILITY.to_string(),
        facility_name: "Westbound main".to_string(),
        blocks,
        ..Default::default()
    }
}

/// Cedar Hill, Mill curve and River bridge, written in eastward order.
fn three_blocks() -> Vec<SelectedBlock> {
    vec![
        block("block:cedar", "Cedar Hill"),
        block("block:mill", "Mill curve"),
        block("block:river", "River bridge"),
    ]
}

fn signal_ids(preview: &BlockSignalsPreview) -> Vec<&str> {
    preview.signals.iter().map(|signal| signal.signal_id.as_str()).collect()
}

fn rules(preview: &BlockSignalsPreview, index: usize) -> Vec<(RuleCondition, SignalIndication)> {
    preview.signals[index]
        .rules
        .iter()
        .map(|SignalAspectRule { condition, indication }| (condition.clone(), indication.clone()))
        .collect()
}

fn next_at_stop_rules() -> Vec<(RuleCondition, SignalIndication)> {
    vec![
        (RuleCondition::BlockOccupied, SignalIndication::Stop),
        (RuleCondition::ClearNextAtStop, SignalIndication::Approach),
        (RuleCondition::ClearNextPermissive, SignalIndication::Clear),
    ]
}

fn stop_clear_rules() -> Vec<(RuleCondition, SignalIndication)> {
    vec![
        (RuleCondition::BlockOccupied, SignalIndication::Stop),
        (RuleCondition::BlockClear, SignalIndication::Clear),
    ]
}

#[test]
fn derives_one_entrance_signal_per_block_in_travel_order() {
    let preview = preview_block_signals(request(three_blocks()));

    assert_eq!(
        signal_ids(&preview),
        [
            "signal:abs:westbound-main:block:cedar:eastward",
            "signal:abs:westbound-main:block:mill:eastward",
            "signal:abs:westbound-main:block:river:eastward",
        ]
    );
    assert_eq!(
        preview.signals.iter().map(|signal| signal.name.as_str()).collect::<Vec<_>>(),
        ["Cedar Hill East", "Mill curve East", "River bridge East"],
        "default names come from the entrance block and the direction governed"
    );
    assert!(preview.signals.iter().all(|signal| signal.governs == TravelDirection::Eastward
        && signal.run == SignalRun::WithList
        && signal.observation == Observation::Unavailable));
    assert_eq!(
        preview.signals[0].next,
        NextSignal::Signal { signal_id: "signal:abs:westbound-main:block:mill:eastward".to_string() }
    );
    assert_eq!(rules(&preview, 0), next_at_stop_rules());
    assert_eq!(preview.signals[2].next, NextSignal::NothingBeyond);
    assert_eq!(
        rules(&preview, 2),
        stop_clear_rules(),
        "with nothing beyond the last block, the last signal falls back to Stop / Clear"
    );
}

#[test]
fn keeps_signal_identity_across_renames_and_scopes_it_to_the_facility() {
    let initial = preview_block_signals(request(three_blocks()));
    let mut renamed = request(three_blocks());
    renamed.facility_name = "Arrival route".to_string();
    renamed
        .signal_names
        .insert("signal:abs:westbound-main:block:mill:eastward".to_string(), "Mill home".to_string());
    let renamed = preview_block_signals(renamed);
    let mut other = request(three_blocks());
    other.facility_id = "abs:yard-arrival".to_string();
    let other = preview_block_signals(other);

    assert_eq!(signal_ids(&renamed), signal_ids(&initial));
    assert_eq!(renamed.signals[1].name, "Mill home");
    assert_ne!(signal_ids(&other)[0], signal_ids(&initial)[0]);
}

#[test]
fn reversing_the_direction_keeps_the_list_and_governs_the_other_way() {
    let mut westward = request(three_blocks());
    westward.direction = TravelDirection::Westward;
    let preview = preview_block_signals(westward);

    assert_eq!(
        preview.blocks.iter().map(|block| block.block_id.as_str()).collect::<Vec<_>>(),
        ["block:cedar", "block:mill", "block:river"],
        "changing the direction never reorders the list"
    );
    assert_eq!(
        signal_ids(&preview),
        [
            "signal:abs:westbound-main:block:cedar:westward",
            "signal:abs:westbound-main:block:mill:westward",
            "signal:abs:westbound-main:block:river:westward",
        ]
    );
    assert_eq!(preview.signals[0].name, "Cedar Hill West");
}

#[test]
fn joined_blocks_share_one_signal_that_protects_every_block_and_turnout_in_them() {
    let preview = preview_block_signals(request(vec![
        with_turnout(block("block:cedar", "Cedar Hill"), "turnout:depot", "Depot track"),
        with_turnout(joined("block:mill", "Mill curve"), "turnout:mill-1", "Mill siding 1"),
        block("block:river", "River bridge"),
    ]));

    assert_eq!(
        signal_ids(&preview),
        [
            "signal:abs:westbound-main:block:cedar:eastward",
            "signal:abs:westbound-main:block:river:eastward",
        ]
    );
    let first = &preview.signals[0];
    assert_eq!(first.protected_block_ids, ["block:cedar", "block:mill"]);
    assert_eq!(first.turnout_ids, ["turnout:depot", "turnout:mill-1"]);
    assert_eq!(
        rules(&preview, 0),
        vec![
            (RuleCondition::BlockOccupied, SignalIndication::Stop),
            (RuleCondition::TurnoutNotLined { turnout_id: "turnout:depot".to_string() }, SignalIndication::Stop),
            (RuleCondition::TurnoutNotLined { turnout_id: "turnout:mill-1".to_string() }, SignalIndication::Stop),
            (RuleCondition::ClearNextAtStop, SignalIndication::Approach),
            (RuleCondition::ClearNextPermissive, SignalIndication::Clear),
        ]
    );
}

#[test]
fn the_first_block_always_starts_a_signal_block() {
    let preview = preview_block_signals(request(vec![
        joined("block:cedar", "Cedar Hill"),
        block("block:mill", "Mill curve"),
    ]));

    assert_eq!(preview.signals.len(), 2);
    assert_eq!(preview.signals[0].entrance_block_id, "block:cedar");
}

#[test]
fn both_directions_add_a_reverse_chain_entering_each_signal_block_from_its_far_end() {
    let mut both = request(vec![
        block("block:cedar", "Cedar Hill"),
        with_turnout(block("block:mill", "Mill curve"), "turnout:mill-1", "Mill siding 1"),
        with_turnout(joined("block:river", "River bridge"), "turnout:mill-2", "Mill siding 2"),
    ]);
    both.movements = Movements::BothDirections;
    let preview = preview_block_signals(both);

    assert_eq!(
        signal_ids(&preview),
        [
            "signal:abs:westbound-main:block:cedar:eastward",
            "signal:abs:westbound-main:block:mill:eastward",
            "signal:abs:westbound-main:block:river:westward",
            "signal:abs:westbound-main:block:cedar:westward",
        ]
    );
    let back_first = &preview.signals[2];
    assert_eq!(back_first.run, SignalRun::AgainstList);
    assert_eq!(back_first.governs, TravelDirection::Westward);
    assert_eq!(back_first.name, "River bridge West");
    assert_eq!(back_first.protected_block_ids, ["block:river", "block:mill"]);
    assert_eq!(back_first.turnout_ids, ["turnout:mill-2", "turnout:mill-1"]);
    assert_eq!(
        back_first.next,
        NextSignal::Signal { signal_id: "signal:abs:westbound-main:block:cedar:westward".to_string() }
    );
    assert_eq!(
        preview.signals[3].next,
        NextSignal::NothingBeyond,
        "with no Plant before the first block, the reverse chain ends with nothing beyond"
    );
}

#[test]
fn a_plant_before_the_first_block_is_read_by_the_reverse_chain() {
    let mut both = request(three_blocks());
    both.movements = Movements::BothDirections;
    both.entrance_end = EntranceEnd::Plant { plant_id: "plant:cedar-jct".to_string() };
    let preview = preview_block_signals(both);

    assert_eq!(
        preview.signals.last().map(|signal| signal.next.clone()),
        Some(NextSignal::PlantEntrance { plant_id: "plant:cedar-jct".to_string() })
    );
    assert_eq!(
        preview.signals[2].next,
        NextSignal::NothingBeyond,
        "the forward chain is unaffected by what lies before the first block"
    );
}

#[test]
fn nothing_beyond_treated_as_stop_limits_the_last_signal_to_approach() {
    let mut staging = request(three_blocks());
    staging.exit_end = ExitEnd::TreatAsStop;
    let preview = preview_block_signals(staging);

    assert_eq!(preview.signals[2].next, NextSignal::TreatedAsStop);
    assert_eq!(
        rules(&preview, 2),
        vec![
            (RuleCondition::BlockOccupied, SignalIndication::Stop),
            (RuleCondition::ClearNothingBeyondTreatedAsStop, SignalIndication::Approach),
        ]
    );
}

#[test]
fn a_plant_beyond_the_last_block_is_its_next_signal() {
    let mut plant = request(three_blocks());
    plant.exit_end = ExitEnd::Plant { plant_id: "plant:rockville".to_string() };
    plant.settings.aspects = 4;
    let preview = preview_block_signals(plant);

    assert_eq!(
        preview.signals[2].next,
        NextSignal::PlantEntrance { plant_id: "plant:rockville".to_string() }
    );
    assert_eq!(
        rules(&preview, 2),
        next_at_stop_rules(),
        "Advance Approach needs one of our own signals to read beyond"
    );
}

#[test]
fn two_aspects_read_only_the_signals_own_block() {
    let mut two = request(three_blocks());
    two.settings.aspects = 2;
    let preview = preview_block_signals(two);

    assert!(preview.signals.iter().all(|signal| signal.next == NextSignal::NotUsed));
    assert_eq!(rules(&preview, 0), stop_clear_rules());
}

#[test]
fn four_aspects_add_advance_approach_where_the_next_signal_reads_beyond_it() {
    let mut four = request(three_blocks());
    four.settings.aspects = 4;
    let preview = preview_block_signals(four);

    assert_eq!(
        rules(&preview, 0),
        vec![
            (RuleCondition::BlockOccupied, SignalIndication::Stop),
            (RuleCondition::ClearNextAtStop, SignalIndication::Approach),
            (RuleCondition::ClearNextAtApproach, SignalIndication::AdvanceApproach),
            (RuleCondition::ClearNextPermissive, SignalIndication::Clear),
        ]
    );
    assert_eq!(
        rules(&preview, 1),
        next_at_stop_rules(),
        "the last signal shows only Stop / Clear, so the one before it never sees Approach"
    );
}

#[test]
fn restricting_policy_changes_the_turnout_rule_and_turnouts_report_no_position_source() {
    let mut restricting = request(vec![
        with_turnout(block("block:cedar", "Cedar Hill"), "turnout:depot", "Depot track"),
    ]);
    restricting.settings.turnout_not_lined = TurnoutNotLined::Restricting;
    let preview = preview_block_signals(restricting);

    assert_eq!(
        rules(&preview, 0)[1],
        (
            RuleCondition::TurnoutNotLined { turnout_id: "turnout:depot".to_string() },
            SignalIndication::Restricting
        )
    );
    assert!(preview.issues.contains(&ProposalIssue::TurnoutPositionUnavailable {
        turnout_id: "turnout:depot".to_string(),
    }));
}

#[test]
fn a_signal_override_replaces_the_facility_settings_for_that_signal_only() {
    let mut custom = request(three_blocks());
    custom.signal_overrides.push(SignalOverride {
        signal_id: "signal:abs:westbound-main:block:mill:eastward".to_string(),
        aspects: 2,
        approach_lighting: ApproachLighting::WhenApproached,
    });
    custom.signal_overrides.push(SignalOverride {
        signal_id: "signal:abs:westbound-main:block:gone:eastward".to_string(),
        aspects: 2,
        approach_lighting: ApproachLighting::AlwaysLit,
    });
    let preview = preview_block_signals(custom);

    assert!(!preview.signals[0].overridden);
    assert_eq!(preview.signals[0].settings.aspects, 3);
    assert!(preview.signals[1].overridden);
    assert_eq!(preview.signals[1].settings.aspects, 2);
    assert_eq!(preview.signals[1].settings.approach_lighting, ApproachLighting::WhenApproached);
    assert_eq!(preview.signals[1].next, NextSignal::NotUsed);
    assert_eq!(
        rules(&preview, 0),
        next_at_stop_rules(),
        "a two-aspect next signal still shows Stop, so the signal before it keeps Approach"
    );
}

#[test]
fn reports_unsupported_aspect_counts_and_previews_with_three() {
    let mut odd = request(three_blocks());
    odd.settings.aspects = 6;
    let preview = preview_block_signals(odd);

    assert!(preview.issues.contains(&ProposalIssue::UnsupportedAspectCount { signal_id: None, aspects: 6 }));
    assert_eq!(preview.signals[0].settings.aspects, 3);
}

#[test]
fn reports_a_block_listed_twice() {
    let preview = preview_block_signals(request(vec![
        block("block:cedar", "Cedar Hill"),
        block("block:cedar", "Cedar Hill"),
    ]));

    assert!(preview.issues.contains(&ProposalIssue::DuplicateBlock { block_id: "block:cedar".to_string() }));
}

#[test]
fn deserializes_a_minimal_request_with_defaults() {
    let request: BlockSignalsRequest = serde_json::from_value(json!({
        "facilityId": FACILITY,
        "blocks": [{"blockId": "block:west-yard", "name": "West Yard"}]
    }))
    .expect("a minimal request deserializes");

    assert_eq!(request.direction, TravelDirection::Eastward);
    assert_eq!(request.movements, Movements::OneDirection);
    assert_eq!(request.exit_end, ExitEnd::NothingBeyond);
    assert_eq!(request.settings.aspects, 3);
    assert_eq!(preview_block_signals(request).signals[0].signal_id, WEST_YARD_EAST);
}

#[test]
fn serializes_ends_and_rules_in_the_ipc_shape() {
    let mut staging = request(vec![block("block:west-yard", "West Yard")]);
    staging.exit_end = ExitEnd::TreatAsStop;
    let preview_json = serde_json::to_value(preview_block_signals(staging)).expect("serializes");

    assert_eq!(preview_json["exitEnd"], json!({"kind": "treatAsStop"}));
    assert_eq!(preview_json["signals"][0]["next"], json!({"kind": "treatedAsStop"}));
    assert_eq!(
        preview_json["signals"][0]["rules"][1],
        json!({"condition": {"kind": "clearNothingBeyondTreatedAsStop"}, "indication": "approach"})
    );
}

#[test]
fn marks_a_preview_incomplete_with_one_missing_binding_issue_until_hardware_is_bound() {
    let preview_json = serde_json::to_value(preview_block_signals(request(three_blocks())))
        .expect("a preview serializes for its API");
    assert_eq!(
        preview_json["capability"],
        json!({
            "kind": "incompletePreview",
            "stage": "unavailable",
            "save": "unavailable",
            "apply": "unavailable"
        }),
        "the preview cannot be staged, saved or applied"
    );
    assert_eq!(
        preview_json["issues"],
        json!([{
            "kind": "missingHardwareBindings",
            "requiredBindings": ["signal-head-output"]
        }]),
        "one actionable missing-binding issue, however many signals are unbound"
    );
}

// ---- S2 hardware projection -------------------------------------------------

fn two_driver_rg_style() -> SignalStyle {
    SignalStyle {
        style_id: "two-driver-rg-signal".to_string(),
        binding_kind: "signalLccRgOutputs".to_string(),
        appearances: SignalAppearanceSet {
            stop: vec![SignalDriver::Red],
            approach: vec![SignalDriver::Red, SignalDriver::Green],
            clear: vec![SignalDriver::Green],
        },
    }
}

fn rg_request(green_output: u32, red_output: u32) -> BlockSignalsRequest {
    BlockSignalsRequest {
        bindings: vec![SignalBindingSelection {
            signal_id: WEST_YARD_EAST.to_string(),
            role: ChannelRole::SignalIndication,
            style: "two-driver-rg-signal".to_string(),
            binding: ChannelBinding::SignalLccRgOutputs {
                node_key: "05010101FF000101".to_string(),
                green_output,
                red_output,
            },
        }],
        ..request(vec![block("block:west-yard", "West Yard")])
    }
}

fn complete_rg_facts(outputs: Vec<SignalOutput>) -> SignalHardwareFacts {
    SignalHardwareFacts {
        hosts: vec![SignalHardwareHost {
            node_key: "05010101FF000101".to_string(),
            node_name: "West Signal-LCC".to_string(),
            style: two_driver_rg_style(),
            evidence: SignalHardwareEvidence::Complete,
            evidence_detail: None,
            outputs,
        }],
    }
}

fn free_outputs() -> Vec<SignalOutput> {
    vec![
        SignalOutput { ordinal: 1, label: "H1-G".to_string(), owner: None },
        SignalOutput { ordinal: 2, label: "H1-R".to_string(), owner: None },
    ]
}

#[test]
fn projects_a_typed_rg_signal_binding_through_the_shared_hardware_style() {
    let preview = preview_block_signals_with_hardware(rg_request(1, 2), complete_rg_facts(free_outputs()));

    assert!(preview.issues.is_empty(), "a complete compatible host validates the typed choice");
    let display = preview.signals[0]
        .display
        .as_ref()
        .expect("the selected signal receives the profile-backed display projection");
    assert_eq!(display.role, ChannelRole::SignalIndication);
    assert_eq!(display.style, "two-driver-rg-signal");
    assert_eq!(
        display.appearances,
        vec![
            SignalAppearance { indication: SignalIndication::Stop, outputs: vec![SignalDriver::Red] },
            SignalAppearance {
                indication: SignalIndication::Approach,
                outputs: vec![SignalDriver::Red, SignalDriver::Green],
            },
            SignalAppearance { indication: SignalIndication::Clear, outputs: vec![SignalDriver::Green] },
        ],
        "the shared style, not independent lamp semantics, defines Stop/Approach/Clear appearances",
    );
    assert_eq!(preview.output_hosts.len(), 1);
    assert_eq!(preview.capability, bowties_core::signaling::ProposalCapability::IncompletePreview {
        stage: bowties_core::signaling::ProposalActionAvailability::Unavailable,
        save: bowties_core::signaling::ProposalActionAvailability::Unavailable,
        apply: bowties_core::signaling::ProposalActionAvailability::Unavailable,
    });
}

#[test]
fn rejects_duplicate_red_and_green_output_choices_before_any_claim_exists() {
    let preview = preview_block_signals_with_hardware(rg_request(1, 1), complete_rg_facts(free_outputs()));

    assert_eq!(
        preview.issues,
        vec![ProposalIssue::DuplicateSignalOutputs {
            signal_id: WEST_YARD_EAST.to_string(),
            node_key: "05010101FF000101".to_string(),
            output: 1,
        }],
    );
}

#[test]
fn rejects_an_output_that_is_not_in_the_authoritative_host_configuration() {
    let preview = preview_block_signals_with_hardware(
        rg_request(1, 99),
        complete_rg_facts(vec![
            SignalOutput { ordinal: 1, label: "H1-G".to_string(), owner: None },
            SignalOutput { ordinal: 2, label: "H1-R".to_string(), owner: None },
        ]),
    );

    assert_eq!(
        preview.issues,
        vec![ProposalIssue::InvalidSignalBinding {
            signal_id: WEST_YARD_EAST.to_string(),
            reason: "Output 99 is not available on West Signal-LCC.".to_string(),
        }],
    );
}

#[test]
fn rejects_a_style_that_the_selected_host_does_not_declare() {
    let mut facts = complete_rg_facts(vec![
        SignalOutput { ordinal: 1, label: "H1-G".to_string(), owner: None },
        SignalOutput { ordinal: 2, label: "H1-R".to_string(), owner: None },
    ]);
    facts.hosts[0].style.style_id = "single-led-direct-lamp".to_string();

    let preview = preview_block_signals_with_hardware(rg_request(1, 2), facts);

    assert_eq!(
        preview.issues,
        vec![ProposalIssue::UnsupportedSignalStyle {
            signal_id: WEST_YARD_EAST.to_string(),
            style: "two-driver-rg-signal".to_string(),
            node_key: "05010101FF000101".to_string(),
        }],
    );
}

#[test]
fn keeps_a_valid_editor_choice_but_reports_unavailable_hardware_evidence() {
    let mut facts = complete_rg_facts(vec![
        SignalOutput { ordinal: 1, label: "H1-G".to_string(), owner: None },
        SignalOutput { ordinal: 2, label: "H1-R".to_string(), owner: None },
    ]);
    facts.hosts[0].evidence = SignalHardwareEvidence::Unavailable;
    facts.hosts[0].evidence_detail = Some(
        "West Signal-LCC configuration evidence is unavailable in this layout.".to_string(),
    );

    let preview = preview_block_signals_with_hardware(rg_request(1, 2), facts);

    assert_eq!(
        preview.issues,
        vec![ProposalIssue::UnavailableSignalHardware {
            signal_id: WEST_YARD_EAST.to_string(),
            reason: "West Signal-LCC configuration evidence is unavailable in this layout."
                .to_string(),
        }],
        "offline or incomplete evidence must not turn a valid transient choice into success",
    );
}

#[test]
fn reports_insufficient_current_output_capacity_from_complete_configuration() {
    let preview = preview_block_signals_with_hardware(
        rg_request(1, 2),
        complete_rg_facts(vec![
            SignalOutput { ordinal: 1, label: "H1-G".to_string(), owner: None },
            SignalOutput {
                ordinal: 2,
                label: "H1-R".to_string(),
                owner: Some("Existing mast".to_string()),
            },
        ]),
    );

    assert_eq!(
        preview.issues,
        vec![ProposalIssue::InsufficientSignalOutputCapacity {
            signal_id: WEST_YARD_EAST.to_string(),
            node_key: "05010101FF000101".to_string(),
            required: 2,
            available: 1,
        }],
    );
}

#[test]
fn reports_the_current_owner_of_a_selected_conflicting_output() {
    let preview = preview_block_signals_with_hardware(
        rg_request(1, 2),
        complete_rg_facts(vec![
            SignalOutput { ordinal: 1, label: "H1-G".to_string(), owner: None },
            SignalOutput {
                ordinal: 2,
                label: "H1-R".to_string(),
                owner: Some("Existing mast".to_string()),
            },
            SignalOutput { ordinal: 3, label: "H1-Y".to_string(), owner: None },
        ]),
    );

    assert_eq!(
        preview.issues,
        vec![ProposalIssue::SignalOutputConflict {
            signal_id: WEST_YARD_EAST.to_string(),
            node_key: "05010101FF000101".to_string(),
            output: 2,
            owner: "Existing mast".to_string(),
        }],
    );
}
