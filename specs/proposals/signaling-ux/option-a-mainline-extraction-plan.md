# Option A — Mainline Reconciliation And Extraction Plan

Status: **execution plan, rebuilt from a complete branch inventory.** This
document describes how to retain approved work from `020-abs-signaling` on
`main`, reject the obsolete Tower-LCC-first ABS implementation, and then delete
the source worktree and branch.

## Goal

Start from `main`, account for every meaningful difference on
`020-abs-signaling`, integrate retained capabilities in owner-scoped changes,
and delete the source branch only after the final parity gate passes.

The source branch is evidence. Commits identify provenance, but they are not
integration units: several commits mix generic capabilities with obsolete ABS
policy, persistence, IPC, and presentation.

## Immutable Inventory Baseline

The inventory in this plan compares five distinct states:

| State | Revision or working-tree state |
|---|---|
| Common ancestor | `409bda798a607c44c3358e308740c20e50365610` |
| Initial `main` baseline | `ad6606b2f5afa8b31ed87a85c1046e21ae967611` |
| Current `main` | `9762fd5` (`Add Tower-LCC+Q profile support`; Units 1–4 landed) |
| Committed source | `0365d5ca04f5082f1f567d0d4e8a77c7a4f7a4f3` |
| Source worktree | committed source plus this uncommitted plan update only |

Do not reset, rebase, or otherwise rewrite these states while an inventory
decision still relies on them. If either branch advances, update this baseline
and re-run the commit and owner completeness checks.

## Governing Decisions

1. `ABS 3-Aspect Signal` was never released. It has no compatibility promise.
2. Do not merge the Tower-LCC-first compiler, allocation persistence, public
   facility flow, target selector, or prediction path into `main`.
3. Preserve a source concept only when it independently fits current product
   direction and has a current owner, behavior contract, and validation path.
4. Integrate functionality, not whole commits. The source implementation and
  its tests are the extraction baseline: port retained hunks together, then
  adapt only where current `main` creates a real conflict. A mixed commit may
  contribute to several retained units while its obsolete portions remain
  rejected. Do not independently reimplement retained behavior merely because
  the whole commit cannot be cherry-picked.
5. Every source-only commit and every materially changed owner must receive a
   disposition before the source branch can be deleted.
6. Preserve main-only behavior, especially the JMRI multiline-string heuristic
   from `01ff74c` and the current developer workflow and signaling direction.
7. Do not land generated SvelteKit output, unexplained lockfile drift, session
   handoffs, stale active Spec 020 claims, or incidental cleanup without an
   independently stated behavior contract.
8. Apply one integration unit at a time to `main`. Validate it and wait for
   explicit commit approval before starting the next unit.

## Extraction Procedure

For Units 3–8, **extract/port; do not redesign or rebuild from scratch**:

1. Inventory every file and hunk in the named source commit(s), excluding only
  surfaces this plan explicitly rejects.
2. Move the retained production implementation, existing tests, and associated
  documentation together. A no-commit cherry-pick or source patch is preferred
  when it cleanly preserves that provenance.
3. Resolve conflicts against evolved `main` at the equivalent owner seam. Keep
  newer mainline behavior; adapt the source hunk only as much as that conflict
  requires.
4. Run the moved focused tests and aggregate suites after the complete port is
  assembled. This is validation of existing behavior, not a new red-green TDD
  implementation cycle.
5. Add or rewrite production code or tests only when a current-main conflict or
  uncovered integration gap makes the source implementation insufficient, and
  record that deviation explicitly in the execution log.
6. Before approval, compare the candidate file/hunk inventory with the source
  evidence and list every omission. Generated output and other rejected noise
  must be omitted deliberately, not accidentally.

## Disposition Vocabulary

- **Retain** — independently valuable behavior that must exist on `main`.
- **Reconstruct** — retain the behavior, but not the source patch as a whole.
- **Already landed** — equivalent behavior or documentation is committed on
  `main`; preserve the main version.
- **Reject obsolete** — belongs to the superseded ABS/Tower-LCC-first design.
- **Reject noise** — generated output, lockfile drift, temporary handoff, or
  incidental cleanup without an independent contract.
- **Defer and re-derive** — preserve the requirement or question, but do not
  retain the source implementation. Resolve it in the headless pipeline design.

## Complete Commit Ledger

This ledger accounts for all 33 commits reachable from the source head after
the common ancestor, including the two merges. A commit marked mixed must not
be cherry-picked wholesale.

| Commit | Concern | Disposition | Integration destination or reason |
|---|---|---|---|
| `bf23d97` | Original Tower-LCC-first Spec 020 | Reject obsolete | Historical source only; do not publish as an active spec. |
| `659f52e` | Original slices and seam notes | Reject obsolete | Task plan implements the rejected architecture. |
| `bae83b6` | ABS template/compiler foundation plus `signal-aspect` role/style | Mixed: reconstruct | Retain only target-independent role/style concepts; reject compiled template types, shared slot flag, target selector, allocations, and registry entry. |
| `ee6d1d9` | Next-step ABS tasks | Reject obsolete | Historical planning only. |
| `e77fd7b` | Merge of then-current `main` | Provenance only | Its durable main-line work is represented by the later common ancestor. |
| `0964722` | Tower conditional-line compiler wiring plus profile filename normalization | Mixed: reconstruct | Retain SNIP identity → bundled filename normalization and its focused test; reject compiler, target capacity/selection, allocation, reset, and IPC flow. |
| `f474e55` | Real-time signal display and multi-row event resolution | Mixed: reconstruct | Retain target-independent multi-row resolution and observation-based display; reject ABS-facility coupling. |
| `aff0504` | ABS comprehension UI and allocation hydration | Reject/superseded | Reject allocation persistence and ABS comprehension; generic card presentation is taken from later commits. |
| `57662f1` | Declarative `SlotCard` presentation | Retain by reconstruction | Generic component foundation. |
| `1a7ddde` | Facility/slot UX refinement | Mixed: reconstruct | Retain role-neutral card behavior and tests; exclude ABS assumptions. |
| `54f1e1e` | ABS S5 plus build-workflow user-test instructions | Mixed: reconstruct | Retain only the developer-workflow instruction; reject S5 allocation/compiler changes. |
| `56dfb09` | Lockfile update and incidental FacilityCard CSS removal | Reject noise | No declared dependency change or independent behavior contract. |
| `1139296` | Initial profile assembler/runtime correction | Retain | Source evidence for Profile Event-Role Annotation. |
| `669899a` | Merge of `main` at the current common ancestor | Provenance only | No separate extraction; this establishes the comparison base. |
| `726c7ed` | ABS event wiring, inverse cleanup, and downstream-signal delete warning | Reject source implementation | Referrer logic is hard-coded to ABS `output`/`downstream-signal`; retain cleanup requirements for the future planner, not this code. |
| `bf697f7` | Draft-aware event IDs, signal observation, and ABS prediction | Mixed: reconstruct | Retain effective drafted-value resolution and observation tests; reject prediction and ABS wiring behavior. |
| `c08423e` | Tower-LCC+Q bundled profile plus lockfile drift | Mixed: retain profile | Retain the profile after validation; reject lockfile drift. |
| `b7c339a` | Shared daughterboard metadata for Tower-LCC+Q | Retain | Land with the profile as one validated unit. |
| `d72499b` | USB sleep/disconnect recovery | Mixed: reconstruct | Retain Transport Health/session ownership and tests; reject `.svelte-kit` output. |
| `4f2b862` | Shared-observer/exclusive-claim filtering | Defer and re-derive | Current implementation exists for obsolete ABS shared slots. Preserve the resource-claim question for the headless planner; do not add unused schema now. |
| `610425f` | CDI-backed channel/facility configuration navigation | Retain by reconstruction | Keep path-label resolution, navigation, and supported-binding tests. |
| `1df8aa3` | Accessible `SingleSelectList` and supported picker migration | Retain | Generic UI unit. |
| `bf6c12e` | Logic target selector fix plus radio layout fix | Mixed: reconstruct | Retain only `flex-shrink: 0` in `SingleSelectList`; reject target selector and route flow. |
| `67ce5b5` | Layout aggregate schema v5 for ABS allocations | Reject obsolete | Main remains on the current schema unless a future owner decision requires a new version. |
| `a4d6bbd` | Initial revised signaling proposal | Superseded | Later signaling decisions and storyboard are the durable version. |
| `ff5ecdf` | Hardware research, mockups, and old Spec 020 slice notes | Mixed | Hardware evidence/storyboard are already landed; reject old Spec 020 task artifacts; reconcile index links separately. |
| `a7139d4` | Practical-first direction and constraints | Already landed | Preserve main's published form; review backlog differences rather than replacing it. |
| `81df48a` | Signal Plant direction | Already landed | Included in main's signaling UX publication. |
| `92c69a4` | Storyboard Step 3 refinement | Already landed | Included in main's signaling UX publication. |
| `3046ff9` | GPT-5.6 Sol agent workflow | Already landed | Equivalent main commit is `b6ba969`; only the separate `54f1e1e` user-test instruction remains. |
| `695cdbb` | Leaf-scoped Signal-LCC role classification | Retain | Landed on `main` in `be21bed` with reconstructed owner/flow/seam documentation. |
| `f880bc9` | Stable C7c fixture and contract test | Retain | Landed on `main` in `be21bed` with the shipping-profile contract. |
| `0365d5c` | Bench findings and current extraction/headless plans | Mixed | Durable evidence/direction are already in `ad6606b`; reject session handoffs; replace the old extraction plan with this inventory-backed plan. |

## Main-Only Commit Ledger

| Commit | Behavior | Required treatment |
|---|---|---|
| `01ff74c` | JMRI heuristic: strings longer than 64 bytes use multiline editing | Preserve and rerun its focused component tests after frontend extraction. |
| `b6ba969` | Current GPT-5.6 Sol developer agents/prompts/skills | Preserve as the landed Phase 1 equivalent. |
| `ad6606b` | Signal-LCC hardware evidence and signaling UX direction | Preserve as the landed documentation baseline. Reconcile indexes and backlog; never overwrite from source wholesale. |

## Capability And Owner Ledger

The commit ledger prevents history omissions. This capability ledger prevents a
mixed commit from hiding behavior at a shared owner.

| Capability or owner | Disposition | Evidence | Dependency or exclusion |
|---|---|---|---|
| Developer-agent workflow | Already landed, plus one retained hunk | `b6ba969`, selected `54f1e1e` | Add user-test instructions separately; no runtime files. |
| Durable Signal-LCC evidence and signaling direction | Already landed | `ad6606b` | Reconcile `aiwiki/README.md`, `product/README.md`, proposal indexes, and backlog without reviving superseded mockups. |
| Profile Event-Role Annotation | Already landed | `1139296`, `695cdbb`, `f880bc9`; main `be21bed` | Preserve leaf-scoped generation, runtime annotation, parity tests, and reconstructed owner/flow/seam documentation. |
| USB connection-session/Transport Health recovery | Reconstruct | `d72499b` | Exclude generated `.svelte-kit`; validate backend, `lcc-rs`, and frontend lifecycle together. |
| Tower-LCC+Q profile and daughterboard metadata | Retain | `c08423e`, `b7c339a` | Exclude lockfile; require representative CDI/profile-loading coverage. |
| Accessible single-selection UI | Retain | `1df8aa3`, selected `bf6c12e` | Migrate a currently supported picker; no logic target selector. |
| Role-neutral `SlotCard` presentation | Reconstruct | `57662f1`, `1a7ddde` | Demonstrate with Block Indicator; exclude prediction/allocation/comprehension. |
| CDI-backed configuration navigation | Reconstruct | `610425f` | Depends on the generic card surface and current `NodeConfigTree`; test connector input and lamp row bindings. |
| Draft-aware event-ID resolution | Retain as its own unit | selected `bf697f7` | Use `effective_value()` for current and future channel resolution; do not couple it to ABS. |
| Target-independent signal-aspect channel/display | Reconstruct | selected `bae83b6`, `f474e55`, `bf697f7` | Depends on profile correctness and draft-aware/multi-row event resolution; exclude public ABS policy and prediction. |
| Shared observer/exclusive resource claims | Defer and re-derive | `4f2b862` | No currently supported shared slot. Define claims in the headless planner only when intent requires them. |
| Facility referrer warning | Reject source implementation | selected `726c7ed` | Hard-coded to obsolete downstream-signal topology. Revisit only with a target-neutral reference model. |
| Generic facility deletion and inverse cleanup | Preserve current main behavior | current Spec 018 owners; selected requirements from `726c7ed` | Future generated plans must own symmetric cleanup; do not retain old compiler types. |
| Tower-LCC compiler and wiring plan | Reject obsolete | `bae83b6`, `0964722`, `54f1e1e`, `726c7ed` | No production module, IPC command, allocation delta, or target selector. |
| ABS prediction/comprehension | Reject obsolete | `aff0504`, `bf697f7` | Observation is retained; policy prediction is not. |
| Layout schema v5 and `LogicAllocation` | Reject obsolete | `aff0504`, `54f1e1e`, `67ce5b5` | No unreleased compatibility requirement. |
| Headless Signal-LCC intent pipeline | New implementation | `headless-intent-pipeline-plan.md` | Starts only after retained extraction units and final source inventory review. |

## Ordered Integration Units

The order below is based on owner dependencies, not source chronology.

### Unit 1 — Complete The Frozen Signal-LCC Profile Correction

Current status: **landed on `main` in `be21bed`.**

Retain:

- exact leaf-scoped mixed producer/consumer extraction metadata;
- SNIP identity → bundled profile filename normalization for punctuation and whitespace;
- assembler expansion, type validation, deduplication, and conflict rejection;
- runtime leaf annotation across replicated CDI groups;
- generated and bundled profile updates, including the Tower-LCC declaration
  migration required by the corrected profile schema;
- stable C7c fixture and shipping-profile contract test;
- owner, flow, seam, and architecture-health documentation.

The landed unit is cohesive. Direct blob comparison showed that its production,
metadata, generated output, bundled output, and tests matched source head. Its
`Profile Event-Role Annotation` flow/seam and relevant owner entries were
reconstructed without importing unrelated ABS documentation.

Acceptance:

- Python profile-tool tests pass;
- focused profile annotation and bundled Signal-LCC contract tests pass;
- full `bowties-core` tests pass;
- generated and bundled Signal-LCC profiles agree;
- mixed Rule-to-Aspect leaves retain Consumer `set aspect` and Producer
  `aspect is set`/`aspect cleared` roles across all replicas;
- landed commit contains only this unit and its documentation.

### Unit 2 — Publish The Reconciliation Plan And Workflow Remainder

Retain:

- this inventory-backed plan;
- the build-skill requirement to provide concise user-test instructions from
  `54f1e1e`;
- missing index links for already-published hardware/product documentation.

Reconcile, do not copy wholesale:

- `specs/backlog.md` against the actual main tree;
- proposal mockups, keeping only artifacts still identified as current;
- `aiwiki/README.md` and `product/README.md` indexes.

Acceptance:

- customization diagnostics are clean;
- Markdown links are clean;
- no runtime files change;
- no active document presents old Spec 020 as the shipping direction.

### Unit 3 — USB Sleep/Disconnect Recovery

Source: `d72499b`.

Retain connection-session ownership, bounded writer/health behavior, frontend
session reconciliation, focused tests, ADR-0017 extension, and aiwiki updates.
Exclude all `app/src-tauri/.svelte-kit/**` files.

Acceptance:

- sleep, disconnect, and reconnect transitions have focused tests;
- `lcc-rs`, backend, and frontend lifecycle suites pass;
- generated files do not appear in the diff.

### Unit 4 — Tower-LCC+Q Profile Support

Source: `c08423e`, `b7c339a`.

Retain the profile and shared daughterboard metadata as one profile-owned unit.
Exclude `app/package-lock.json`.

Acceptance:

- representative CDI loads without warnings;
- daughterboard selection resolves expected connectors;
- profile-loading tests and relevant `bowties-core` tests pass;
- no dependency on ABS or signaling-intent code exists.

### Unit 5 — Accessible Single-Selection UI

Source: `1df8aa3`, the one-line generic refinement from `bf6c12e`.

Retain `SingleSelectList`, its harness/tests, and migration of an existing
supported facility picker. Exclude `LogicTargetSelector` and its route state.

Acceptance:

- keyboard, selection, confirmation, disabled-state, and layout tests pass;
- the migrated picker works with Block Indicator;
- full Vitest and touched-surface Svelte diagnostics pass.

### Unit 6 — Generic Slot Presentation And Configuration Navigation

Source: selected `57662f1`, `1a7ddde`, and `610425f`.

These source changes form one user-visible capability: a role-neutral slot card
can show its CDI-derived configuration target and navigate to it. Keeping them
together avoids landing a second card abstraction without its demonstrated use.

Retain:

- declarative `SlotCard` rendering;
- CDI/profile-aware path-label construction;
- instance-aware tree traversal;
- role-neutral resolution for currently supported connector-input and lamp-row
  bindings;
- navigation through `configFocusStore`;
- route/component plumbing and focused tests.

Exclude ABS comprehension, prediction, target selection, allocations, and
hard-coded Stop/Approach/Clear policy.

Acceptance:

- Block Indicator and supported standalone channels show useful CDI-derived
  targets and navigate to the correct configuration section;
- missing tree/profile data degrades safely;
- focused component/utility tests and full Vitest pass;
- main-only multiline editing remains covered and unchanged.

### Unit 7 — Draft-Aware Channel Event Resolution

Source: selected `bf697f7`.

Make current channel event resolution use the effective leaf value so unsaved
draft Event IDs resolve consistently with the configuration UI. Keep this
separate from signal policy and multi-row display.

Acceptance:

- committed-only values still resolve;
- `modified_value` wins when present;
- existing connector and lamp-row consumers remain unchanged;
- focused and full `bowties-core` tests pass.

### Unit 8 — Target-Independent Signal-Aspect Capability

Source: selected `bae83b6`, `f474e55`, and non-prediction portions of
`bf697f7`.

Retain:

- `signal-aspect` channel role;
- validated Signal-LCC-backed display style(s);
- multi-row lamp binding and event resolution;
- observation-based aspect and lamp-state derivation;
- role/style/profile validation and channel display tests.

Exclude:

- `ABS_3_ASPECT_SIGNAL` and every production `abs-3-aspect-signal` identifier;
- compiled behavior-template fields and shared-slot schema added only for ABS;
- `logic_adapter`, `WiringPlan`, allocation persistence/deltas, target IPC/UI,
  and prediction-first rendering.

Acceptance:

- a signal-aspect channel can be represented, validated, navigated, and
  observed without an ABS facility;
- existing Block Indicator and lamp channels remain unchanged;
- all retained tests use target-independent terminology;
- production searches find none of the obsolete surfaces listed above.

### Unit 9 — Final Reconciliation And Source-Branch Retirement

Before deleting the source worktree or branch:

1. Re-run the 33-commit ledger against the final source head.
2. Re-run the capability/owner ledger against final `main`.
3. Confirm every Retain/Reconstruct item is committed and validated.
4. Confirm every main-only behavior remains present.
5. Search production code for rejected template IDs, compiler modules, IPC
   commands, allocation types/deltas, schema-v5 rationale, prediction helpers,
   and target-selector UI.
6. Confirm generated output, lockfile drift, and session handoffs did not land.
7. Reconcile `specs/backlog.md`, removing completed work and retaining only
   actionable future work not owned by another active plan.
8. Confirm the source worktree contains no unique uncommitted durable work.
9. Optionally create a safety tag at the final source commit, remove the
   worktree, and delete `020-abs-signaling`.

### Unit 10 — Build The Headless Signal-LCC Intent Pipeline

This is new implementation, not extraction. Follow
[headless-intent-pipeline-plan.md](headless-intent-pipeline-plan.md) only after
Unit 9 proves that retained branch work is safely on `main`.

Risk-first sequence:

1. typed Signal Mast intent and bundled policy;
2. pure Signal-LCC plan preview;
3. validation against current CDI and effective configuration;
4. normal draft/application workflow;
5. generated reproduction of the proven single-mast bench configuration;
6. generated Track Circuit cascade acceptance;
7. signaling wizard as an adapter that produces the same intent.

Re-derive resource claims, reference topology, diagnostics, and inverse cleanup
from this pipeline's actual contracts. Do not revive source branch types merely
because they addressed similarly named concerns.

## Explicitly Rejected Production Surface

The final `main` tree must not contain source implementations of:

- the `ABS 3-Aspect Signal` public template or identifier;
- `CompilationTarget`, `RuleCondition`, or `ConditionActionRule` as artifacts
  of the old behavior-template compiler;
- Tower-LCC `logic_adapter` or `WiringPlan` production modules;
- `LogicAllocation`, allocation persistence, or Allocate/Free layout deltas;
- layout schema v5 introduced solely for those allocations;
- logic capacity, compile/reset, or target-selection IPC and UI;
- downstream-Stop or predicted-aspect facility rendering;
- downstream-signal-specific deletion/referrer UI;
- generated `app/src-tauri/.svelte-kit/**` files;
- unexplained `app/package-lock.json` changes;
- session-handoff documents;
- active Spec 020 claims that the old facility is current product direction.

## Repository Hygiene Gates

Apply to every integration unit:

- `git diff --check` passes for the exact candidate;
- focused tests pass before aggregate suites;
- full relevant Rust/Vitest suites pass;
- touched files introduce no new static diagnostics;
- generated and lock files change only when the unit explicitly owns them;
- aiwiki owners/flows/seams and durable product docs change with their owner;
- staged diff is inspected, not only the working-tree diff;
- no unrelated integration unit is stacked before explicit commit approval.

## Execution Log

| Unit | Source evidence | Landed commit on `main` | Result | Validation |
|---|---|---|---|---|
| Baseline — multiline editor | `01ff74c` | `01ff74c` | landed | existing focused tests |
| Baseline — developer agents | `3046ff9` equivalent | `b6ba969` | landed | customization review completed |
| Baseline — evidence/direction | `ff5ecdf`..`0365d5c` selected | `ad6606b` | landed | documentation publication completed |
| 1 — Signal-LCC profile correction | selected `0964722`; `1139296`, `695cdbb`, `f880bc9` | `be21bed` | landed; runtime confirmed on Signal-LCC | 5 Python tests; 428 core + 4 smoke + 1 contract tests; 16-declaration parity; backend tests compile; backend build passes; Windows DLL blocks Tauri test execution |
| 2 — reconciliation plan/workflow | selected `54f1e1e`, this plan | `3d3628a` | landed | customization diagnostics clean; local Markdown links resolve; 33-commit ledger complete; no runtime files changed |
| 3 — USB recovery | `d72499b` selected | `9313c33` | landed | source implementation and tests ported directly; 14 focused + 455 aggregate `lcc-rs` tests pass; 46 focused frontend tests pass; full Vitest 1447/1448 with the unrelated timeout passing alone; backend tests compile and backend builds; Windows DLL blocks backend test execution |
| 4 — Tower-LCC+Q profile | `c08423e`, `b7c339a` | `9762fd5` | landed with byte-identical source profile/metadata; lockfile excluded; representative CDI contract added | 428 core + 4 smoke + 1 Signal-LCC contract + 1 Tower-LCC+Q contract tests pass; backend focused test compiles but Windows DLL blocks execution |
| 5 — single-selection UI | `1df8aa3`, selected `bf6c12e` | this commit | source component, tests, and all three supported picker migrations ported; obsolete logic-target selector and route state excluded | 24 focused + 84 multiline-editor regression tests and full Vitest 1460/1460 pass; production build passes; touched files have no diagnostics; repository-wide `svelte-check` remains blocked by 123 pre-existing errors in 32 unrelated files |
| 6 — slot navigation | selected `57662f1`, `1a7ddde`, `610425f` | pending | pending | pending |
| 7 — draft-aware event resolution | selected `bf697f7` | pending | pending | pending |
| 8 — signal-aspect capability | selected `bae83b6`, `f474e55`, `bf697f7` | pending | pending | pending |
| 9 — final reconciliation/branch deletion | complete ledgers | pending | pending | pending |
| 10 — headless pipeline | new implementation | pending | pending | pending |

Unit 4 integration deviation: the two source commits contain the production
profile and shared metadata but no tests or committed Tower-LCC+Q CDI capture.
The mainline candidate therefore adds a stable representative CDI fixture and
a bundled-profile contract test that exercises every declared role path, the
v1.15 firmware signature, both connector slots, and the Q-specific detector
constraints. The retained production YAML remains byte-identical to source.

Unit 5 integration deviation: the source `AddFacilityDialog` test fixtures use
the rejected ABS template names and cast obsolete compiler fields into the
current `BehaviorTemplate` type. The mainline test preserves the same five UI
contracts with role-neutral fixture names and the current type shape. The
production component and picker migrations otherwise match the retained source
hunks; current main's narrower Block Indicator-only `AddChannelPicker` role and
style prop types remain in place.
