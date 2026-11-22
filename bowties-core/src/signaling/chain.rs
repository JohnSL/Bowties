//! Pure derivation of the signals for an Automatic Block Signals facility.
//!
//! The block list is in travel order for the selected direction. Blocks are
//! grouped into signal blocks (a block joined to the one before it shares its
//! signals), and each signaled direction forms one chain of entrance signals.
//! A signal protects every block and turnout in its signal block and, with
//! three or more aspects, reads the next signal in its chain or whatever lies
//! beyond the last block.

use std::collections::HashSet;

use super::{
    ApproachLighting, BlockSignalsRequest, EntranceEnd, ExitEnd, Movements, NextSignal,
    Observation, ProposalIssue, RuleCondition, SignalAspectRule, SignalIndication,
    SignalPreview, SignalRun, SignalSettings, TravelDirection, TurnoutNotLined,
};

const DEFAULT_ASPECTS: u8 = 3;

/// One direction's chain before rules are derived.
struct ChainSignal {
    signal_id: String,
    name: String,
    entrance_block_id: String,
    protected_block_ids: Vec<String>,
    turnout_ids: Vec<String>,
    settings: SignalSettings,
    overridden: bool,
}

pub(super) fn derive_signals(
    request: &BlockSignalsRequest,
    issues: &mut Vec<ProposalIssue>,
) -> Vec<SignalPreview> {
    report_request_issues(request, issues);

    let groups = signal_blocks(request);
    let mut claimed_saved = HashSet::new();
    let line_settings = SignalSettings {
        aspects: checked_aspects(request.settings.aspects, None, issues),
        approach_lighting: request.settings.approach_lighting.clone(),
    };

    let mut signals = chain(
        request,
        &groups,
        SignalRun::WithList,
        &line_settings,
        &request.exit_end,
        &mut claimed_saved,
        issues,
    );
    if request.movements == Movements::BothDirections {
        let reversed = groups
            .iter()
            .rev()
            .map(|group| group.iter().rev().copied().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let beyond = match &request.entrance_end {
            EntranceEnd::NoPlant => ExitEnd::NothingBeyond,
            EntranceEnd::Plant { plant_id } => ExitEnd::Plant {
                plant_id: plant_id.clone(),
            },
        };
        signals.extend(chain(
            request,
            &reversed,
            SignalRun::AgainstList,
            &line_settings,
            &beyond,
            &mut claimed_saved,
            issues,
        ));
    }
    signals
}

/// The saved signal ID for a signal block, when a not-yet-claimed saved signal
/// protects one of its blocks. Each saved signal is claimed at most once.
fn saved_signal_id(
    request: &BlockSignalsRequest,
    group: &[usize],
    claimed_saved: &mut HashSet<usize>,
) -> Option<String> {
    let (index, saved) = request.saved_signals.iter().enumerate().find(|(index, saved)| {
        !claimed_saved.contains(index)
            && group.iter().any(|&b| request.blocks[b].block_id == saved.protected_block)
    })?;
    claimed_saved.insert(index);
    Some(saved.signal_id.clone())
}

/// Signal blocks as lists of block indexes, in list order. The first block
/// always starts a signal block, whatever its joined flag says.
fn signal_blocks(request: &BlockSignalsRequest) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, block) in request.blocks.iter().enumerate() {
        match groups.last_mut() {
            Some(group) if block.joined_to_previous => group.push(index),
            _ => groups.push(vec![index]),
        }
    }
    groups
}

fn chain(
    request: &BlockSignalsRequest,
    groups: &[Vec<usize>],
    run: SignalRun,
    line_settings: &SignalSettings,
    beyond: &ExitEnd,
    claimed_saved: &mut HashSet<usize>,
    issues: &mut Vec<ProposalIssue>,
) -> Vec<SignalPreview> {
    let governs = match run {
        SignalRun::WithList => request.direction.clone(),
        SignalRun::AgainstList => request.direction.opposite(),
    };
    let skeletons = groups
        .iter()
        .map(|group| {
            let entrance = &request.blocks[group[0]];
            let derived_id = format!(
                "signal:{}:{}:{}",
                request.facility_id,
                entrance.block_id,
                governs.id_suffix()
            );
            let signal_id = saved_signal_id(request, group, claimed_saved).unwrap_or(derived_id);
            let name = request
                .signal_names
                .get(&signal_id)
                .cloned()
                .unwrap_or_else(|| format!("{} {}", entrance.name, governs.name_suffix()));
            let turnout_ids = group
                .iter()
                .flat_map(|&index| {
                    let turnouts = request.blocks[index].turnouts.iter().map(|t| t.turnout_id.clone());
                    match run {
                        SignalRun::WithList => turnouts.collect::<Vec<_>>(),
                        SignalRun::AgainstList => turnouts.rev().collect::<Vec<_>>(),
                    }
                })
                .collect();
            let override_settings = request
                .signal_overrides
                .iter()
                .find(|candidate| candidate.signal_id == signal_id);
            let (settings, overridden) = match override_settings {
                Some(custom) => (
                    SignalSettings {
                        aspects: checked_aspects(custom.aspects, Some(&signal_id), issues),
                        approach_lighting: custom.approach_lighting.clone(),
                    },
                    true,
                ),
                None => (line_settings.clone(), false),
            };
            ChainSignal {
                signal_id,
                name,
                entrance_block_id: entrance.block_id.clone(),
                protected_block_ids: group
                    .iter()
                    .map(|&index| request.blocks[index].block_id.clone())
                    .collect(),
                turnout_ids,
                settings,
                overridden,
            }
        })
        .collect::<Vec<_>>();

    let last = skeletons.len().saturating_sub(1);
    let uses_next = |index: usize| {
        skeletons[index].settings.aspects >= 3
            && (index < last || *beyond != ExitEnd::NothingBeyond)
    };

    skeletons
        .iter()
        .enumerate()
        .map(|(index, signal)| {
            let next = if signal.settings.aspects < 3 {
                NextSignal::NotUsed
            } else if !uses_next(index) {
                NextSignal::NothingBeyond
            } else if index < last {
                NextSignal::Signal {
                    signal_id: skeletons[index + 1].signal_id.clone(),
                }
            } else {
                match beyond {
                    ExitEnd::TreatAsStop => NextSignal::TreatedAsStop,
                    ExitEnd::Plant { plant_id } => NextSignal::PlantEntrance {
                        plant_id: plant_id.clone(),
                    },
                    ExitEnd::NothingBeyond => NextSignal::NothingBeyond,
                }
            };
            let rules = rules_for(
                signal,
                &next,
                &request.settings.turnout_not_lined,
                index < last && uses_next(index + 1),
            );
            SignalPreview {
                signal_id: signal.signal_id.clone(),
                name: signal.name.clone(),
                governs: governs.clone(),
                run: run.clone(),
                entrance_block_id: signal.entrance_block_id.clone(),
                protected_block_ids: signal.protected_block_ids.clone(),
                turnout_ids: signal.turnout_ids.clone(),
                settings: signal.settings.clone(),
                overridden: signal.overridden,
                next,
                rules,
                observation: Observation::Unavailable,
                display: None,
            }
        })
        .collect()
}

/// The conditions and indications for one signal, most restrictive first.
fn rules_for(
    signal: &ChainSignal,
    next: &NextSignal,
    turnout_not_lined: &TurnoutNotLined,
    next_uses_its_next: bool,
) -> Vec<SignalAspectRule> {
    let mut rules = vec![SignalAspectRule {
        condition: RuleCondition::BlockOccupied,
        indication: SignalIndication::Stop,
    }];
    let turnout_indication = match turnout_not_lined {
        TurnoutNotLined::Stop => SignalIndication::Stop,
        TurnoutNotLined::Restricting => SignalIndication::Restricting,
    };
    for turnout_id in &signal.turnout_ids {
        rules.push(SignalAspectRule {
            condition: RuleCondition::TurnoutNotLined {
                turnout_id: turnout_id.clone(),
            },
            indication: turnout_indication.clone(),
        });
    }
    let rule = |condition, indication| SignalAspectRule { condition, indication };
    match next {
        NextSignal::NotUsed | NextSignal::NothingBeyond => {
            rules.push(rule(RuleCondition::BlockClear, SignalIndication::Clear));
        }
        NextSignal::TreatedAsStop => {
            rules.push(rule(
                RuleCondition::ClearNothingBeyondTreatedAsStop,
                SignalIndication::Approach,
            ));
        }
        NextSignal::Signal { .. } | NextSignal::PlantEntrance { .. } => {
            rules.push(rule(RuleCondition::ClearNextAtStop, SignalIndication::Approach));
            let reads_a_signal_of_ours = matches!(next, NextSignal::Signal { .. });
            if signal.settings.aspects >= 4 && reads_a_signal_of_ours && next_uses_its_next {
                rules.push(rule(
                    RuleCondition::ClearNextAtApproach,
                    SignalIndication::AdvanceApproach,
                ));
            }
            rules.push(rule(RuleCondition::ClearNextPermissive, SignalIndication::Clear));
        }
    }
    rules
}

fn checked_aspects(aspects: u8, signal_id: Option<&str>, issues: &mut Vec<ProposalIssue>) -> u8 {
    if (2..=4).contains(&aspects) {
        return aspects;
    }
    issues.push(ProposalIssue::UnsupportedAspectCount {
        signal_id: signal_id.map(str::to_string),
        aspects,
    });
    DEFAULT_ASPECTS
}

fn report_request_issues(request: &BlockSignalsRequest, issues: &mut Vec<ProposalIssue>) {
    let mut seen = HashSet::new();
    for block in &request.blocks {
        if !seen.insert(block.block_id.as_str()) {
            issues.push(ProposalIssue::DuplicateBlock {
                block_id: block.block_id.clone(),
            });
        }
        for turnout in &block.turnouts {
            issues.push(ProposalIssue::TurnoutPositionUnavailable {
                turnout_id: turnout.turnout_id.clone(),
            });
        }
    }
}

impl TravelDirection {
    fn opposite(&self) -> Self {
        match self {
            Self::Eastward => Self::Westward,
            Self::Westward => Self::Eastward,
        }
    }

    fn id_suffix(&self) -> &'static str {
        match self {
            Self::Eastward => "eastward",
            Self::Westward => "westward",
        }
    }

    fn name_suffix(&self) -> &'static str {
        match self {
            Self::Eastward => "East",
            Self::Westward => "West",
        }
    }
}

impl Default for ApproachLighting {
    fn default() -> Self {
        Self::AlwaysLit
    }
}
