# One-Direction Automatic Block Signals — First E2E Slice Proposal

Status: **proposed input for Speckit**. This defines a deliberately narrow but
operational Automatic Block Signals, not the complete facility-first storyboard. The
storyboard remains the UX direction; unsupported choices must not appear
available in the first working flow.

The [active spec](spec.md) controls subsequent decisions, including the
downstream Approach mapping, orientation-only direction changes and input-loss
scope. The 2026-11-05 display clarification below retains RG without a
subjective yellow-quality gate.

## Outcome

A modeler creates a named Automatic Block Signals for one direction of travel by ordering
existing detected blocks. Bowties creates one signal at the entrance to each
block, connects adjacent signals when present, and configures the compatible
Signal-LCC nodes that host them. The modeler can review, apply, observe, edit,
and remove the Line using Bowties' normal draft and hardware workflow.

A Line may begin with one block and one signal: occupancy selects Stop or
Clear, and no downstream signal is presumed. The modeler can later add a
second block and signal beyond the first in the chosen travel direction,
retaining the original signal's identity and compatible bindings while
introducing a real downstream cascade.
Both eastward and westward travel are supported, but a Line governs only the
chosen direction. Neither choice implies protection for opposing trains.

## Supported Choices And Deliberate Limits

| Dimension | First slice | Later |
|---|---|---|
| Facility | Automatic Block Signals, with individual named signals belonging to it | Interlocking Plant, shared or cross-facility signal ownership |
| Blocks | One or more existing named blocks with usable detection, in travel order; one signal protects each block | Undetected/placeholder blocks and other topologies |
| Movements | One direction, eastward or westward; uncoordinated ABS | Both directions, APB, opposing-movement coordination |
| Signaling | Speed family, Essential vocabulary: Stop, Approach, Clear where downstream signaling applies; a single or terminal signal uses Stop and Clear | Route, Common, Rulebook, signal-type customization |
| Behavior | Internal signals use their own protected block and the next signal's published restriction; the terminal signal uses its own block only | External downstream signal or Plant entrance, per-signal behavior/policy overrides |
| Hardware | Signals may be placed on one or more compatible Signal-LCC nodes, subject to available resources on each; at least two nodes must work before the feature is complete | Tower LCC+Q fallback |
| Display | One supported two-driver red/green searchlight arrangement: red for Stop, red + green for Approach, green for Clear | Dedicated R/Y/G elements, other heads, flashing, display choices and per-signal display overrides |

The first UI offers the supported searchlight arrangement as the only display,
not a wiring-variant selector or an unsupported choice. **Searchlight is not
currently an interactive option in the storyboard's Line Display step**; the
Speckit work must reconcile that step with this slice rather than claim its
current three-lamp/two-lamp examples are already the delivered display. The
physical red and green outputs still need to be assigned **for each signal**.
Per-signal names and output assignments are required bindings, not policy or
display overrides.

The red + green appearance is supported by the recorded Signal-LCC bench
configuration, but its visible yellow depends on the connected searchlight.
Rule-to-Aspect can select multiple lamp outputs for one rule. Bowties validates
technical hardware/output compatibility and correct red/red-plus-green/green
activation, not whether the blended color is acceptable. The modeler chooses
that color-quality/resource tradeoff. No dedicated yellow driver or flashing
substitute is allocated in this slice.

## Working Behavior

The signal at each block entrance protects that block. For every signal except
the last, the next signal follows from the Line's ordered blocks and the chosen
travel direction. In a one-block Line that signal is the terminal signal:
there is no inferred downstream signal or Track Circuit.

| Protected block | Next signal's restriction | Internal signal | Terminal signal |
|---|---|---|---|
| Occupied | Any | Stop | Stop |
| Clear | Restrictive (Stop) | Approach | Clear |
| Clear | Permissive (Clear) | Clear | Clear |

For Lines with three or more blocks, an internal downstream Approach must propagate the
appropriate restriction upstream according to a validated **speed** rule; it
must not be treated as Clear merely because it is not Stop. Speckit must define
and test the actual Track Speed comparison/mapping for Stop, Approach, and
Clear, including three or more blocks. The terminal Clear means only that its
own protected block is clear; it does not assert anything beyond the Line.

Signal-LCC Rule-to-Aspect owns each mast's mutually exclusive appearances and
published Track Speed. Conditional logic selects the local aspect from
occupancy and, for internal signals, a Track Circuit carrying the next mast's
speed. The receiving circuit belongs to the **upstream signal's node** even
when the next mast is on another node. Line order determines the relationship;
node placement must not change its semantics. The modeler does not configure
those tables, event IDs, or receiver numbers.

Unknown or unavailable occupancy/downstream information is **not Clear**.
Speckit must specify and verify a conservative runtime response for missing,
stale, or uninitialized inputs, including startup and disconnection; no
permissive aspect may be asserted from absence of information. If the supported
Signal-LCC configuration cannot guarantee the chosen response, the feature
must not be represented as operationally complete. Offline observations must
be labeled unavailable, not shown as current. This proposal does not claim
that a UI warning alone enforces a fail-safe hardware state.

## Guided End-To-End Flow

Follow the storyboard's Blocks → Signals → Display → Bindings → Confirm
structure, with unsupported controls absent or clearly unavailable:

1. **Blocks:** name the Line, choose one travel direction, select and order
   existing detected blocks. Show observed occupancy separately from the
   configuration preview; identify missing or unavailable detection.
2. **Signals:** show the signal protecting each block and allow its name to be
   edited. Explain the common Speed/Essential behavior, any derived internal
   links, and the terminal Clear/Stop boundary. Do not offer per-signal
   signaling overrides or an external endpoint connection.
3. **Display:** explain and preview the single supported red/green searchlight
   and the appearances actually used by each signal (Stop/Clear at the
   terminal signal, plus Approach for internal signals). Do not present an unimplemented color-light,
   dedicated-yellow, or per-signal display choice.
4. **Bindings:** select a signal and assign a compatible Signal-LCC node and
   its red and green outputs. Reuse the blocks' existing detection; do not ask
   the user to re-enter occupancy sources or wire internal next-signal
   references manually. Refuse duplicate or externally claimed outputs, and
   validate the capacity and configuration of each selected node.
5. **Confirm:** identify each signal's block, behavior, physical outputs, and
   the complete Line's expected operation; show capacity, conflicts, affected
   existing configuration, and proposed changes. Block application until
   required inputs, display compatibility, wiring, and logic are validated.

Saving a draft and applying it to hardware remain distinct actions. Cancel or
validation failure must not partially claim resources or alter committed
hardware. Errors during application must identify what applied and what did
not, including failures partway through configuring multiple nodes, and
provide a safe retry/recovery path rather than report success. The proposal
does not assume that changes to separate physical nodes can be atomic.

## Creation, Editing, And Removal

- Creation persists Line identity, ordered membership, direction, stable signal
  identities, protected-block references, per-signal node and output bindings,
  and ownership of generated resources on each node. Reopening the Line loads
  its actual configuration, not storyboard defaults.
- Editing the Line can change its name, direction, block membership/order,
  signal names, node placement, and output assignments. Recompute affected
  signal relationships and resource claims across nodes; preserve unaffected
  identities, names, and assignments.
  Appending a second block in travel order changes the original signal from
  terminal Stop/Clear to downstream-aware Stop/Approach/Clear without replacing
  it or discarding its red/green output bindings. Prepending instead leaves
  the original signal terminal and makes the new signal downstream-aware.
  Show what will change or be
  released before applying. Removing a block from a Line does not delete the
  shared block or its detector; removing the final block requires removing
  the Line rather than leaving an empty facility.
- Removing the Line removes only the Line's signals and the configuration and
  resources Bowties owns, after showing affected references; it must not delete
  shared blocks, detectors, or unrelated node configuration. Conflicts or
  unknown ownership block destructive application rather than silently
  overwriting hardware.
- Draft save, discard, readback, reconnect, and partial-application recovery
  use the existing Bowties lifecycle. Speckit must decide the persisted Line
  representation and the exact edit/removal ownership rules; this proposal
  does not prescribe a schema.

## Acceptance Evidence

Speckit should turn the following into testable requirements and scenarios:

1. A one-block Line on one compatible Signal-LCC is created through the guided
   UI, reviewed, applied through normal drafts, read back, and reopened.
   Occupying its block shows Stop; clearing it shows Clear. The same Line and
   signal can be edited in place and removed without orphaned owned resources.
2. Appending a second detected block to that existing Line creates its signal,
   preserves the original signal's identity, name, protected-block reference
   and compatible outputs, and reviews the new link and resource claims before
   applying. This two-mast progression may initially be proven on one node.
   With the upstream block clear, downstream Stop makes the upstream
   searchlight show Approach; downstream Clear makes it show Clear.
   Occupying either protected block makes that block's signal show Stop.
   Verify observed Rule-to-Aspect events, Track Speed/Track Circuit behavior,
   and **physical red/green appearance**, not just generated configuration.
3. Before this feature is complete, a three-block Line with three signals
   distributed across at least two Signal-LCC nodes is created, applied,
   read back, reopened, edited, and removed. The next-mast restriction crosses
   a node boundary via a Track Circuit on the receiving signal's node, and
   propagates correctly through the middle signal. Both travel directions
   derive the correct next signal from block order without reversing
   protected-block ownership. Verify actual events, Track Speeds, and physical
   appearances on both nodes, not only a same-node or simulated cascade.
4. A one-block Line and the terminal signal of a longer Line remain valid
   occupancy-only Clear/Stop configurations; terminal Clear is not presented
   as authority beyond the final block.
5. Output conflicts, insufficient masts/conditionals/Track Circuits/drivers,
   incompatible hardware, missing detection, malformed existing CDI groups,
   and unsupported physical displays prevent application with actionable
   per-node diagnostics and without silent repair. Cross-node links cannot be
   applied if the downstream mast's published address cannot be resolved.
6. Startup, stale/unavailable inputs, disconnection, cancellation, application
   failure, and retry do not silently turn unknown state into a permissive
   aspect or misrepresent stale observations as live.
7. Editing block order or membership, direction, or output bindings preserves
   unaffected signals and shared blocks; the review identifies affected
   downstream references and resource releases on every affected node before
   application. Removing a block from a multi-block Line can leave a valid
   one-block Line.
8. A headless scenario and the guided flow produce the same validated plan
   for equivalent Line/mast intent; neither bypasses the normal draft/apply
   path. Deterministic plan tests complement hardware acceptance.

The one-block operating signal, the later cross-node hardware cascade,
conservative unknown-state behavior, and normal lifecycle must all be
demonstrated for this feature to be done; a working single-node two-mast
increment or configuration preview alone does not suffice.

## Prior Branch Work To Reuse Deliberately

The local `020-abs-signaling` branch at `0365d5c` is read-only implementation
evidence. The [reconciliation plan](../proposals/signaling-ux/option-a-mainline-extraction-plan.md)
records what has already landed and why its earlier Tower-LCC-first ABS
facility cannot be merged wholesale. To inspect a branch-only file without
switching branches, use `git show 020-abs-signaling:<path>`.

| Status | Branch evidence (path after `020-abs-signaling:`) | Use for this Line |
|---|---|---|
| Already on the current branch | `bowties-core/src/channel_events.rs` (source commit `bf697f7`, landed as `7b6d9d7`) | Reuse effective drafted Event ID resolution; do not rebuild it for signaling. The existing `resolve_channel_event_ids_returns_drafted_modified_value` test protects this contract. |
| Already on the current branch | Generic `SlotCard`, CDI-backed navigation, single-selection UI, Signal-LCC profile corrections and hardware evidence (Units 1–7 of the reconciliation plan) | Build on the supported shared UI/configuration owners; do not copy the ABS-specific facility card or target picker. |
| Adapt only after mast binding ownership is decided | `bowties-core/src/channel_events.rs`: `resolve_lamp_row_range_event_ids`, with two-row and out-of-range tests | Evidence for ordered multi-output event resolution. It reads **Direct Lamp Control** event leaves, not Rule-to-Aspect mast appearances; do not use its range/ordinal scheme as the Line's mast allocation contract without validating the new binding. |
| Adapt tests and mapping, not the old schema | `app/src/lib/utils/channelStyles.ts`, `app/src/lib/utils/channelStyles.test.ts`, `app/src/lib/layout/eligibleLampRows.test.ts` | The `2-led-bicolor-aspect` mapping and adjacent-row claim tests describe red/green On/Off events and two claimed rows. Re-derive physical output claims for mast-owned appearances and the supported searchlight; the profile does not make this old style a ready-to-apply mast configuration. |
| Reuse observation cases with corrections | `app/src/lib/utils/channelState.ts`, `app/src/lib/utils/channelState.test.ts`, `app/src/lib/orchestration/eventStateOrchestrator.ts` | Retain the Stop/Approach/Clear red/green observation cases and event-subscription pattern. The source also assumes unseen On/Off events mean *off*, deriving known aspects from partial observation; that assumption is **not** proof of safe unknown/startup behavior. Specify and test unknown/unavailable state under the new owner before reuse. |
| Reject as production architecture | `bowties-core/src/behavior_templates.rs`, `bowties-core/src/logic_adapter/`, `app/src-tauri/src/commands/logic_adapter.rs`, and ABS-specific facility UI/prediction tests | Do not port `ABS_3_ASPECT_SIGNAL`, `WiringPlan`, `LogicAllocation`, the Tower-LCC compiler/target selection, downstream-signal-specific referrers, or predicted-aspect display. Re-derive resource claims, cleanup, and references for a Line of Signal-LCC masts. |

Source commits `bae83b6`, `f474e55`, and `bf697f7` mix useful primitives
with rejected policy; do not cherry-pick them as units. The proposed
[headless intent pipeline](../proposals/signaling-ux/headless-intent-pipeline-plan.md) and its Unit 10
boundary in the reconciliation plan establish where mast intent, physical
bindings, validation, and draft/application belong. Restate focused source
tests against those contracts, then add the one-block, upgrade, and cross-node
acceptance scenarios above. Neither the old branch nor its tests establish
cross-node application or conservative runtime behavior for this feature.

## Speckit Decisions To Resolve

- How Line identity and its owned signals persist, and how existing facility
  and draft owners represent a multi-signal edit spanning nodes without
  claiming atomic hardware writes.
- The exact speed-to-aspect mapping, especially Approach propagation, and
  validated hardware behavior when inputs are missing or stale.
- Technical compatibility of Signal-LCC output bindings for the two-driver
  RG mapping, verified separately from the modeler's judgment of blended color.
- Capacity and conflict validation across existing mast appearances, direct
  lamps, conditionals, Track Circuits, and unrelated configuration on each
  affected node.
- How node-local Track Circuit receivers link to a next mast on another node,
  including address resolution, unavailable downstream nodes, and reference
  cleanup when a signal moves or is removed.
- The application/recovery sequence when an edit changes signals across
  several nodes, and the treatment of external references on removal.

These are specification and design decisions, not permission to substitute a
mock-only path for working signals. Reconcile this proposal with the older
Mast-first phasing text in [goals-and-constraints.md](../proposals/signaling-ux/goals-and-constraints.md)
and the single-Mast [block-signal-feature-proposal.md](../proposals/signaling-ux/block-signal-feature-proposal.md)
before promoting it to an active spec.

## Source Material

- [Facility-first storyboard](../proposals/signaling-ux/signaling-facilities-storyboard.html#line)
- [Signaling UX charter](../proposals/signaling-ux/goals-and-constraints.md)
- [Current Line decisions](../proposals/signaling-ux/design-decisions.md)
- [Headless intent pipeline plan](../proposals/signaling-ux/headless-intent-pipeline-plan.md)
- [Branch reconciliation plan](../proposals/signaling-ux/option-a-mainline-extraction-plan.md)
- [Rule to Aspect](../../product/hardware/signal-lcc/rule-to-aspect.md)
- [Track Circuits](../../product/hardware/signal-lcc/track-circuits.md)
- [Two-driver bench procedure](../../product/hardware/signal-lcc/clean-room-test-procedure.md)
