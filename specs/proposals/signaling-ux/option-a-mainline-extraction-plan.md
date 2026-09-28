# Option A — Mainline Extraction Plan

Status: **execution plan.** This document describes how to salvage durable work from `020-abs-signaling` without merging its obsolete Tower-LCC-first ABS facility implementation.

## Goal

Start from `main`, integrate independently valuable capabilities in owner-scoped changes, and then implement the headless Signal-LCC intent pipeline from the current product direction.

The unmerged branch is evidence and a source of selected changes. It is **not** the unit to merge.

## Governing Decisions

1. The developer-agent improvements land before every other extracted change.
2. `ABS 3-Aspect Signal` was never released; remove it rather than preserve compatibility.
3. Do not merge the Tower-LCC-first ABS compiler, public facility flow, allocation persistence, target selector, or prediction path as production architecture.
4. Preserve concepts only when they independently fit the Signal-LCC-first plan: typed intent, ordered policy, reviewable plans, event-wiring requirements, resource claims, diagnostics, and inverse cleanup.
5. Every extracted change has one architectural owner and passes its own validation.
6. Do not merge generated SvelteKit output, unexplained lockfile drift, or session-handoff files.

## Workflow

This is a single-developer repository. Extraction items are **not** landed via feature branches or pull requests. Instead:

1. Each extraction item is applied directly to `main` in the working tree, one item at a time, as a staged (or unstaged) change set — not yet committed.
2. The developer validates the change locally against the item's Acceptance criteria and the Repository Hygiene Gates.
3. Only when the developer explicitly says to commit does the change land as a commit on `main`.
4. If validation fails, the working-tree change is amended, reset, or discarded before the next item begins.
5. The next extraction item does not start until the previous one is either committed or discarded. Do not stack unrelated in-progress items in the working tree.

"Land on `main`" throughout the rest of this document means apply to the `main` working tree and wait for explicit commit approval, not push, merge, or open a PR.

## Phase 0 — Preserve The Current Worktree

Complete this phase on `020-abs-signaling` before resetting, rebasing, or reconstructing anything.

### 0.1 Review the working tree

Classify current uncommitted files into two durable commits:

**Hardware findings and architecture evidence**

- `product/hardware/signal-lcc/**`
- the Signal-LCC invalid-conditional-boundary entry in `aiwiki/architecture-health.md`

**Signaling direction and planning**

- `specs/proposals/signaling-ux/README.md`
- `specs/proposals/signaling-ux/design-decisions.md`
- `specs/proposals/signaling-ux/goals-and-constraints.md`
- `specs/proposals/signaling-ux/signaling-ux-practical-first-storyboard.html`
- `specs/proposals/signaling-ux/headless-intent-pipeline-plan.md`
- this extraction plan

Do **not** commit:

- `specs/proposals/signaling-ux/session-handoff-2026-09-07.md`
- `specs/proposals/signaling-ux/session-handoff-phase2-rewrite-2026-09-07.md`

Those are temporary session aids, not durable product or planning artifacts.

### 0.2 Validate and commit

For each durable commit:

- run `git diff --check` on the included files;
- inspect the staged diff, not only the working-tree diff;
- use a message that names the evidence or planning decision;
- record the resulting commit IDs in the execution log below.

These commits preserve work on the source branch. They are not permission to merge the branch wholesale.

### 0.3 Tag the source point

After the preservation commits, create a local safety tag or record the final source commit ID. This gives later extraction sessions one immutable reference even if the branch is subsequently edited.

## Phase 1 — Developer-Agent Improvements First

This is the first change integrated from the source branch.

### Source

- commit `3046ff9` — developer prompts, agents, skills, and Copilot instructions

### Method

Apply `3046ff9` directly on `main` in the working tree. Cherry-pick is appropriate only if it applies cleanly and the complete diff remains coherent against current `main`; otherwise reconstruct the same behavior. Wait for explicit commit approval before committing.

### Scope

- `.github/agents/**`
- `.github/prompts/**`
- `.github/skills/**`
- `.github/copilot-instructions.md`

Exclude every product, application, profile, or signaling file.

### Acceptance

- customization files have valid frontmatter and references;
- removed worker agents are not referenced as required callable agents;
- architecture-first and TDD delegation instructions remain internally consistent;
- no application/runtime files change;
- this change is committed on `main` before the next extraction item begins.

## Phase 2 — Publish Durable Evidence And Direction

After Phase 1 is committed on `main`, apply a documentation-only change directly on `main`.

### Include

- current Signal-LCC hardware documentation and bench findings;
- signaling UX charter, current decisions, and current storyboard;
- the headless intent pipeline plan;
- this extraction plan;
- proposal index updates.

### Exclude or supersede

- session-handoff files;
- old active Spec 020 as a current plan;
- claims that the Tower-LCC-first `ABS 3-Aspect Signal` implementation is the shipping direction.

### Spec 020 treatment

Do not copy `specs/020-abs-signaling/**` to `main` as an active feature specification. Preserve useful historical reasoning only if it is explicitly moved under `specs/archive/**` with a supersession note pointing to the signaling UX charter and headless pipeline plan. Otherwise leave it on the source branch as historical evidence.

### Acceptance

- durable docs distinguish measured hardware behavior from planned behavior;
- current direction is Signal-LCC-first and railroad-intent-first;
- no document instructs users to create the obsolete ABS facility;
- Markdown diagnostics and link checks are clean.

## Phase 3 — Independent Runtime And Profile Changes

Each item is a separate change applied to `main` in isolation, validated, and committed on explicit approval before the next item begins. Order within this phase may change when dependencies demand it, but do not combine the items merely because they originated on one branch, and do not stack them in the working tree simultaneously.

### 3A — Signal-LCC profile correctness

Source evidence:

- `1139296`
- `695cdbb`
- `f880bc9`

Retain:

- leaf-scoped mixed producer/consumer event-role generation;
- assembler conflict validation;
- bundled/generated Signal-LCC profile corrections;
- stable test-owned C7c CDI fixture and contract tests;
- corresponding owner/seam documentation.

Do not blindly cherry-pick all three commits. Reconstruct the cohesive correction against current `main`; some files and tests evolved across the commits.

Acceptance:

- extraction-tool tests pass;
- generated and bundled profile declarations agree;
- mixed-role Rule-to-Aspect leaves retain producer/consumer distinctions;
- full `bowties-core` tests pass.

### 3B — USB sleep/disconnect recovery

Source evidence:

- `d72499b`

Retain:

- connection-session ownership;
- bounded transport writer/health behavior;
- frontend session reconciliation;
- focused transport/session tests;
- governing ADR and aiwiki updates.

Exclude:

- all changes under `app/src-tauri/.svelte-kit/**`.

Acceptance:

- sleep/disconnect/reconnect behavior is covered by focused tests;
- `lcc-rs`, backend, and frontend lifecycle tests pass;
- generated files do not appear in the diff.

### 3C — Tower-LCC+Q profile support

Source evidence:

- `c08423e`
- `b7c339a`

Retain only after independent profile validation:

- Tower-LCC+Q profile;
- shared daughterboard metadata required by that profile;
- focused profile-loading/connector tests.

Exclude `app/package-lock.json` unless a deliberate dependency change in `app/package.json` requires regeneration.

Acceptance:

- captured or representative CDI builds the expected profile without warnings;
- daughterboard selection resolves expected connectors;
- no signaling-intent or ABS facility dependency exists.

## Phase 4 — Generic UX And Domain Salvage

Extract small, independently useful capabilities. Do not recreate the obsolete ABS user flow while salvaging generic pieces.

### 4A — `SingleSelectList`

Source evidence:

- `1df8aa3`
- the generic portion of `bf6c12e`

Retain the reusable accessible single-selection component and migrate an existing supported picker only when that migration is coherent by itself.

Do not restore the ABS logic-target selector.

### 4B — Channel configuration navigation

Source evidence:

- `610425f`

Retain role-neutral navigation from channel/facility presentation to the owning configuration section, with tests for supported binding shapes.

### 4C — Generic facility presentation

Source evidence:

- `57662f1`
- `1a7ddde`
- selected portions of `610425f`

Candidates:

- `SlotCard` declarative rendering;
- role-neutral FacilityCard layout improvements;
- reusable channel-card presentation.

Exclude:

- compiled-facility comprehension tied to Tower conditional lines;
- hard-coded Stop/Approach/Clear evaluation;
- downstream-Stop prediction;
- logic-target selection;
- ABS-specific status or allocation rendering.

### 4D — Generic delete/referrer behavior

Source evidence:

- selected portions of `726c7ed`

Retain only if it applies to supported facilities such as Block Indicator without requiring compiled ABS template types. Fix all retained type contracts rather than carrying branch-only `FacilityRecord` imports or stale fixtures.

### Acceptance for every Phase 4 change

- behavior is demoable with a currently supported facility or channel;
- no `abs-3-aspect-signal` identifier enters production code;
- no Tower-LCC conditional type is required;
- focused tests and full Vitest pass;
- the change adds no new `svelte-check` diagnostics over its touched surface.

## Phase 5 — Signal-Aspect Capability Without ABS Policy

Create a focused branch for target-independent signal display capability.

### Retain or reconstruct

- `signal-aspect` channel role;
- Signal-LCC-backed signal display styles;
- multi-row event resolution needed by those styles;
- observation-based aspect/lamp state derivation;
- profile and channel-style validation.

### Remove

- `ABS_3_ASPECT_SIGNAL` constant and registry entry;
- public ABS creation flow;
- ABS facility persistence;
- `LogicAllocation` persistence introduced only for the old implementation;
- Tower-LCC `logic_adapter` production module;
- Tower-specific `WiringPlan` production types;
- ABS-specific prediction in FacilityCard;
- current logic-target IPC and UI.

### Acceptance

- signal-aspect channels can be represented, bound to validated styles, navigated, and observed without an ABS facility;
- Block Indicator and all existing supported facilities remain unchanged;
- searches of production code find no obsolete ABS template ID or public label;
- all retained tests use target-independent terminology.

## Phase 6 — Build The Headless Signal-LCC Intent Pipeline

This phase is a distinct feature build on `main`, following the same one-item-at-a-time, explicit-commit workflow. Use [headless-intent-pipeline-plan.md](headless-intent-pipeline-plan.md) as the source plan.

This phase is new implementation, not extraction.

Risk-first sequence:

1. typed Signal Mast intent and bundled policy;
2. pure Signal-LCC plan preview;
3. validation against current CDI and effective configuration, including unknown Function values and broken group boundaries;
4. normal draft/application workflow;
5. generated reproduction of the proven single-mast bench configuration;
6. generated Track Circuit cascade acceptance;
7. signaling wizard as an adapter that produces the same intent.

Do not introduce a generalized multi-target planning framework until Signal-LCC proves which contracts are genuinely target-neutral.

## Repository Hygiene Gates

Apply these gates to every extracted change:

- no generated `app/src-tauri/.svelte-kit/**` diff;
- no unexplained `app/package-lock.json` diff;
- no session-handoff files;
- no stale active Spec 020 assertions;
- no production `abs-3-aspect-signal` registration;
- `git diff --check` passes;
- focused tests pass;
- full relevant Rust/Vitest suites pass;
- touched files introduce no new static diagnostics;
- aiwiki and durable product docs are updated for changed owners, flows, seams, or behavior.

## Why Not Cherry-Pick The Feature Commits Wholesale

The source commits mix concerns. Generic FacilityCard work is interleaved with ABS prediction; the USB fix includes generated SvelteKit output; profile work evolved across multiple commits; and layout format changes encode branch-only ABS persistence. Commit IDs identify evidence, not necessarily clean integration units.

Prefer behavior-level reconstruction when a source commit crosses owners.

## Execution Log

Fill this in during extraction so another session can resume safely.

| Phase | Source commit(s) | Landed commit on `main` | Result | Validation |
|---|---|---|---|---|
| 0 — hardware evidence | pending | source branch only | pending | pending |
| 0 — signaling plans | pending | source branch only | pending | pending |
| 1 — developer agents | `3046ff9` | pending | pending | pending |
| 2 — durable docs | Phase 0 commits | pending | pending | pending |
| 3A — Signal-LCC profile | `1139296`, `695cdbb`, `f880bc9` | pending | pending | pending |
| 3B — USB recovery | `d72499b` | pending | pending | pending |
| 3C — Tower-LCC+Q profile | `c08423e`, `b7c339a` | pending | pending | pending |
| 4A — selection list | `1df8aa3`, selected `bf6c12e` | pending | pending | pending |
| 4B — config navigation | `610425f` | pending | pending | pending |
| 4C — facility presentation | selected `57662f1`, `1a7ddde`, `610425f` | pending | pending | pending |
| 4D — delete/referrers | selected `726c7ed` | pending | pending | pending |
| 5 — signal-aspect capability | reconstructed | pending | pending | pending |
| 6 — headless pipeline | new implementation | pending | pending | pending |
