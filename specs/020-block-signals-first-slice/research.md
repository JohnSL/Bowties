# Research: One-Direction Automatic Block Signals

**Date**: 2026-11-05 | **Scope**: [spec.md](spec.md)

## 1. Extend the existing facility and draft owner

**Decision**: Represent the Line in the existing facilities aggregate; extend layout edit deltas, effective draft views and persistence. Do not store a parallel copy in a new Line file or frontend-only authoritative store.

**Rationale**: The current `LayoutState` owns saved/captured/draft layers, including effective facilities. Layout directory IO already writes `facilities.yaml` and pending node changes. Stable signal identity and generated-resource ownership must survive save/open with those layers.

**Alternatives considered**: New `signal-lines.yaml` plus a separate store (duplicate ownership and lifecycle); directly modifying committed facilities during the wizard (violates draft semantics).

**Evidence**:
- [Layout state](../../bowties-core/src/layout/state.rs), `LayoutState`, `DraftLayer`, `effective_facilities`, `sync_drafts`, `clear_drafts`.
- [Layout deltas](../../bowties-core/src/layout/types.rs), `LayoutEditDelta`, `apply_layout_deltas`; legacy `SCHEMA_VERSION = "1.0"`.
- [Directory IO](../../bowties-core/src/layout/io.rs) and [manifest](../../bowties-core/src/layout/manifest.rs), manifest schema version 4.
- [ADR-0012](../../product/architecture/adr/0012-all-layout-edits-draft-layer.md), [ADR-0015](../../product/architecture/adr/0015-backend-layout-state-single-owner.md).

## 2. One explicit policy; orientation is not operational direction

**Decision**: Derive immediate downstream signals from entered travel order. Persist stable identities; exclude eastward/westward from the hardware projection. Terminal Stop/Clear and internal Stop/Approach/Clear use the exact spec mapping.

**Rationale**: The proposal's conservative propagation language is superseded by the spec: next Approach means upstream Clear when the protected block is clear. Node placement must not change this policy.

**Alternatives considered**: Reversing membership for westward (contradicts FR-006); persisting an editable graph alongside the ordered list (two sources of truth); propagating Approach indefinitely (contradicts FR-005).

**Evidence**: [Spec](spec.md), clarification and FR-004 through FR-006, FR-012/013.

## 3. Narrow Signal-LCC compiler, using real profile paths

**Decision**: A pure `bowties-core` compiler produces reviewable per-node config patches from the Line and resolved hardware facts. Backend adapters provide observed facts and own application. Reuse replicated binding helpers; do not introduce a general recipe registry.

**Rationale**: The current profile defines mast/rule, conditional, driver and circuit paths. The shared 16-driver pool and mast/logic/receiver limits must be validated against occupied slots. No existing Automatic Block Signals compiler or dedicated apply contract was found.

**Alternatives considered**: Logic tables constructed in UI components (wrong owner and no headless parity); hard-coded numeric CDI addresses (firmware/profile drift); general style/recipe engine (no second real call site).

**Evidence**:
- [Placement rules](../../product/architecture/code-placement-and-ownership.md).
- [Hardware model](../../product/hardware/signal-lcc/hardware-model.md).
- [Bundled profile](../../app/src-tauri/profiles/RR-CirKits_Inc._Signal-LCC.profile.yaml).
- [Profile contract tests](../../bowties-core/tests/bundled_signal_lcc_profile_contract.rs).
- [ADR-0013](../../product/architecture/adr/0013-channel-role-style-ownership.md), rejection of speculative role registries and requirement to reuse replicated binding helpers.

## 4. Track Circuit mapping requires measurement

**Measured update, 2026-11-07:** the [single-board record](manual-track-circuit-test.md) establishes Stop -> Approach and Approach/Clear -> Clear with `Track Speed = Stop`, true -> Approach, false -> Clear; `On Variable Change` stopped the command flood. The [two-board record](manual-two-board-test.md) confirms Stop/Clear reception after correcting the receiver binding to freshly read publisher address `02.01.57.10.09.96.07.30`. Attempts to write `...04.68` to the publisher were not retained. Its edited/saved value was not authoritative.

The 12:31:46-47 trace records publisher Clear speed `...09.96.07.37` -> S4r Clear command `...09.97.07.3E` / acknowledgement `...07.3F`, and Stop speed `...09.96.07.30` -> Approach command `...09.97.07.3B` / acknowledgement `...07.3C`. Resolve generated publisher addresses from hardware before constructing cross-node receiver patches, and verify receiver readback. Speed is represented by distinct EventIDs, not an additional payload.

Cross-board Approach, receiver restart/initialization, the final cross-board trigger setting, exact firmware revisions, and sustained quiet-bus behavior remain unverified. These results support the recipe but do not close full internal-signal hardware acceptance.

**Decision**: Use a small [single-board experiment](manual-track-circuit-test.md), followed by [cross-board reception and one receiver-restart check](manual-two-board-test.md), before finalizing the recipe. Test one candidate Stop comparison against actual Stop/Approach/Clear publications, with one defined Approach-selector follow-up only if needed. We need a usable Stop-versus-non-Stop distinction, not a complete eight-selector characterization. Record the working selector/action mapping and firmware/CDI evidence.

**Rationale**: Local evidence establishes the mechanism, not exact equality versus threshold semantics. A mast publishes its Link Address; each upstream host uses a local receiving circuit with that address. The vocabulary includes Stop, Restricting/Tumble Down, Slow, Medium, Limited, Approach, Approach-Medium and Clear/Proceed. No numeric selector or event-range encoding is inferred from the order of those labels.

**Alternatives considered**: Assuming "Approach or better"; treating speeds as arbitrary application enum integers; replacing actual Track Circuit reception with detector-event lookahead; silently weakening the three-block acceptance scenario. All would miss the required hardware behavior.

**Evidence**:
- [Track Circuits](../../product/hardware/signal-lcc/track-circuits.md), speed vocabulary and explicit warning to verify comparison semantics.
- [Rule-to-Aspect](../../product/hardware/signal-lcc/rule-to-aspect.md), rule Track Speed and Link Address publication.
- [Clean-room procedure](../../product/hardware/signal-lcc/clean-room-test-procedure.md), Track Circuit cascade phase not established by the earlier direct-detector ladder.
- Local manual research did not find a definitive exact-match/threshold statement.

**Resolved planning question**: The user requested Bowties-native field/value tables grouped by Mast/Rule/Appearance Lamp and Logic/Variable/Action, not general application/reset instructions. Use fixed test slots and generated-ID copies; no current exports or literal IDs are prerequisites. Bowties does not provide the JMRI event-field Trigger menu, so the existing Bt/B2 buttons drive a known conditional group selecting publisher rules. Never reset or reconfigure the Tower-LCC. Same-board mapping and cross-board Stop/Clear reception are measured as recorded above; remaining discovery checks and full occupancy/placement/recovery acceptance are still open.

## 5. RG support does not require subjective yellow approval

**Decision**: Retain the two-driver searchlight: red=Stop, red+green=Approach, green=Clear. Validate technical output/wiring compatibility and the actual Rule-to-Aspect output combinations. Do not require acceptable-yellow certification, a modeler color-approval flag, a dedicated yellow driver, or a flashing proxy.

**Rationale**: The latest 2026-11-05 clarification supersedes the earlier "working arrangement" interpretation: "we don't need to validate if it's 'acceptable' because that's a modeler's choice." RG reduces output-driver demand compared with a dedicated R/Y/G mapping, so the modeler chooses the resource/color-quality tradeoff. This does not waive electrical compatibility, resource checks or correct output activation.

**Alternatives considered**: Requiring a third yellow driver (outside this slice); rejecting RG because blended yellow is imperfect (a modeler preference, not a technical failure); flashing red as Approach (withdrawn by the user); declaring arbitrary red/green equipment electrically compatible (not established).

**Evidence**: Latest user clarification above; [spec FR-007](spec.md); [Rule-to-Aspect](../../product/hardware/signal-lcc/rule-to-aspect.md) supports multiple Appearance entries per rule. The pasted external AI answer's single-output claim is contradicted by that structure. Conditionals select a rule, the rule carries Track Speed, and its appearances select lamps. Positional driver labels do not establish actual lamp colors.

**Validation consequence**: The [single-board procedure](manual-track-circuit-test.md) includes a short command/output check, then the unknown receiving comparison/trigger behavior. The [two-board procedure](manual-two-board-test.md) changes only the publisher location/binding and checks receiver initialization. Wider host-placement, startup/reconnection and application-recovery cases remain later acceptance. Neither experiment judges perceived yellow.

## 6. Reuse apply ownership; extend truthful recovery evidence

**Decision**: Integrate generated changes into existing layout drafts/offline changes and sync application. Add backend-owned operation evidence sufficient for partial node writes and readback; use current save/recovery machinery instead of a second transaction engine.

**Rationale**: Existing `apply_sync_changes` writes config changes and updates baselines, while non-config changes can be marked applied without writes. That generic status alone cannot prove a Line's hardware configuration is complete. A node can fail midway through its own patch, and multiple nodes are never atomic.

**Alternatives considered**: Marking the Line applied when its facility delta saves; all-or-nothing labels for multiple physical nodes; blind rollback/replay; using frontend desired values as readback. These create success-shaped recovery failures.

**Evidence**:
- [Backend sync](../../app/src-tauri/src/commands/sync_panel.rs), `apply_sync_changes`.
- [Sync API](../../app/src/lib/api/sync.ts), `ApplySyncResult`.
- [Post-apply orchestration](../../app/src/lib/orchestration/syncApplyOrchestrator.ts).
- [Draft mirror](../../app/src/lib/orchestration/configDraftMirrorOrchestrator.svelte.ts).
- [Layout open/save](../../app/src-tauri/src/commands/layout_capture.rs), `save_layout_directory`, `save_layout_with_bus_writes`, `open_layout_directory` and recovery/reconciliation results.

## 7. Desktop contracts and parity

**Decision**: Document typed IPC and in-process service contracts, not OpenAPI/REST. The guided flow and headless scenario use the same domain proposal and lifecycle service.

**Rationale**: The application has Tauri commands and events, not an HTTP API. Endpoint generation would introduce a nonexistent transport and duplicate validation.

**Alternatives considered**: REST server solely to satisfy a generic planning template; separate headless compile-only route. Neither fits the existing architecture or FR-017.

**Evidence**: [IPC contracts](contracts/tauri-ipc.md), existing draft/sync commands cited above, spec FR-017.

## Prior-work and quality checks

The conversation's single open `kind/idea` scan returned #23, #22, #21, #18, #15 and #11. No Automatic Block Signals-specific idea was found; adjacent ownership/profile topics do not authorize expanding this scope. No issue was created or edited.

Use [testing strategy](../../product/quality/testing-strategy.md) for implementation validation. Relevant existing tests are listed in [plan.md](plan.md). This planning change does not run application tests.

## Evidence to capture before hardware acceptance

- Actual Signal-LCC firmware/CDI identity and technically compatible RG output wiring; no subjective yellow-approval evidence.
- Initially: one working comparison/action mapping for Stop versus Approach/Clear, same-/cross-board responses and one receiver-initialization observation, with actual publications/commands captured.
- Tested rule order, conditional functions, selector values and per-signal resource cost.
- Later acceptance: one-/two-/three-block behavior, swapped-host and A-B-A placements, broader startup/input-loss/recovery observations and post-implementation interrupted-apply evidence; not all are required before design.

If evidence contradicts the specified mapping, stop and revise the affected scope with the user; do not substitute a software preview for physical acceptance.
