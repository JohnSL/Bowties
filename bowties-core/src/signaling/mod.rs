//! Pure, preview-only Automatic Block Signals policy.
//!
//! A request lists detected blocks in travel order, with hand-thrown turnouts
//! in the blocks that contain them. [`chain`] derives one entrance signal per
//! signal block and signaled direction, with the rules that decide its
//! indication. S2 adds a read-only role/style/binding projection for the
//! supported two-driver Signal-LCC display. Everything here is transient:
//! this module creates no channel, claim, draft, patch, persistence record, or
//! bus operation.

mod chain;
pub mod compiler;
pub mod facts;
pub mod logic_placement;
pub mod outputs;
pub mod slots;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use crate::layout::channels::{ChannelBinding, ChannelRole};

/// A detected block in the facility's list, in travel order.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SelectedBlock {
    pub block_id: String,
    pub name: String,
    /// True when there are no signals between this block and the one before
    /// it, so both share one signal block. Ignored on the first block.
    #[serde(default)]
    pub joined_to_previous: bool,
    /// Hand-thrown turnouts whose points are in this block.
    #[serde(default)]
    pub turnouts: Vec<PlaceholderTurnout>,
}

/// A hand-thrown turnout entered by name. Its position source is not chosen
/// yet, so its position is always unknown.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaceholderTurnout {
    pub turnout_id: String,
    pub name: String,
}

/// A geographic direction of travel.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TravelDirection {
    #[default]
    Eastward,
    Westward,
}

/// Which movements the facility's signals govern.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Movements {
    #[default]
    OneDirection,
    BothDirections,
}

/// What lies before the first block in the list.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EntranceEnd {
    #[default]
    NoPlant,
    Plant { plant_id: String },
}

/// What lies beyond the last block in the list.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ExitEnd {
    /// The last signal shows Stop / Clear.
    #[default]
    NothingBeyond,
    /// For staging or the end of signaled track: the last signal shows
    /// Approach at best.
    TreatAsStop,
    /// The last signal reads the Interlocking Plant's entrance signal.
    Plant { plant_id: String },
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TurnoutNotLined {
    #[default]
    Stop,
    Restricting,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApproachLighting {
    AlwaysLit,
    WhenApproached,
}

/// Settings that apply to every signal without an override.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FacilitySignalSettings {
    #[serde(default = "default_aspects")]
    pub aspects: u8,
    #[serde(default)]
    pub turnout_not_lined: TurnoutNotLined,
    #[serde(default)]
    pub approach_lighting: ApproachLighting,
}

impl Default for FacilitySignalSettings {
    fn default() -> Self {
        Self {
            aspects: default_aspects(),
            turnout_not_lined: TurnoutNotLined::default(),
            approach_lighting: ApproachLighting::default(),
        }
    }
}

fn default_aspects() -> u8 {
    3
}

/// The settings one signal uses, from the facility or its own override.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalSettings {
    pub aspects: u8,
    pub approach_lighting: ApproachLighting,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalOverride {
    pub signal_id: String,
    pub aspects: u8,
    pub approach_lighting: ApproachLighting,
}

/// A named release of an untracked owner on one node. Never implied: an owner
/// is released only when the request lists it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseRequest {
    pub node_key: String,
    /// Serialized as "Mast N" / "Direct lamp N".
    pub owner: SignalOutputOwner,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSignalsRequest {
    pub facility_id: String,
    #[serde(default)]
    pub facility_name: String,
    /// The direction the block list is written in.
    #[serde(default)]
    pub direction: TravelDirection,
    #[serde(default)]
    pub movements: Movements,
    pub blocks: Vec<SelectedBlock>,
    #[serde(default)]
    pub entrance_end: EntranceEnd,
    #[serde(default)]
    pub exit_end: ExitEnd,
    #[serde(default)]
    pub settings: FacilitySignalSettings,
    /// Overrides for individual signals; unknown signal IDs are ignored.
    #[serde(default)]
    pub signal_overrides: Vec<SignalOverride>,
    /// Signal names by signal ID; signals without one get a default name.
    #[serde(default)]
    pub signal_names: BTreeMap<String, String>,
    /// Untracked owners (a mast or Direct Lamp row no facility claims) the
    /// modeler chose to release, so their pins read as free. Applied to the
    /// effective configuration before any fact is derived.
    #[serde(default)]
    pub release: Vec<ReleaseRequest>,
    /// Ephemeral user selections. These reuse the existing Information
    /// Channel role/style/binding vocabulary, but are never persisted as
    /// channels in S2.
    #[serde(default)]
    pub bindings: Vec<SignalBindingSelection>,
    /// Signals already saved for this facility, filled by the backend from the
    /// layout. A derived signal whose protected block matches one keeps its
    /// saved signal ID, so names and channels stay attached across edits.
    #[serde(default)]
    pub saved_signals: Vec<SavedSignalIdentity>,
}

/// The identity of one saved signal: its ID and the block it protects.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedSignalIdentity {
    pub signal_id: String,
    pub protected_block: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Observation {
    Unavailable,
}

/// Whether a signal governs movement in list order or against it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SignalRun {
    WithList,
    AgainstList,
}

/// What a signal reads to choose between its permissive indications.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NextSignal {
    /// Two aspects: the signal looks only at its own signal block.
    NotUsed,
    /// The last signal, with nothing beyond: Stop / Clear.
    NothingBeyond,
    /// The last signal, with nothing beyond treated as Stop.
    TreatedAsStop,
    Signal { signal_id: String },
    PlantEntrance { plant_id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RuleCondition {
    BlockOccupied,
    /// Not lined for the main, or its position is unknown.
    TurnoutNotLined { turnout_id: String },
    BlockClear,
    ClearNothingBeyondTreatedAsStop,
    ClearNextAtStop,
    ClearNextAtApproach,
    ClearNextPermissive,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalAspectRule {
    pub condition: RuleCondition,
    pub indication: SignalIndication,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalPreview {
    pub signal_id: String,
    pub name: String,
    /// The geographic direction of the movements this signal governs.
    pub governs: TravelDirection,
    pub run: SignalRun,
    pub entrance_block_id: String,
    /// The blocks in this signal's signal block, in its direction of travel.
    pub protected_block_ids: Vec<String>,
    pub turnout_ids: Vec<String>,
    pub settings: SignalSettings,
    pub overridden: bool,
    pub next: NextSignal,
    /// Conditions and resulting indications, most restrictive first.
    pub rules: Vec<SignalAspectRule>,
    /// No live observation is read; absence is intentional and explicit.
    pub observation: Observation,
    /// Display semantics are present only when authoritative hardware facts
    /// support the selected style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<SignalDisplay>,
}

/// One transient selection of the existing role/style/binding mechanism.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalBindingSelection {
    pub signal_id: String,
    pub role: ChannelRole,
    pub style: String,
    pub binding: ChannelBinding,
}

/// Driver names used by the profile-declared two-driver RG appearance map.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignalDriver {
    Red,
    Green,
}

/// The facility's fixed indication vocabulary. Keeping it typed prevents a
/// profile/adapter consumer from manufacturing another lamp-like meaning.
/// The two-driver RG style maps only Stop, Approach and Clear.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SignalIndication {
    Stop,
    Restricting,
    Approach,
    AdvanceApproach,
    Clear,
}

/// The profile's complete semantic appearance map for the supported style.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalAppearanceSet {
    pub stop: Vec<SignalDriver>,
    pub approach: Vec<SignalDriver>,
    pub clear: Vec<SignalDriver>,
}

/// One resolved indication/physical appearance pair sent to consumers.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalAppearance {
    pub indication: SignalIndication,
    pub outputs: Vec<SignalDriver>,
}

/// A profile-declared signal style made available by a compatible host.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalStyle {
    pub style_id: String,
    pub binding_kind: String,
    pub appearances: SignalAppearanceSet,
}

/// Explicit provenance state for a host's current output configuration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SignalHardwareEvidence {
    Complete,
    Unavailable,
    Invalid,
}

/// One physical output, including its current authoritative owner when known.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalOutput {
    pub ordinal: u32,
    pub label: String,
    pub owner: Option<SignalOutputOwner>,
    /// The Bowties facility that owns this output's mast, when one does.
    /// Absent for direct lamps and for masts configured outside Bowties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facility: Option<OutputFacility>,
}

/// A Bowties facility that owns a physical output through a claimed mast.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutputFacility {
    pub facility_id: String,
    pub name: String,
}

/// Who currently drives a physical output. Serialized as its display label
/// ("Mast 2", "Direct lamp 1") so the frontend can show "Taken by ...".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalOutputOwner {
    /// A Rule-to-Aspect mast (1-based).
    Mast(u32),
    /// A Direct Lamp Control row (1-based).
    DirectLamp(u32),
}

impl std::fmt::Display for SignalOutputOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mast(n) => write!(f, "Mast {n}"),
            Self::DirectLamp(n) => write!(f, "Direct lamp {n}"),
        }
    }
}

impl Serialize for SignalOutputOwner {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SignalOutputOwner {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let label = String::deserialize(deserializer)?;
        let number = |prefix: &str| label.strip_prefix(prefix)?.parse::<u32>().ok();
        number("Mast ")
            .map(Self::Mast)
            .or_else(|| number("Direct lamp ").map(Self::DirectLamp))
            .ok_or_else(|| serde::de::Error::custom(format!("unknown output owner '{label}'")))
    }
}

/// Read-only, profile-supported output host facts. The backend gathers these
/// from the effective configuration and profile evidence; core only projects
/// and validates them.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalHardwareHost {
    pub node_key: String,
    pub node_name: String,
    pub style: SignalStyle,
    pub evidence: SignalHardwareEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_detail: Option<String>,
    pub outputs: Vec<SignalOutput>,
}

/// Complete input for the pure hardware projection.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalHardwareFacts {
    #[serde(default)]
    pub hosts: Vec<SignalHardwareHost>,
}

/// The display portion of an otherwise preview-only signal.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignalDisplay {
    pub role: ChannelRole,
    pub style: String,
    pub binding: Option<ChannelBinding>,
    pub appearances: Vec<SignalAppearance>,
}

/// A fact that prevents an otherwise normalized preview from becoming a
/// complete hardware proposal.  S1 deliberately exposes the absence rather
/// than manufacturing a binding or an observation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ProposalIssue {
    /// The same detected block appears more than once in the list.
    DuplicateBlock {
        block_id: String,
    },
    /// Aspects must be 2, 3 or 4; the preview uses 3 instead. `signal_id`
    /// is set when the value came from that signal's override.
    UnsupportedAspectCount {
        signal_id: Option<String>,
        aspects: u8,
    },
    /// The turnout has no position source yet, so its signals treat it as
    /// not lined for the main.
    TurnoutPositionUnavailable {
        turnout_id: String,
    },
    MissingDetectionFacts {
        block_id: String,
        missing_states: Vec<String>,
    },
    /// S1 can describe signal behavior but has no output binding to make that
    /// behavior real. The stable requirement lets callers explain the next
    /// required step without inventing a binding shape from later slices.
    MissingHardwareBindings {
        required_bindings: Vec<String>,
    },
    InvalidSignalBinding {
        signal_id: String,
        reason: String,
    },
    DuplicateSignalOutputs {
        signal_id: String,
        node_key: String,
        output: u32,
    },
    SignalOutputConflict {
        signal_id: String,
        node_key: String,
        output: u32,
        owner: String,
    },
    /// Another signal in the same edit already drives this output of the board.
    SignalOutputHeldBySignal {
        signal_id: String,
        node_key: String,
        output: u32,
        holder_signal_id: String,
        holder_name: String,
    },
    InsufficientSignalOutputCapacity {
        signal_id: String,
        node_key: String,
        required: usize,
        available: usize,
    },
    UnsupportedSignalStyle {
        signal_id: String,
        style: String,
        node_key: String,
    },
    UnavailableSignalHardware {
        signal_id: String,
        reason: String,
    },
}

/// What actions the proposal currently permits.
///
/// S1 only exposes an incomplete preview: it intentionally cannot enter the
/// staging, save, or apply lifecycles until a later slice supplies hardware
/// bindings.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProposalCapability {
    IncompletePreview {
        stage: ProposalActionAvailability,
        save: ProposalActionAvailability,
        apply: ProposalActionAvailability,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProposalActionAvailability {
    Unavailable,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSignalsPreview {
    pub facility_name: String,
    pub direction: TravelDirection,
    pub movements: Movements,
    pub blocks: Vec<SelectedBlock>,
    pub entrance_end: EntranceEnd,
    pub exit_end: ExitEnd,
    pub signals: Vec<SignalPreview>,
    pub capability: ProposalCapability,
    #[serde(default)]
    pub output_hosts: Vec<SignalHardwareHost>,
    /// Every mast outside Bowties that drives a pin on an output host, with
    /// the Logic that sets its aspects (`matchedLogic`) and the part of that
    /// kept when it also sends other events (`keptLogic`). Filled by the
    /// backend from the effective configuration; empty from the pure policy.
    #[serde(default)]
    pub outside_owners: Vec<compiler::UntrackedOwnerRelease>,
    #[serde(default)]
    pub issues: Vec<ProposalIssue>,
}

/// Derive a preview without hardware facts.
///
/// The backend uses [`preview_block_signals_with_hardware`] so guided and headless
/// callers share the one hardware interpretation. This wrapper keeps the
/// earlier pure policy API useful for callers that intentionally have no
/// hardware evidence.
pub fn preview_block_signals(request: BlockSignalsRequest) -> BlockSignalsPreview {
    preview_block_signals_with_hardware(request, SignalHardwareFacts::default())
}

/// Derive a preview from supplied, read-only hardware facts without allocating
/// a resource, mutating configuration, or changing state.
pub fn preview_block_signals_with_hardware(
    request: BlockSignalsRequest,
    hardware: SignalHardwareFacts,
) -> BlockSignalsPreview {
    let mut issues = Vec::new();
    let mut signals = chain::derive_signals(&request, &mut issues);

    let mut unbound = false;
    for signal in &mut signals {
        let binding = request
            .bindings
            .iter()
            .find(|candidate| candidate.signal_id == signal.signal_id);
        match binding {
            Some(binding) => project_signal_binding(signal, binding, &hardware, &[], &mut issues),
            None => unbound = true,
        }
    }
    if unbound || signals.is_empty() {
        issues.push(missing_hardware_bindings_issue());
    }

    BlockSignalsPreview {
        facility_name: request.facility_name,
        direction: request.direction,
        movements: request.movements,
        blocks: request.blocks,
        entrance_end: request.entrance_end,
        exit_end: request.exit_end,
        signals,
        capability: incomplete_preview_capability(),
        output_hosts: hardware.hosts,
        outside_owners: Vec::new(),
        issues,
    }
}

fn project_signal_binding(
    signal: &mut SignalPreview,
    selection: &SignalBindingSelection,
    hardware: &SignalHardwareFacts,
    own_masts: &[u32],
    issues: &mut Vec<ProposalIssue>,
) {
    // Pins held by the facility's own masts are free to it.
    let held_by_other = |output: &SignalOutput| match output.owner {
        None => false,
        Some(SignalOutputOwner::Mast(n)) => !own_masts.contains(&n),
        Some(SignalOutputOwner::DirectLamp(_)) => true,
    };
    if selection.role != ChannelRole::SignalIndication {
        issues.push(ProposalIssue::InvalidSignalBinding {
            signal_id: signal.signal_id.clone(),
            reason: "An Automatic Block Signals binding must use the signal-indication role.".to_string(),
        });
        return;
    }

    let ChannelBinding::SignalLccRgOutputs {
        node_key,
        green_output,
        red_output,
    } = &selection.binding
    else {
        issues.push(ProposalIssue::InvalidSignalBinding {
            signal_id: signal.signal_id.clone(),
            reason: "The selected signal style requires one same-host red/green output binding."
                .to_string(),
        });
        return;
    };

    let Some(host) = hardware.hosts.iter().find(|host| host.node_key == *node_key) else {
        issues.push(ProposalIssue::UnavailableSignalHardware {
            signal_id: signal.signal_id.clone(),
            reason: format!("The selected board '{}' has no authoritative Signal-LCC evidence.", node_key),
        });
        return;
    };
    if host.evidence != SignalHardwareEvidence::Complete {
        issues.push(ProposalIssue::UnavailableSignalHardware {
            signal_id: signal.signal_id.clone(),
            reason: host
                .evidence_detail
                .clone()
                .unwrap_or_else(|| format!("{} has incomplete or invalid output configuration evidence.", host.node_name)),
        });
        return;
    }
    if host.style.style_id != selection.style
        || host.style.binding_kind != "signalLccRgOutputs"
    {
        issues.push(ProposalIssue::UnsupportedSignalStyle {
            signal_id: signal.signal_id.clone(),
            style: selection.style.clone(),
            node_key: node_key.clone(),
        });
        return;
    }

    signal.display = Some(SignalDisplay {
        role: selection.role.clone(),
        style: selection.style.clone(),
        binding: Some(selection.binding.clone()),
        appearances: appearances_for(&host.style.appearances),
    });

    if green_output == red_output {
        issues.push(ProposalIssue::DuplicateSignalOutputs {
            signal_id: signal.signal_id.clone(),
            node_key: node_key.clone(),
            output: *red_output,
        });
        return;
    }

    for output_ordinal in [*green_output, *red_output] {
        if host
            .outputs
            .iter()
            .all(|output| output.ordinal != output_ordinal)
        {
            issues.push(ProposalIssue::InvalidSignalBinding {
                signal_id: signal.signal_id.clone(),
                reason: format!(
                    "Output {} is not available on {}.",
                    output_ordinal, host.node_name
                ),
            });
            return;
        }
    }

    let available = host
        .outputs
        .iter()
        .filter(|output| !held_by_other(output))
        .count();
    if available < 2 {
        issues.push(ProposalIssue::InsufficientSignalOutputCapacity {
            signal_id: signal.signal_id.clone(),
            node_key: node_key.clone(),
            required: 2,
            available,
        });
        return;
    }

    for output_ordinal in [*green_output, *red_output] {
        let output = host
            .outputs
            .iter()
            .find(|output| output.ordinal == output_ordinal)
            .expect("validated output remains present");
        if let Some(owner) = output.owner.filter(|_| held_by_other(output)) {
            issues.push(ProposalIssue::SignalOutputConflict {
                signal_id: signal.signal_id.clone(),
                node_key: node_key.clone(),
                output: output_ordinal,
                owner: owner.to_string(),
            });
        }
    }
}

fn appearances_for(appearances: &SignalAppearanceSet) -> Vec<SignalAppearance> {
    vec![
        SignalAppearance {
            indication: SignalIndication::Stop,
            outputs: appearances.stop.clone(),
        },
        SignalAppearance {
            indication: SignalIndication::Approach,
            outputs: appearances.approach.clone(),
        },
        SignalAppearance {
            indication: SignalIndication::Clear,
            outputs: appearances.clear.clone(),
        },
    ]
}

fn incomplete_preview_capability() -> ProposalCapability {
    ProposalCapability::IncompletePreview {
        stage: ProposalActionAvailability::Unavailable,
        save: ProposalActionAvailability::Unavailable,
        apply: ProposalActionAvailability::Unavailable,
    }
}

fn missing_hardware_bindings_issue() -> ProposalIssue {
    ProposalIssue::MissingHardwareBindings {
        required_bindings: vec!["signal-head-output".to_string()],
    }
}
