# Implementation Plan: One-Direction Automatic Block Signals

**Branch**: `020-block-signals-first-slice` | **Date**: 2026-11-05 | **Spec**: [spec.md](spec.md)
**Input**: [Feature specification](spec.md), with the 2026-11-05 planning clarification: retain two-driver RG support; perceived blended color is the modeler's choice, not a validation gate. Track Circuit and two-board behavior require manual tests.

## Summary

Build a durable, one-direction Automatic Block Signals over existing detected blocks. One stable signal protects each block in entered travel order. Eastward/westward affects diagram orientation only. Support the guided Blocks, Signals, Display, Bindings, Confirm flow and a headless caller through the same validated proposal service.

Extend the existing facilities document and layout draft lifecycle rather than inventing a second signaling persistence or apply system. Pure Line policy and hardware proposal compilation belong in `bowties-core`; the backend owns observed configuration, validation, staging and recovery; frontend components only render and emit intent.

The pre-design hardware discovery is deliberately small: the [single-board S4r/S3r probe](manual-track-circuit-test.md), then the [same probe across two boards](manual-two-board-test.md) and one receiver-restart check. Tables follow Bowties' Mast/Rule/Appearance and Logic/Variable/Action selectors. Use factory-reset Signal-LCC test slots and copy generated IDs/addresses between named fields; current-layout exports and literal IDs are not prerequisites. Existing Bt/B2 detector buttons drive a known two-input publisher group, without JMRI or a direct event-trigger feature. Keep the Tower-LCC configuration unchanged. Full occupancy, placement and application matrices remain later acceptance work. Internal-signal compilation/application remains blocked until measured behavior supports the specified Stop/Approach/Clear mapping. Documentary support is not hardware acceptance.

## Technical Context

**Language/Version**: Rust 2021 or later; TypeScript with strict checking; Svelte 5
**Primary Dependencies**: Existing bowties-core, lcc-rs, Tauri 2, SvelteKit 2, tokio, serde and YAML persistence; no new dependency planned
**Storage**: Existing layout directory, facilities.yaml, node configuration snapshots and offline changes; extend existing draft/save/recovery ownership
**Testing**: Rust unit/integration tests, Vitest store/orchestrator/component tests, headless parity scenarios, isolated Signal-LCC bench acceptance
**Target Platform**: Existing Windows/macOS/Linux desktop application; use existing TCP connection workflow for this feature's examples
**Project Type**: Desktop frontend plus Tauri backend and independent Rust domain/protocol libraries
**Performance Goals**: No new latency SLA; proposal evaluation performs no bus writes, and asynchronous read/apply work must not block rendering
**Constraints**: Existing detected blocks only; compatible Signal-LCC only; two-driver RG searchlight, with color blending chosen by the modeler; no opposing-movement protection; shared driver pool; immutable review baseline; non-atomic physical nodes
**Scale/Scope**: One or more ordered blocks, one entrance signal per block; acceptance at one, two and three blocks across at least two nodes, both orientations; bound proposals by actual per-node resource capacity

No unresolved product clarification remains. Track Circuit measurements are an explicit experimental prerequisite, not an assumed technical fact. Same-board Stop/Approach/Clear mapping and corrected cross-board Stop/Clear reception are now recorded in the manual tests. Remaining checks still gate acceptance. The initial cross-board failure was resolved by hardware readback and receiver binding correction, not by weakening the specified mapping.

## Constitution Check

*Planning gates evaluated before research and again after the design below.*

| Principle / gate | Before research | After design |
|---|---|---|
| I. Rust 2021+, typed errors, no production panic paths | Pass: retain existing Rust stack | Pass: pure domain service and typed backend validation/errors |
| II. Cargo-based environment | Pass: existing workspace | Pass: no new build system or dependency |
| III. TDD and integration coverage | Pass: required in implementation | Pass: owner-level red/green tests, parity, lifecycle and real-node matrix specified |
| IV. Protocol correctness | Pass: reuse existing protocol layer | Pass: no invented speed/event encoding; bench gate and JMRI-aligned checks for any new wire emission |
| V. UX first | Pass: five-step flow from spec | Pass: actionable node/signal validation, review, unavailable vs expected vs observed states |
| VI. TCP-only scope constraint | Pass: no new transport scope | Pass: this feature uses existing connections; adds no transport |
| VII. Event management | Pass: review before hardware writes | Pass: managed identities/link addresses, collision checks and readback; no manual event IDs in the guided flow |
| Layer/state ownership | Pass: existing core/backend/frontend boundaries | Pass: facilities/draft/sync integration; no component-owned workflows or parallel apply engine |

Hardware feasibility is **partially measured**, separately from constitution compliance. The single-board probe passed Stop -> Approach and Approach/Clear -> Clear; the cross-board probe passed Stop/Clear after correcting a stale receiver binding to the publisher's hardware-read address. Cross-board Approach, receiver restart, final trigger verification and sustained quiet-bus checks remain open. One-block terminal behavior may be developed and tested independently; these probe results do not establish full two-/three-block Line acceptance.

## Project Structure

### Documentation (this feature)

```text
specs\020-block-signals-first-slice\
  spec.md
  block-signals-first-slice-proposal.md
  plan.md
  research.md
  data-model.md
  manual-track-circuit-test.md
  manual-two-board-test.md
  quickstart.md
  contracts\
    tauri-ipc.md
```

`tasks.md` and `slices.md` are downstream outputs, not generated by this planning run.

### Source Code (existing owners and proposed additions)

```text
bowties-core\src\
  layout\types.rs, state.rs, io.rs, manifest.rs    existing persistence/delta owners
  signaling\                                    proposed pure Line policy/compiler
bowties-core\tests\                              policy, profile, persistence, parity
app\src-tauri\src\
  commands\layout_drafts.rs                      existing draft IPC
  commands\layout_capture.rs                     existing open/save/recovery IPC
  commands\sync_panel.rs                         existing hardware apply boundary
  signaling\                                    proposed observed-state/proposal adapter
app\src\
  lib\api\                                      typed proposal IPC, existing sync API
  lib\stores\                                   facility drafts and Line editor state
  lib\orchestration\                            Line staging; existing draft/sync lifecycle
  lib\components\                               guided steps, diagram and review
  routes\                                       existing screen composition
lcc-rs\                                         existing protocol/read/write owners
```

**Structure Decision**: Extend the existing facilities aggregate and its draft deltas. Do not introduce a general recipe framework, independent Line store on disk, global role registry, or second hardware apply path. Proposed source names are design placements, not assertions that those modules already exist.

## Phase 0: Research Decisions

See [research.md](research.md) for evidence and alternatives:

1. Existing facilities/layout state is the authoritative saved/draft owner.
2. Line policy is deterministic, independent of direction and node placement.
3. Signal-LCC compilation is a narrow, evidence-backed adapter, not a generic recipe engine.
4. Local receiving circuits belong to the upstream signal's host node, including same-node links.
5. Reuse draft mirroring and sync apply; strengthen their result/recovery contract where required.
6. Use typed Tauri/service contracts, not a fictitious REST server.
7. Resolve receiver semantics and cross-node behavior with manual tests; keep the two-driver RG mapping without subjective color approval.

## Phase 1: Design

### Policy and identity

- Persist stable Line and signal IDs. Retain a signal when its protected block remains a member, even after reorder, prepend, append or host rebind.
- Derive downstream references exclusively from the ordered membership list. Do not persist a second independently editable downstream graph.
- Terminal: occupied -> Stop, clear -> Clear. Internal: occupied -> Stop; clear plus next Stop -> Approach; clear plus next Approach/Clear -> Clear.
- Conditionals select rules; each rule carries the semantic Track Speed; Rule-to-Aspect maps that rule to red, red-plus-green or green. A blended-color preference cannot change the published restriction or block a technically valid RG binding.
- Treat missing input as unavailable preview/observation. Do not invent a firmware fail-safe or input-expiry behavior.
- Apply eastward/westward and naming changes as metadata unless the reviewed hardware projection genuinely changes. In particular, orientation is excluded from compilation.

### Compile and validate

The pure compiler takes an effective Line, resolved block detection, supported profile/CDI facts, a tested hardware recipe and existing node/resource snapshots. It returns a deterministic per-node patch and ownership diff, or actionable issues; it never writes hardware or mutates lasting claims.

Reuse profile binding helpers for replicated mast, conditional and circuit paths (ADR-0013). Resolve addresses and selector values from supported CDI/profile data; do not infer numeric values from dropdown order or allocate arbitrary Track Circuit events. Preserve retained signal resources where compatible.

Validation covers detection and input compatibility, technical output/wiring compatibility, valid node identity/profile/firmware evidence, mast/rule/conditional/circuit/shared-driver capacity, complete current configuration, duplicate claims, external uses and reference integrity. It does not judge blended yellow quality or require a modeler color-approval flag. Cross-node consumers resolve the next mast's freshly hardware-read, node-generated Link Address on the upstream host; an edited/saved publisher value is not proof of that identity. Apply and read back receiver bindings against that observed address rather than attempting to assign the publisher address. Removal/rebinding checks every affected node, including the old host and receivers.

No numeric resource formula is accepted until the tested recipe defines actual rows and paths. Compare required slots against current allocations, not just manufacturer's maximums. Uncertain ownership or references block destructive writes.

### Draft, apply and recovery

- Preview is side-effect-free. Accepting a proposal stages its facility delta and node patches together through existing draft ownership.
- Confirm is based on the exact reviewed proposal revision and node snapshots. Revalidate before staging/apply; stale review cannot authorize a different patch.
- Saving preserves proposed Line metadata and pending node changes; it is not hardware application. Discard/cancel cannot leave committed resource claims.
- Use the existing sync apply executor and config readback. Extend backend-owned operation results to preserve per-node and, on failure, per-patch evidence; a failed node may itself be partly written.
- Persist recovery evidence through the existing layout save/recovery machinery: reviewed patch, preimages, intended values, acknowledged writes and subsequent readback. Never silently mark intended configuration as observed.
- Retry by reading actual state and validating a new delta. Do not replay a stale patch or automatically roll back other nodes. Unavailable or diverged state blocks unsafe destructive recovery.
- Removal releases only proven Line-owned resources. Keep saved ownership/recovery information until release is verified or safely recorded as pending; never delete shared blocks or detectors.

See [data-model.md](data-model.md) and [IPC contracts](contracts/tauri-ipc.md).

### UI and headless integration

Routes compose the editor; components render step state and emit intent; stores own deterministic edits; an orchestrator sequences preview, staging, save/apply/readback and cancellation. Both UI and headless scenarios call the same domain proposal builder and backend lifecycle service; the latter is not a mock-only compiler shortcut.

Confirm shows expected aspects, protected blocks, real outputs, owned allocations/releases and every affected node. Observations carry availability and are never replaced by preview defaults. Partial apply is a persistent, visible recovery state.

#### Approved wizard UX contract

The [Automatic Block Signals storyboard](../proposals/signaling-ux/signaling-facilities-storyboard.html#line)
is the approved interaction and visual structure, specifically
[Blocks](../proposals/signaling-ux/signaling-facilities-storyboard.html#line-blocks)
and [Signals](../proposals/signaling-ux/signaling-facilities-storyboard.html#line-signals)
for S1. Implement a wide shared app dialog, five-step numbered stepper, contextual
guidance, structured selection/configuration cards, track diagram and
Cancel/Back/Next footer navigation using existing dialog/style primitives.
The existing Add facility entry offers Automatic Block Signals alongside unchanged Block
Indicator; it must not become an unrelated inline form.

Authorized S1 differences are functional, not a UX redesign: one block and one
direction only; Display, Bindings and Confirm are visibly unavailable; no
staging, saving or application; unavailable observations remain explicit.
Preserve the wizard framing and hierarchy when restricting those capabilities.
Any additional visual/interaction departure requires user approval.

The wizard keeps Cancel flush left and Back/Next flush right on both pages.
Block selection opens a searchable, scrollable selection dialog labelled
Add block..., not an inline list. Form labels and text boxes use the storyboard's
horizontal rows. Preserve user-facing ABS emphasis and the Signals track strip,
separate signal-selection tile, name row and policy/expectation cards; do not
insert storyboard narration or design notes into the product UI.

Caller acceptance must compare the running Blocks and Signals screens against
those storyboard sections, including selection, navigation, cancel/close,
orientation, app styling and narrow-screen usability. Record results and allowed
differences in `slices.md`; automated field/text assertions alone cannot establish
wizard completion.

For this Tauri workflow, visual acceptance must use the native app and user
screenshots. Do not use the integrated browser as a substitute. Preserve the
storyboard's compact track units, actual boundary gaps, SVG signal mast,
selected tiles with names below their icons, and policy form affordances.
Keep native visual review pending when native runtime evidence is unavailable.

Both Add facility types require an initial name. The existing trimmed name
passes through the Automatic Block Signals orchestrator into transient editor initialization
and remains editable there. Alphabetize block-picker candidates only, not the
Line's operational membership. Required hardware capabilities remain in the
backend proposal but are not displayed as routine notices on these steps where
no save or apply action is offered; actual fact and IPC errors remain visible.

The Line wizard uses the shared dialog's opt-in contained layout. A roomy
viewport keeps the upper context and footer fixed and lets policy/indication
panes scroll independently; a constrained viewport uses one inner workspace
scrollbar. Do not add a competing outer body scrollbar or hide unreachable
controls. Rail labels use explicit line height and symmetric gaps.

### Persistence compatibility

Extend the existing facilities schema/variant and layout deltas rather than duplicating Line records. Preserve older facilities and layouts without Lines. Follow existing manifest migration policy if a new required persisted shape is introduced; the current manifest schema is version 4, distinct from the legacy layout-file `"1.0"` constant. Unknown/malformed Line data must produce an explicit load/reconciliation error, not disappear. Backward-load and save/open fixtures are mandatory.

## Phase 2: Implementation Sequencing and Verification

This section orders deliverables; downstream `/design` and `/slices` determine worker batches and tasks.

1. **Pre-design discovery**: a factory-reset S4r/S3r single-board probe distinguishes downstream Stop from Approach/Clear; repeat with a publisher on the second board and one receiver-restart check. Copy actual generated values, do not require a current-layout export, survey all speed selectors or rebuild known detector logic. Record the working comparison/action mapping and initialization evidence; investigate failures before committing to hardware-dependent design. Full behavior matrices follow during implementation/acceptance; perceived yellow quality is not a gate.
2. **One-block vertical flow (US1)**: stable entities, terminal recipe, proposals, draft/save/apply/readback, five-step UI and headless parity, rename and guarded removal.
3. **Extension (US2)**: retained identity/resources, local receiving circuit, tested restriction mapping; occupied precedence and append transition.
4. **Cross-node maintenance (US3)**: three-block/two-node lifecycle, reorder/prepend/rebind/nonfinal removal, affected old/new host references; orientation-only zero-write regression.
5. **Failure paths throughout (US4)**: validation/cancellation, unavailable observations, interrupted writes, truthful readback, safe retry/recovery. Do not defer safe failure handling until after shipping the happy paths.
6. **Acceptance/documentation**: complete the matrix below and update product behavior, relevant aiwiki owners/flows/seams, and backlog only for actionable remaining work. Extend a relevant seam ADR rather than creating a generic architecture framework.

### Automated verification

| Owner | Required behavior contracts |
|---|---|
| Core policy/compiler | Full terminal/internal truth tables; all 8 three-block occupancy combinations; block-order/host/orientation invariants; stable identity and allocation; independent RG appearance/Track Speed mapping without subjective color gate; invalid evidence and resource/reference conflicts |
| Core layout | Old-layout load, facility deltas, draft discard, save/reopen, ownership/recovery roundtrip, unknown/malformed schema errors |
| Backend | Preview has zero writes; stale review blocked; staging is coherent; metadata save has zero writes; per-node/per-patch failure accounting; readback mismatch; recovery and guarded release |
| Frontend stores/orchestrators | Five steps, cancellation, pending vs committed claims, unavailable observations, partial failures, reconnect/readback, orientation-only edits |
| UI/headless parity | Equal normalized proposals and patches for identical input, including invalid and destructive changes; both use the real lifecycle adapter |
| Protocol, only if changed | Focused `lcc-rs` tests and equivalent JMRI behavior/reference citation for new transmit sites |

Existing nearby regression tests include `syncApplyOrchestrator.test.ts`, `configReadOrchestrator.test.ts`, `offlineChanges.store.test.ts`, `page.route.test.ts` and `bundled_signal_lcc_profile_contract.rs`; preserve their behavior.

During implementation, select focused tests first, then the affected aggregates and diagnostics according to [testing strategy](../../product/quality/testing-strategy.md):

```powershell
# From the verified Bowties root; selectors are chosen from actual changed test files.
cargo test -p bowties-core
cargo test -p app
Push-Location .\app
npm test
npm run check
Pop-Location
cargo check -p bowties-core -p app
cargo clippy -p bowties-core -p app -- -D warnings
```

No write-mode formatter is part of validation.

### Physical and usability acceptance

- Complete the small [single-board probe](manual-track-circuit-test.md) and [two-board extension](manual-two-board-test.md) before finalizing hardware-dependent design. The full acceptance cases below are not prerequisites for that initial experiment.
- One-block occupied/clear; two-block all 4 combinations; three-block all 8 combinations. Repeat both orientations and test same-node, cross-node, swapped-host and A-B-A placements. Require 100% matching rule/output combinations plus published/received restrictions; do not grade yellow hue.
- Apply, read back, reopen, edit and remove one-/three-block Lines. Preserve unaffected signals, shared detection and unrelated configuration.
- Record interrupted application after a successful first node and within a later node; never label it fully applied. Validate retry from actual readback.
- Disconnect/reconnect: UI freshness is truthful; record, but do not redefine, firmware input-loss behavior.
- Conduct the specified five-modeler review: at least 4/5 identify protected block, expected aspect and required wiring without raw configuration.

## Complexity Tracking

No unjustified constitution violation. Extensions to facilities, sync evidence and recovery are required by durable identity and non-atomic multi-node application; no parallel framework is proposed.

## Planning Outcome

Research/design artifacts are complete. Production code is unchanged. Implementation may proceed to design/slicing using the measured same-board mapping and cross-board Stop/Clear results. Hardware-dependent internal-signal acceptance remains gated by the outstanding checks in the manual records and the full physical acceptance matrix.

## Architecture Assessment

**Reviewed**: 2026-11-07. The user approved the architectural shape, findings and
six-slice roadmap presented by `/design`. This section is the approved input to
`/slices`; per-layer implementation tasks remain just-in-time `/build` work.

### Affected Modules

| Module | Layer | Impact | Notes |
|--------|-------|--------|-------|
| `bowties-core/src/layout/facilities.rs`, `types.rs`, `state.rs`, `io.rs`, `manifest.rs` | Core domain / persistence | Modified | Extend the facilities aggregate, deltas, effective view and compatible save/open lifecycle; no second Line document |
| Proposed `bowties-core/src/signaling/` | Core domain | New | Deterministic Line policy, stable signal retention, resource diff and narrow Signal-LCC compilation |
| Proposed `app/src-tauri/src/signaling/` and typed command adapters | Backend workflow / IPC | New | Resolve authoritative facts, validate immutable proposals, coordinate existing staging/apply/readback and recovery |
| `commands/layout_drafts.rs`, `layout_capture.rs` | Backend IPC / persistence | Modified | Reuse draft mirroring, save/open and recovery ownership |
| `commands/sync_panel.rs` | Backend apply workflow | Modified | Extend existing execution with proposal-linked per-patch outcomes and verification evidence |
| `app/src/lib/layout/` and `stores/facilities.svelte.ts` | Frontend read model / draft state | Modified | Line projections and facility drafts participate in existing aggregate dirty/save/discard behavior |
| Proposed Line editor state and orchestration | Frontend stores / orchestration | New | Deterministic editor state and lifecycle-scoped sequencing; no parallel persistent Line owner |
| `configDraftMirrorOrchestrator.svelte.ts`, `configDraftOrchestrator.ts` | Frontend orchestration | Modified | Consolidate overlapping flushing ownership and establish an awaited synchronization/error contract |
| `layoutLifecycleOrchestrator.ts`, existing save/sync orchestration | Frontend orchestration | Modified | Preserve reset, close, discard, save and post-apply reconciliation |
| Guided steps, diagram and review; Railroad composition | Frontend rendering / route | New / Modified | Render composed state and emit intent; do not own validation, resource allocation or async workflows |
| `channel_events.rs`, profile bindings and channel style helpers | Core / frontend translation | Touched | Reuse detection, canonical paths and binding semantics rather than duplicating event/style ownership |
| `lcc-rs` | Protocol | Touched | Reuse existing peer-session read/write interfaces; no new protocol abstraction planned |

### Assessment Summary

The design is a facilities extension, not a signaling framework. Its depth comes
from one proposal interface hiding policy, identity retention, hardware
compilation, ownership checks and resource diffs. Backend workflows supply facts
and perform effects; rendering modules do not learn hardware allocation rules.

Patterns:

- **Ordered aggregate**: membership is travel order and the only source of
  downstream relationships. Orientation is presentation metadata.
- **Functional core / imperative shell**: pure Line policy/compiler in
  `bowties-core`; observation, validation, effects and recovery in the backend.
- **Immutable proposal**: confirmation authorizes the exact reviewed revision
  and snapshot generations, not a client-reconstructed or silently changed patch.
- **Effective-view facade**: Line displays consume composed state, not competing
  local interpretations of saved values and drafts.
- **Forward recovery**: fresh readback produces a new reviewable correction;
  physical multi-node application is not assumed atomic or automatically rolled
  back.

The Signal-LCC compiler is a narrow implementation, not a generic recipe engine
or a speculative adapter framework. Existing journaled persistence, draft
ownership and sync execution remain the integration seams.

### Findings

**F1: Existing contracts and current code have drifted**
- Category: ADR / debt / locality
- Affected: layout facade, dirty/lifecycle contracts, draft interfaces and persistent-state seam documentation.
- Concern: ADR-0004's exclusive facade import/mutation rules are violated by existing route/rendering callers. ADR-0012's literal four-method interface does not describe every current editable store. ADR-0015's original `saved_trees` invariant conflicts with its later accepted cache deletion. Config Draft Backend Mirror entries name obsolete backend consumers.
- Decision: **include narrowly**. Correct contracts and directly touched Line surfaces; establish evidence for applicable Unknown audit rows before relying on those interfaces. Do not restore the deleted cache. Defer repository-wide facade migration outside this feature.

**F2: Draft staging needs one authority and awaited synchronization**
- Category: ownership / depth / duplication / ADR
- Affected: Line staging, existing draft stores and configuration draft orchestration.
- Concern: backend staging must not become a second authoritative UI draft layer. Reactive mirroring is fire-and-forget, while legacy explicit flushing remains active.
- Decision: **include** in S2/S3. Retain frontend UI draft authority under ADR-0012; backend validates the exact reviewed edit batch. One orchestrator stages the whole batch, with acknowledged synchronization before dependent preview/apply. Settle the shared headless lifecycle contract at the S2 HITL seam without adding a bypass or duplicate owner. Consolidate overlapping flush ownership rather than adding another path.

**F3: Generic facility teardown cannot prove safe Line resource release**
- Category: locality / ownership / correctness
- Affected: Line compiler/resource diff and existing facility lifecycle.
- Concern: generic event-leaf teardown is not proof that mast, circuit, driver and cross-node references can be released safely; unresolved leaves or metadata fallback cannot authorize destructive writes.
- Decision: **include** in S3 and widen in S5/S6. Put proven ownership, preimages and reference checks behind the Line proposal interface. Reuse persistence/apply mechanisms, not generic teardown assumptions. Retain ownership/recovery evidence until release is verified or safely pending.

**F4: Acknowledged writes are not verification**
- Category: depth / ownership / testability
- Affected: backend sync application, readback, operation evidence and UI state.
- Concern: existing sync records write acknowledgement as applied and updates tree values; that does not establish the Line's required hardware-read verification.
- Decision: **include from S3**, not as a final hardening slice. Extend the existing executor with per-patch outcomes, truthful unverified/diverged/unavailable state, durable recovery evidence and fresh-read retry. Do not create a second executor.

**F5: Hardware-dependent widening remains evidence-gated**
- Category: seam / YAGNI / testability
- Affected: internal-signal and cross-node compilation/application.
- Concern: measured same-board mapping and cross-board Stop/Clear do not establish cross-board Approach, receiver restart, final trigger behavior or sustained quiet-bus acceptance.
- Decision: **include**. Develop terminal one-block behavior independently; gate S4/S5 on the relevant manual evidence. Keep RG appearance separate from semantic Track Speed and do not introduce subjective blended-color validation.

### Seam Audit Disposition

The current-state audit covered save-delta collection, Railroad channel rendering,
user-owned channel lifecycle, channel event resolution, persistent in-memory
layout state, aggregate dirty state, lifecycle reset and configuration draft
mirroring. Audit dates were recorded in [aiwiki/seams.md](../../aiwiki/seams.md).
Relevant governing decisions include ADR-0002/0003/0004/0005/0009/0011/0012/0013/0015.
ADR-0019 remains a required compatibility gate if implementation adds protocol
emission or peer-state behavior; this assessment is not a protocol-wide re-audit.

The `/design` presentation supplied fixed invariant tables for ADR-0004,
ADR-0009, ADR-0011, ADR-0012 and ADR-0015. Preserve these dispositions:

| Audit result | Evidence / implementation consequence |
|--------------|---------------------------------------|
| ADR-0004 facade imports, projections and mutation ownership: Drift | Existing direct store imports/mutations in `+page.svelte`, `BowtieCatalogPanel.svelte` and `TreeLeafRow.svelte`; new Line surfaces must not repeat them |
| ADR-0011 aggregate implementation and participant dispatch: OK; exclusive consumer use: Drift | `effectiveNodeStore.svelte.ts` owns the aggregate, but the route's legacy Save branch still combines raw dirty signals |
| ADR-0011 close/disconnect guarantee and exclusive persistability consumption: Unknown | Close relies on a caller-provided disconnect callback; establish the relevant Line lifecycle guarantees with focused interface tests |
| ADR-0012 literal uniform store contract: Drift; exhaustive no-op suppression and Save/apply persistence wording: Unknown | Document actual interfaces and the authorized apply/recovery checkpoint semantics; validate Line mutations rather than assuming all current stores conform |
| ADR-0012 load-time repair: implemented, not established drift | Route reconciles dangling channel references after both channel/facility baselines load |
| Config-draft mirror single-owner extension: overlapping ownership | Legacy `flushDraftToBackend` remains alongside reactive mirroring; resolve at F2's seam |
| ADR-0015 CDI fallback, captured precedence and live-proxy CDI exclusion: OK | `proxy_snapshot_data`, `LayoutState` accessors and `NodeProxyHandle` implement these rules |
| ADR-0015 original saved-tree cache invariant: stale text | Later Option C extension explicitly deletes the cache; amend the invariant, do not recreate the cache |
| ADR-0015 saved-versus-pending offline-change ownership: Unknown | Save reads `offline_changes_cache`; clarify provenance and authority before placing Line recovery data |
| ADR-0015 draft refresh and discovered-role precedence: implemented | Save refreshes saved documents and clears drafts; discovered classifications merge with user-explicit-wins precedence |
| ADR-0009 typed registry/proxy and placeholder factory: OK; layout prefix gate: Drift | Use canonical typed identity in new Line paths; avoid copying the existing prefix-based binding gate |

Unknown is not a pass. These results justify F1's narrow corrections and
verification obligations; they do not authorize unrelated repository-wide fixes.

### Vertical Slices

| Slice | User-visible change | Demoable |
|-------|---------------------|----------|
| S1: Review a one-block Line | Complete the five-step flow and inspect terminal expectations and real bindings | Yes |
| S2: Keep or discard Line drafts | Stage, save, reopen, rename, change orientation or discard without hardware application | Yes |
| S3: Safely operate one signal | Apply, inspect verification, recover interrupted work and safely remove owned resources | Yes |
| S4: Extend on one board | Append a second block while retaining the first signal and demonstrating downstream restrictions | Yes, bench-gated |
| S5: Operate across nodes | Place signals on separate boards and recover visibly from partial application | Yes, bench-gated |
| S6: Maintain a three-block Line | Reorder, prepend, rebind and remove nonfinal members while retaining surviving identities | Yes |

**S1: Review a one-block Line**
- Type: **HITL**
- Intent: Prove the complete proposal interface with real terminal policy.
- Layers: Rendering -> frontend orchestration -> IPC/backend -> core compiler.
- Blocked by: None.
- Test: Guided and headless equivalent inputs exercise the same proposal service and produce equal normalized terminal proposals and scoped issues.
- Acceptance: All five steps are exercisable. Occupied means Stop; clear means Clear. Real outputs and protected block are reviewable. Missing facts remain unavailable and block staging/application where required. Preview performs zero writes and creates no lasting reservations.
- Architecture note: Establish the immutable proposal seam without a generic recipe abstraction. Fold necessary types/compiler foundation into this visible path.

**S2: Keep or discard Line drafts**
- Type: **HITL**
- Intent: Integrate the Line with the existing durable edit lifecycle.
- Layers: Rendering -> orchestration/stores -> backend -> core layout persistence.
- Blocked by: S1.
- Test: A validated proposal stages coherently, survives save/reopen as pending work and can be discarded through the real lifecycle adapter; UI/headless behavior agrees.
- Acceptance: The whole batch is staged or neither half is. Discard restores the baseline. Saving metadata/pending changes performs zero bus writes. Rename/orientation changes generate zero hardware patches and preserve operational order/identities. Old layouts load unchanged; malformed Line data errors explicitly.
- Architecture note: Resolve F2's authority, synchronization and headless contracts before implementation; extend existing facility drafts, aggregate dirty state and lifecycle participation.

**S3: Safely operate one signal**
- Type: **HITL**
- Intent: Establish truthful application, verification, recovery and guarded release.
- Layers: UI -> lifecycle orchestration -> backend sync/readback -> existing protocol interface -> hardware.
- Blocked by: S2 and supported terminal recipe evidence.
- Test: Exercise the actual staging/apply/readback interface with stale review, mid-node interruption, mismatch, unavailable readback, reopen and fresh-read recovery; verify guarded removal.
- Acceptance: Stale review never writes. Write acknowledgement alone is unverified. Interrupted or mismatching readback remains visible through reopen. Retry uses fresh facts. Removal releases only proven Line-owned resources and preserves shared detection/unrelated configuration. Required observed terminal cases match Stop/Clear 100%.
- Architecture note: Establish F3/F4's ownership and durable forward-recovery contract in the existing executor. Consolidate any draft flushing needed for reliable application; do not add a parallel apply path.

**S4: Extend on one board**
- Type: **HITL**
- Intent: Widen a working terminal Line to two ordered blocks on one host.
- Layers: Editor -> proposal/compiler -> draft/apply/readback -> hardware.
- Blocked by: S3 and completed relevant internal-signal bench evidence.
- Test: Append a block through the full lifecycle, preserve the first signal/resources and verify all four two-block occupancy combinations.
- Acceptance: Terminal-to-internal conversion is reviewed. Occupied precedence and the measured Stop/Approach/Clear mapping work. The receiving circuit belongs to the upstream host even for same-node links. Capacity checks use actual current allocations. RG Approach activates both drivers without a subjective color gate.
- Architecture note: Introduce only the narrow measured receiving-circuit implementation; no independently persisted downstream graph.

**S5: Operate across nodes**
- Type: **HITL**
- Intent: Widen the established Line lifecycle to multiple physical nodes.
- Layers: Editor -> backend/core -> existing apply/readback -> multiple nodes.
- Blocked by: S4 and completed relevant cross-board/receiver-restart evidence.
- Test: Exercise cross-node restriction propagation, receiver address readback and failures within and between nodes through the same operation/recovery interface.
- Acceptance: Receivers bind to hardware-read publisher Link Addresses. Cross-node restrictions match the tested mapping. Actual acknowledged/not-attempted/unknown progress survives reopen. Reconnect exposes safe recovery; no stale replay or assumed rollback. References on every affected node are checked.
- Architecture note: Widen S3's operation contract rather than introducing another executor or resource registry.

**S6: Maintain a three-block Line**
- Type: **AFK**
- Intent: Exercise structural maintenance through the established interfaces.
- Layers: Editor -> core identity/resource diff -> existing lifecycle -> affected nodes.
- Blocked by: S5.
- Test: Full lifecycle scenarios cover all eight three-block occupancy combinations and reorder/prepend/rebind/nonfinal removal, including same-node, cross-node, swapped-host and A-B-A placements.
- Acceptance: Surviving signal identities/resources are retained where compatible. Old/new hosts and references are checked before writes/releases. Both orientations preserve operational order and produce zero hardware changes when only orientation changes. Apply/readback/reopen/removal preserve shared detection and unrelated configuration.

The roadmap is acyclic. Every slice exposes a user-exercisable result; no
standalone store, schema, compiler-foundation or test-only slice is authorized.
Failure handling belongs in each affected slice, not in a final hardening pass.
The existing physical acceptance matrix and five-modeler review remain required.

### Deferred Improvements

Repository-wide facade migration is outside this feature; F1 includes only
directly relevant repairs and contract clarification. No new deferred-work issue
has been created. An issue proposal requires separate explicit confirmation.
Existing adjacent ideas #18, #22 and #23 are not prerequisites to absorb into
this feature.

### Architecture Decisions

No new ADR was created by this assessment. The approved shape preserves the
existing facilities/draft/persistence commitments. During implementation, extend
ADR-0012/0015 at the staging and recovery seams as the concrete contracts land;
correct stale invariant text rather than inventing a new generic architecture.
No terminology change or newly rejected approach requires a separate glossary
entry or ADR at this stage.
