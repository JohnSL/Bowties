//! Read-only Automatic Block Signals proposal adapter.

pub mod compose;

use std::collections::HashMap;

use bowties_core::{
    layout::channels::{ChannelBinding, ChannelRole, InformationChannel},
    layout::state::LayoutState,
    node_key::NodeKey,
    node_tree::NodeConfigTree,
    profile::StructureProfile,
    signaling::{
        compiler::{CompileIssue, NodeConfigEdit, UntrackedOwnerRelease},
        facts::{effective_trees, outside_mast_owners, overlay_releases, signal_hardware_facts},
        outputs::attribute_output_facilities,
        preview_block_signals_with_hardware,
        slots::claimed_slots_from_facilities,
        ProposalIssue, ReleaseRequest, SignalHardwareFacts,
        BlockSignalsPreview, BlockSignalsRequest, SavedSignalIdentity,
    },
};

use crate::state::AppState;

/// The immutable, authoritative state facts a Automatic Block Signals proposal reads.
///
/// Node configuration trees are keyed by [`NodeKey`] rather than the
/// user-provided binding string, so callers cannot accidentally select facts
/// through a prefix or display-name match.
#[derive(Debug, Clone, Default)]
pub struct ProposalFacts {
    channels: Vec<InformationChannel>,
    trees: HashMap<NodeKey, NodeConfigTree>,
    hardware: SignalHardwareFacts,
    /// Edits that release the request's named untracked owners; their effect
    /// is already applied to `trees` and `hardware`.
    release_edits: Vec<NodeConfigEdit>,
    release_issues: Vec<CompileIssue>,
    outside_owners: Vec<UntrackedOwnerRelease>,
}

impl ProposalFacts {
    pub fn new(
        channels: Vec<InformationChannel>,
        trees: HashMap<NodeKey, NodeConfigTree>,
    ) -> Self {
        Self::with_hardware(channels, trees, SignalHardwareFacts::default())
    }

    pub fn with_hardware(
        channels: Vec<InformationChannel>,
        trees: HashMap<NodeKey, NodeConfigTree>,
        hardware: SignalHardwareFacts,
    ) -> Self {
        Self {
            channels,
            trees,
            hardware,
            release_edits: Vec::new(),
            release_issues: Vec::new(),
            outside_owners: Vec::new(),
        }
    }
}

/// Build one progressive, side-effect-free proposal from authoritative facts.
///
/// This is deliberately the single entry point for guided IPC and headless
/// callers. It only reads the effective channel inventory and cached CDI
/// trees; it does not allocate drafts, claims, configuration writes, or bus
/// activity.
pub fn propose(
    facts: &ProposalFacts,
    mut request: BlockSignalsRequest,
) -> Result<BlockSignalsPreview, String> {
    let mut issues = Vec::new();
    for selected in &mut request.blocks {
        let channel = facts
            .channels
            .iter()
            .find(|channel| channel.id == selected.block_id)
            .ok_or_else(|| format!("Detected block '{}' is unavailable.", selected.block_id))?;
        if channel.role != ChannelRole::BlockOccupancy {
            return Err(format!("'{}' is not a detected block.", channel.name));
        }
        // The layout owns the block's name; the list keeps its order, joins and turnouts.
        selected.name = channel.name.clone();

        let event_ids = resolved_detection_events(facts, channel);
        let missing_states = ["occupied", "clear"]
            .into_iter()
            .filter(|state| !event_ids.contains_key(*state))
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !missing_states.is_empty() {
            issues.push(ProposalIssue::MissingDetectionFacts {
                block_id: channel.id.clone(),
                missing_states,
            });
        }
    }

    let mut preview = preview_block_signals_with_hardware(request, facts.hardware.clone());
    preview.issues.extend(issues);
    preview.outside_owners = facts.outside_owners.clone();
    Ok(preview)
}

/// The saved signals of an existing facility, so editing keeps their IDs.
/// Empty when the facility is new or has no Automatic Block Signals record.
pub(crate) fn saved_signals_from_layout(
    layout: &LayoutState,
    facility_id: &str,
) -> Vec<SavedSignalIdentity> {
    layout
        .effective_facilities()
        .facilities
        .iter()
        .find(|f| f.facility_id == facility_id)
        .and_then(|f| f.block_signals.as_ref())
        .map(|record| {
            record
                .signals
                .iter()
                .map(|s| SavedSignalIdentity {
                    signal_id: s.signal_id.clone(),
                    protected_block: s.protected_block.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolved_detection_events(
    facts: &ProposalFacts,
    channel: &InformationChannel,
) -> HashMap<String, String> {
    let ChannelBinding::ConnectorInput {
        node_key,
        connector,
        input,
    } = &channel.binding
    else {
        return HashMap::new();
    };
    let Ok(node_key) = NodeKey::parse(node_key) else {
        return HashMap::new();
    };
    let Some(tree) = facts.trees.get(&node_key) else {
        return HashMap::new();
    };
    let Some(mapping) = detection_event_mapping(&channel.style) else {
        return HashMap::new();
    };

    bowties_core::channel_events::resolve_channel_event_ids(tree, connector, *input, &mapping)
}

fn detection_event_mapping(style: &str) -> Option<HashMap<String, u32>> {
    match style {
        "bod-block-detector-input" => Some(HashMap::from([
            ("occupied".to_string(), 0),
            ("clear".to_string(), 1),
        ])),
        _ => None,
    }
}

/// Assemble proposal facts from the effective LayoutState. Signal hosts come
/// from every captured/saved configuration tree, not from detector-channel
/// bindings, so a compatible Signal-LCC is visible even when it hosts no
/// detector.
///
/// The request's named releases are applied here, once, before any fact is
/// derived: pin owners, slot facts and the free-slot pick all read the
/// released trees, so preview and compose cannot disagree.
fn proposal_facts_from_layout(
    layout: &bowties_core::layout::state::LayoutState,
    profiles: &HashMap<String, Option<StructureProfile>>,
    release: &[ReleaseRequest],
) -> ProposalFacts {
    let channels = layout.effective_channels().channels.clone();
    let claimed = claimed_slots_from_facilities(layout.effective_facilities(), None);
    let overlay = overlay_releases(effective_trees(layout), release, &claimed);
    let mut hardware = signal_hardware_facts(&overlay.trees, profiles);
    attribute_output_facilities(&mut hardware.hosts, layout.effective_facilities());
    let outside_owners = outside_mast_owners(&overlay.trees, &hardware.hosts, &claimed);
    ProposalFacts {
        release_edits: overlay.edits,
        release_issues: overlay.issues,
        outside_owners,
        ..ProposalFacts::with_hardware(channels, overlay.trees, hardware)
    }
}

/// Resolve requested block identities against the active effective layout,
/// then delegate terminal policy to the pure core module.  This path has no
/// draft, claim, configuration, or bus-write effect.
pub async fn preview(
    state: &AppState,
    mut request: BlockSignalsRequest,
) -> Result<BlockSignalsPreview, String> {
    let facts = {
        let layout = state.layout_state.read().await;
        let Some(layout) = layout.as_ref() else {
            request.saved_signals = Vec::new();
            return propose(&ProposalFacts::default(), request);
        };
        request.saved_signals = saved_signals_from_layout(layout, &request.facility_id);
        let profiles = state.profiles.read().await;
        proposal_facts_from_layout(layout, &profiles, &request.release)
    };
    if let Some(issue) = facts.release_issues.first() {
        return Err(issue.detail.clone());
    }

    propose(&facts, request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bowties_core::signaling::{SelectedBlock, SignalIndication};
    use bowties_core::layout::channels::{
        ChannelBinding, ChannelOwnership, ChannelsDocument, InformationChannel,
    };
    use bowties_core::layout::io::LayoutDirectoryReadData;
    use bowties_core::layout::manifest::LayoutManifest;
    use bowties_core::layout::state::{CapturedNode, LayoutState};
    use bowties_core::node_tree::{
        ConfigNode, ConfigValue, ConnectorProfile, ConnectorSlot, GroupNode, LeafNode,
        LeafType, NodeConfigTree, SegmentNode,
    };
    use bowties_core::profile::{make_profile_key, StructureProfile};
    use lcc_rs::cdi::{EventRole, Identification};
    use std::path::PathBuf;

    fn test_block(block_id: &str, name: &str) -> SelectedBlock {
        SelectedBlock {
            block_id: block_id.to_string(),
            name: name.to_string(),
            joined_to_previous: false,
            turnouts: vec![],
        }
    }

    fn test_request(blocks: Vec<SelectedBlock>) -> BlockSignalsRequest {
        BlockSignalsRequest {
            facility_id: "abs:westbound-main".to_string(),
            facility_name: "Westbound main".to_string(),
            blocks,
            ..Default::default()
        }
    }

    fn detected_block() -> InformationChannel {
        InformationChannel {
            id: "block-west-yard".to_string(),
            name: "West Yard".to_string(),
            role: ChannelRole::BlockOccupancy,
            style: "bod-block-detector-input".to_string(),
            ownership: ChannelOwnership::HardwareOwned,
            binding: ChannelBinding::ConnectorInput {
                node_key: "05010101FF01".to_string(),
                connector: "detector".to_string(),
                input: 1,
            },
        }
    }

    fn detected_block_tree() -> NodeConfigTree {
        let event = |name: &str, path: Vec<&str>, hex: &str| ConfigNode::Leaf(LeafNode {
            name: name.to_string(),
            description: None,
            element_type: LeafType::EventId,
            address: 0,
            size: 8,
            space: 253,
            path: path.into_iter().map(str::to_string).collect(),
            value: Some(ConfigValue::EventId {
                bytes: [0; 8],
                hex: hex.to_string(),
            }),
            event_role: Some(EventRole::Producer),
            constraints: None,
            button_text: None,
            dialog_text: None,
            action_value: 0,
            hint_slider: None,
            hint_radio: false,
            modified_value: None,
            write_state: None,
            write_error: None,
            read_only: false,
        });
        NodeConfigTree {
            node_id: "05.01.01.01.FF.01".to_string(),
            identity: None,
            connector_profile: Some(ConnectorProfile {
                node_id: "05.01.01.01.FF.01".to_string(),
                carrier_key: "test::detector".to_string(),
                slots: vec![ConnectorSlot {
                    slot_id: "detector".to_string(),
                    label: "Detector".to_string(),
                    order: 0,
                    allow_none_installed: true,
                    supported_daughterboard_ids: vec![],
                    affected_paths: vec![],
                    resolved_affected_paths: vec![vec![
                        "seg:0".to_string(),
                        "elem:0#1".to_string(),
                    ]],
                    base_behavior_when_empty: None,
                    supported_daughterboard_constraints: vec![],
                }],
                supported_daughterboards: vec![],
            }),
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: true,
            segments: vec![SegmentNode {
                name: "Inputs".to_string(),
                description: None,
                origin: 0,
                space: 253,
                children: vec![ConfigNode::Group(GroupNode {
                    name: "Line".to_string(),
                    has_name: true,
                    description: None,
                    instance: 1,
                    instance_label: "Line 1".to_string(),
                    replication_of: "Line".to_string(),
                    replication_count: 1,
                    path: vec!["seg:0".to_string(), "elem:0#1".to_string()],
                    children: vec![
                        event(
                            "Occupied",
                            vec!["seg:0", "elem:0#1", "elem:0"],
                            "0501010101000001",
                        ),
                        event(
                            "Clear",
                            vec!["seg:0", "elem:0#1", "elem:1"],
                            "0501010101000002",
                        ),
                    ],
                    display_name: None,
                    hideable: false,
                    hidden_by_default: false,
                    read_only: false,
                })],
            }],
        }
    }

    fn layout_with_authoritative_detection(
        channels: Vec<InformationChannel>,
        tree: Option<NodeConfigTree>,
    ) -> LayoutState {
        let mut layout = LayoutState::from_loaded(
            PathBuf::from("signal-preview-test"),
            LayoutDirectoryReadData {
                manifest: LayoutManifest {
                    schema_version: 4,
                    layout_id: "signal-preview-test".to_string(),
                    captured_at: "2026-11-07T00:00:00Z".to_string(),
                    last_saved_at: "2026-11-07T00:00:00Z".to_string(),
                    active_mode: "offline_file".to_string(),
                    match_thresholds: Default::default(),
                    connections: vec![],
                },
                node_snapshots: vec![],
                bowties: Default::default(),
                offline_changes: vec![],
                recovery_occurred: false,
                channels: ChannelsDocument::new(channels),
                facilities: Default::default(),
                load_warnings: vec![],
            },
            Default::default(),
            Default::default(),
        );
        if let Some(tree) = tree {
            layout.record_captured(
                NodeKey::parse("05010101FF01").unwrap(),
                CapturedNode {
                    tree: Some(tree),
                    ..Default::default()
                },
            );
        }
        layout
    }

    fn int_leaf(name: &str, path: Vec<&str>, value: i64) -> ConfigNode {
        ConfigNode::Leaf(LeafNode {
            name: name.to_string(),
            description: None,
            element_type: LeafType::Int,
            address: 0,
            size: 1,
            space: 253,
            path: path.into_iter().map(str::to_string).collect(),
            value: Some(ConfigValue::Int { value }),
            event_role: None,
            constraints: None,
            button_text: None,
            dialog_text: None,
            action_value: 0,
            hint_slider: None,
            hint_radio: false,
            modified_value: None,
            write_state: None,
            write_error: None,
            read_only: false,
        })
    }

    fn group(name: &str, instance: u32, children: Vec<ConfigNode>) -> ConfigNode {
        ConfigNode::Group(GroupNode {
            name: name.to_string(),
            has_name: true,
            description: None,
            instance,
            instance_label: format!("{name} {instance}"),
            replication_of: name.to_string(),
            replication_count: 3,
            path: vec![format!("group:{name}#{instance}")],
            children,
            display_name: None,
            hideable: false,
            hidden_by_default: false,
            read_only: false,
        })
    }

    fn signal_lcc_tree() -> NodeConfigTree {
        NodeConfigTree {
            node_id: "05.01.01.01.FF.02".to_string(),
            identity: Some(Identification {
                manufacturer: Some("RR-CirKits, Inc.".to_string()),
                model: Some("Signal-LCC".to_string()),
                hardware_version: None,
                software_version: Some("C7c".to_string()),
            }),
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: vec![],
            profile_applied: true,
            segments: vec![
                SegmentNode {
                    name: "Direct Lamp Control".to_string(),
                    description: None,
                    origin: 0,
                    space: 253,
                    children: vec![
                        group(
                            "Lamp",
                            1,
                            vec![int_leaf("Lamp Selection", vec!["lamp:1", "selection"], 0)],
                        ),
                        group(
                            "Lamp",
                            2,
                            vec![int_leaf("Lamp Selection", vec!["lamp:2", "selection"], 0)],
                        ),
                        group(
                            "Lamp",
                            3,
                            vec![int_leaf("Lamp Selection", vec!["lamp:3", "selection"], 0)],
                        ),
                    ],
                },
                SegmentNode {
                    name: "Rule to Aspect".to_string(),
                    description: None,
                    origin: 0,
                    space: 253,
                    children: vec![group(
                        "Mast",
                        1,
                        vec![
                            int_leaf("Mast Processing", vec!["mast:1", "processing"], 1),
                            group(
                                "Rule",
                                1,
                                vec![group(
                                    "Appearance",
                                    1,
                                    vec![int_leaf(
                                        "Lamp Selection",
                                        vec!["mast:1", "rule:1", "appearance:1", "selection"],
                                        2,
                                    )],
                                )],
                            ),
                        ],
                    )],
                },
            ],
        }
    }

    #[test]
    fn guided_and_headless_partial_requests_share_normalized_preview_and_detection_issues() {
        let facts = ProposalFacts::new(vec![detected_block()], Default::default());
        let request = test_request(vec![test_block("block-west-yard", "stale client name")]);

        let guided = propose(&facts, request.clone()).expect("guided input is selectable");
        let headless = propose(&facts, request).expect("headless input is selectable");

        assert_eq!(
            serde_json::to_value(&guided).unwrap(),
            serde_json::to_value(&headless).unwrap(),
            "both adapters must expose the same core-normalized proposal"
        );
        assert_eq!(guided.blocks[0].name, "West Yard");
        assert_eq!(
            guided.issues,
            vec![
                ProposalIssue::MissingHardwareBindings {
                    required_bindings: vec!["signal-head-output".to_string()],
                },
                ProposalIssue::MissingDetectionFacts {
                    block_id: "block-west-yard".to_string(),
                    missing_states: vec!["occupied".to_string(), "clear".to_string()],
                },
            ]
        );

        let unavailable = propose(
            &facts,
            BlockSignalsRequest { facility_id: String::new(), blocks: vec![test_block("gone", "Gone")], ..Default::default() },
        );
        assert_eq!(unavailable.unwrap_err(), "Detected block 'gone' is unavailable.");

        let invalid = ProposalFacts::new(
            vec![InformationChannel {
                role: ChannelRole::LampIndicator,
                ..detected_block()
            }],
            Default::default(),
        );
        assert_eq!(
            propose(
                &invalid,
                BlockSignalsRequest { facility_id: String::new(), blocks: vec![test_block("block-west-yard", "West Yard")], ..Default::default() },
            )
            .unwrap_err(),
            "'West Yard' is not a detected block."
        );
    }

    #[tokio::test]
    async fn app_state_preview_matches_headless_proposal_for_valid_cdi_facts_without_mutation() {
        let channel = detected_block();
        let tree = detected_block_tree();
        let facts = ProposalFacts::new(
            vec![channel.clone()],
            HashMap::from([(NodeKey::parse("05010101FF01").unwrap(), tree.clone())]),
        );
        let state = AppState::new();
        *state.layout_state.write().await =
            Some(layout_with_authoritative_detection(vec![channel.clone()], Some(tree)));
        let request = test_request(vec![test_block("block-west-yard", "stale client name")]);

        let headless = propose(&facts, request.clone()).expect("headless input is selectable");
        let (channels_before, effective_channels_before) = {
            let layout = state.layout_state.read().await;
            let layout = layout.as_ref().expect("test layout is loaded");
            assert!(layout.offline_changes().is_empty());
            (
                layout.channels() as *const _,
                layout.effective_channels() as *const _,
            )
        };

        let previewed = preview(&state, request).await.expect("preview input is selectable");
        assert_eq!(
            serde_json::to_value(&previewed).unwrap(),
            serde_json::to_value(&headless).unwrap(),
            "the AppState adapter must expose the headless normalized proposal"
        );
        assert_eq!(
            previewed.issues,
            vec![ProposalIssue::MissingHardwareBindings {
                required_bindings: vec!["signal-head-output".to_string()],
            }],
            "resolved detection facts leave only the required signal capability issue"
        );

        let layout = state.layout_state.read().await;
        let layout = layout.as_ref().expect("test layout remains loaded");
        assert_eq!(layout.channels() as *const _, channels_before);
        assert_eq!(
            layout.effective_channels() as *const _,
            effective_channels_before
        );
        assert!(layout.offline_changes().is_empty());
    }

    #[tokio::test]
    async fn app_state_adapter_and_headless_proposal_share_profile_backed_rg_hosts_without_detector_channels() {
        let channel = detected_block();
        let mut layout = layout_with_authoritative_detection(
            vec![channel.clone()],
            Some(detected_block_tree()),
        );
        layout.record_captured(
            NodeKey::parse("05010101FF02").unwrap(),
            CapturedNode {
                tree: Some(signal_lcc_tree()),
                ..Default::default()
            },
        );
        let state = AppState::new();
        *state.layout_state.write().await = Some(layout);
        let profile: StructureProfile = serde_yaml_ng::from_str(include_str!(
            "../../profiles/RR-CirKits_Inc._Signal-LCC.profile.yaml"
        ))
        .expect("the shipped Signal-LCC profile parses");
        state.profiles.write().await.insert(
            make_profile_key("RR-CirKits, Inc.", "Signal-LCC"),
            Some(profile),
        );
        let request = BlockSignalsRequest {
            bindings: vec![bowties_core::signaling::SignalBindingSelection {
                signal_id: "signal:abs:westbound-main:block-west-yard:eastward".to_string(),
                role: ChannelRole::SignalIndication,
                style: "two-driver-rg-signal".to_string(),
                binding: ChannelBinding::SignalLccRgOutputs {
                    node_key: "05010101FF02".to_string(),
                    green_output: 1,
                    red_output: 3,
                },
            }],
            ..test_request(vec![test_block("block-west-yard", "stale client name")])
        };

        let facts = {
            let layout = state.layout_state.read().await;
            let profiles = state.profiles.read().await;
            proposal_facts_from_layout(
                layout.as_ref().expect("test layout is loaded"),
                &profiles,
                &[],
            )
        };
        let headless = propose(&facts, request.clone()).expect("headless proposal succeeds");
        let guided = preview(&state, request).await.expect("adapter proposal succeeds");

        assert_eq!(
            serde_json::to_value(&guided).unwrap(),
            serde_json::to_value(&headless).unwrap(),
            "the real AppState adapter and headless path share normalized profile/configuration interpretation",
        );
        assert!(guided.issues.is_empty());
        assert_eq!(guided.output_hosts.len(), 1);
        assert_eq!(guided.output_hosts[0].outputs.len(), 3);
        assert_eq!(
            guided.output_hosts[0].outputs[1].owner,
            Some(bowties_core::signaling::SignalOutputOwner::Mast(1)),
            "current active mast appearances must make their physical output unavailable",
        );
        assert_eq!(
            guided.signals[0].display.as_ref().unwrap().appearances[1].indication,
            SignalIndication::Approach,
        );
        let layout = state.layout_state.read().await;
        let layout = layout.as_ref().expect("test layout remains loaded");
        assert!(layout.offline_changes().is_empty());
        assert_eq!(layout.channels().channels.len(), 1);
    }

    #[test]
    fn guided_and_headless_multiple_block_requests_keep_list_order_joins_and_detection_issues() {
        let east_yard = InformationChannel {
            id: "block-east-yard".to_string(),
            name: "East Yard".to_string(),
            ..detected_block()
        };
        let facts = ProposalFacts::new(vec![detected_block(), east_yard], Default::default());
        let mut joined_east = test_block("block-east-yard", "stale");
        joined_east.joined_to_previous = true;
        joined_east.turnouts.push(bowties_core::signaling::PlaceholderTurnout {
            turnout_id: "turnout:east-lead".to_string(),
            name: "East lead".to_string(),
        });
        let request = test_request(vec![test_block("block-west-yard", "West Yard"), joined_east]);

        let guided = propose(&facts, request.clone()).expect("guided input is selectable");
        let headless = propose(&facts, request).expect("headless input is selectable");

        assert_eq!(guided.blocks[1].name, "East Yard", "the layout owns block names");
        assert!(guided.blocks[1].joined_to_previous, "normalization keeps the join");
        assert_eq!(guided.signals.len(), 1, "joined blocks share one signal");
        assert_eq!(guided.signals[0].protected_block_ids, ["block-west-yard", "block-east-yard"]);
        assert_eq!(guided.signals[0].turnout_ids, ["turnout:east-lead"]);
        assert_eq!(
            guided.issues,
            vec![
                ProposalIssue::TurnoutPositionUnavailable {
                    turnout_id: "turnout:east-lead".to_string(),
                },
                ProposalIssue::MissingHardwareBindings {
                    required_bindings: vec!["signal-head-output".to_string()],
                },
                ProposalIssue::MissingDetectionFacts {
                    block_id: "block-west-yard".to_string(),
                    missing_states: vec!["occupied".to_string(), "clear".to_string()],
                },
                ProposalIssue::MissingDetectionFacts {
                    block_id: "block-east-yard".to_string(),
                    missing_states: vec!["occupied".to_string(), "clear".to_string()],
                },
            ]
        );
        assert_eq!(
            serde_json::to_value(&guided).unwrap(),
            serde_json::to_value(&headless).unwrap(),
            "guided and headless callers share the core policy result"
        );
    }
}
