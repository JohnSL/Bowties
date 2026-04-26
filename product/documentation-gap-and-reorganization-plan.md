# Documentation Gap And Reorganization Plan

## Purpose

This plan defines the next documentation pass for Bowties.

It covers two related tasks:

- adding the durable product documentation that is still missing from `product/`
- reorganizing and updating older documents in `docs/` and `specs/` so the source-of-truth model stays clear

This plan is based on the current state of `product/`, the active non-archived specs, and the behavior that appears to be implemented in the frontend, backend, and `lcc-rs`.

## Current State

### What Exists In `product/`

The full initial documentation spine is now in place:

- `product/README.md`
- `product/current-docs-and-regression-plan.md`
- `product/documentation-gap-and-reorganization-plan.md`
- `product/architecture/code-placement-and-ownership.md`
- `product/architecture/frontend-boundaries.md`
- `product/architecture/lifecycle-and-state-ownership.md`
- `product/architecture/naming-and-normalization.md`
- `product/architecture/sync-panel-workflow.md`
- `product/quality/engineering-principles.md`
- `product/quality/testing-strategy.md`
- `product/user-experience/bowties-model.md`
- `product/user-stories/README.md`
- `product/user-stories/connect-discover-read-configuration.md`
- `product/user-stories/bowties-viewing-and-editing.md`
- `product/user-stories/offline-capture-edit-sync.md`

All `docs/design/**` files now carry status classification notes (historical, migrated, active design input, or future design input).

### Main Gap

The repository now has Copilot instructions that depend on `product/` as the durable current source of truth, but `product/` does not yet contain enough current behavior and architecture documentation to support that role consistently.

## Planning Rules

- Put durable current behavior, workflows, architecture boundaries, and testing strategy in `product/`.
- Keep `docs/user/**` for end-user documentation.
- Keep `docs/project/**` for project-process, release, and development workflow documentation.
- Keep active feature planning and contracts in `specs/**` while a feature is being built.
- Graduate implemented and durable behavior from `specs/**` into `product/**`.
- Do not promote speculative design notes into `product/` until the design is implemented or explicitly adopted as current architecture.
- When migrating a document, prefer rewriting it into current-state form over copying historical planning prose.

## Missing Product Documentation Backlog

### Tier 1: Foundation Documents

These should be created first because they define the product-level entry points and the highest-value current behavior.

1. `product/README.md`
   - Purpose: index the product docs and explain the source-of-truth model.
   - Sources: `product/current-docs-and-regression-plan.md`, `.github/copilot-instructions.md`.

2. `product/quality/testing-strategy.md`
   - Purpose: document test layers, when to add tests, and how behavior-contract tests protect risky seams.
   - Sources: `product/current-docs-and-regression-plan.md`, `specs/010-offline-layout-editing/regression-test-plan.md`, current Vitest and Rust test layout.

3. `product/quality/engineering-principles.md`
   - Purpose: turn SOLID, DRY, YAGNI, and TDD into Bowties-specific rules.
   - Sources: `product/current-docs-and-regression-plan.md`, `.github/copilot-instructions.md`, scoped instruction files under `.github/instructions/`.

4. `product/architecture/frontend-boundaries.md`
   - Purpose: capture the current frontend ownership model in more detail than the placement matrix.
   - Sources: `docs/technical/architecture.md`, `product/architecture/code-placement-and-ownership.md`, current route/orchestrator/store/component organization.

### Tier 2: Core User And Workflow Documents

These should capture implemented workflows and the remaining deeper behavior descriptions that still live mostly in specs or older design docs.

The current user-story layer for the highest-value implemented stories now exists under `product/user-stories/**`. The items below remain useful as richer workflow or model docs rather than as the first missing user-story pass.

5. `product/user-experience/bowties-model.md`
   - Purpose: document the Bowties mental model, event abstraction, and why the UI is organized around bowties and roles.
   - Sources: `docs/design/design-bowtieMvp.md`, current bowtie catalog and metadata implementation.

6. `product/workflows/connect-discover-read-config.md`
   - Purpose: document the main online workflow from connect to discovery to configuration reading.
   - Sources: `docs/design/workflows.md`, `docs/technical/architecture.md`, specs `004-read-node-config`, `005-config-sidebar-view`, `006-bowties-event-discovery`, current orchestrators and stores.

7. `product/workflows/offline-capture-edit-sync.md`
   - Purpose: document offline capture, local editing, reconnect, sync conflict handling, and apply semantics.
   - Sources: `specs/010-offline-layout-editing/spec.md`, `specs/010-offline-layout-editing/plan.md`, `specs/010-offline-layout-editing/contracts/tauri-ipc.md`, current offline and sync orchestrators/stores/backend layout modules.

8. `product/user-stories/offline-sync-workflows.md`
   - Purpose: capture a more focused offline/sync user-story contract if the current broader `offline-capture-edit-sync.md` needs to be split later.
   - Sources: `specs/010-offline-layout-editing/spec.md`, current route/orchestrator behavior, regression plan.

### Tier 3: Architecture And Behavior Seams

These should document the current high-risk areas that have already produced regressions.

9. `product/architecture/lifecycle-and-state-ownership.md`
   - Purpose: name the owner of major lifecycle transitions and state transitions.
   - Sources: `specs/010-offline-layout-editing/refactoring-roadmap.md`, current lifecycle helpers, sync and layout orchestrators, route lifecycle behavior.

10. `product/architecture/naming-and-normalization.md`
   - Purpose: document Node ID normalization, display-name fallback, alias handling, and other canonical comparison rules.
   - Sources: current frontend utils/stores/orchestrators, `specs/010-offline-layout-editing`, regression fixes, current backend normalization patterns.

11. `product/architecture/sync-panel-workflow.md`
   - Purpose: document sync-session building, conflict classification, partial apply behavior, and dismiss/reopen rules.
   - Sources: `specs/010-offline-layout-editing/spec.md`, sync panel commands, sync orchestrators, regression plan.

### Tier 4: Guided Configuration And Profile System

These should be created once the team wants the profile system documented as current product architecture rather than only spec work.

12. `product/architecture/node-profile-system.md`
   - Purpose: document profile loading, role resolution, relevance rules, and how guided configuration depends on profiles.
   - Sources: `specs/008-guided-configuration/plan.md`, profile loader/resolver modules, profile-related skills.

13. `product/workflows/guided-configuration.md`
   - Purpose: document the intended product workflow for guided configuration once the implementation is active enough to treat as current behavior.
   - Sources: `specs/008-guided-configuration/**`.

## Reorganization And Update Plan For Existing Docs

### `docs/user/**`

Keep under `docs/user/**`.

Actions:

- review for terminology consistency after `product/` becomes the durable internal documentation area
- add links to relevant product docs only when useful for maintainers, not end users

### `docs/project/**`

Keep under `docs/project/**`.

Actions:

- review development and release docs for any references to old architecture sources
- update references so contributors know that current behavior and architecture live in `product/`

### `docs/design/**`

Split this directory into three categories.

1. Migrate into `product/` after rewriting into current-state form:
   - `docs/design/design-bowtieMvp.md`
   - `docs/design/workflows.md`

2. Keep as active design notes until implemented or adopted:
   - `docs/design/node-proxy-plan.md`
   - `docs/design/actor-transport-refactor.md`
   - other clearly future-looking design notes

3. Archive or clearly mark historical if superseded:
   - early design explorations that no longer define current behavior

Actions:

- add a short status note at the top of each retained design doc stating whether it is current design input, future design, or historical material
- remove implicit source-of-truth status from design docs that have been superseded by `product/`

### `docs/technical/**`

Split by document type.

1. Migrate durable behavior and architecture content into `product/`:
   - `docs/technical/architecture.md`
   - parts of `docs/technical/protocol-reference.md` that explain product-facing concepts

2. Keep implementation reference docs in `docs/technical/**` or move them closer to their owning code:
   - `docs/technical/tauri-api.md`
   - `docs/technical/lcc-rs-api.md`
   - `docs/technical/cdi-support.md`

3. Update retained technical docs so they clearly state whether they are current implementation reference, backend contract, or library reference.

Actions:

- decide whether `docs/technical/lcc-rs-api.md` should move to `lcc-rs/API.md`
- update `docs/technical/architecture.md` after migrating its durable parts into `product/`
- add links from retained technical docs to the corresponding durable product docs

### `specs/**`

Keep feature planning, contracts, and build artifacts in `specs/**`.

Actions:

- for each active feature that has reached durable implemented behavior, extract the current behavior into `product/`
- leave feature-planning rationale, implementation steps, and temporary acceptance scaffolding in `specs/**`
- do not copy speculative future-state language into `product/`

## Source-To-Target Migration Map

| Current source | Target | Action |
|---|---|---|
| `docs/design/design-bowtieMvp.md` | `product/user-experience/bowties-model.md` | Rewrite into current product model doc |
| `docs/design/workflows.md` | `product/workflows/connect-discover-read-config.md` and related workflow docs | Split and rewrite by workflow |
| `docs/technical/architecture.md` | `product/architecture/frontend-boundaries.md` and related architecture docs | Extract current ownership rules and workflow model |
| `specs/010-offline-layout-editing/spec.md` | `product/workflows/offline-capture-edit-sync.md` | Distill durable current behavior |
| `specs/010-offline-layout-editing/regression-test-plan.md` | `product/quality/testing-strategy.md` and architecture seam docs | Extract durable regression-protection rules |
| `specs/008-guided-configuration/plan.md` | `product/architecture/node-profile-system.md` and future workflow docs | Promote only when behavior is current enough |
| `docs/technical/protocol-reference.md` | `product/` plus retained technical reference | Split product-facing concepts from low-level reference |
| `docs/technical/lcc-rs-api.md` | `lcc-rs/API.md` or retained technical reference | Re-home with owning library if maintained |

## Execution Phases

### Phase 1: Product Spine

1. Keep `product/README.md` current as the product-doc entry point.
2. Keep `product/quality/testing-strategy.md` current as the durable test-strategy document.
3. Keep `product/quality/engineering-principles.md` current as the Bowties engineering-rules document.
4. Keep `product/architecture/frontend-boundaries.md` current as the frontend ownership document.

### Phase 2: Highest-Value Current Workflows

5. Keep the validated `product/user-stories/**` set current as implemented behavior changes.
6. Create `product/workflows/connect-discover-read-config.md` if a deeper workflow document is needed beyond `product/user-stories/connect-discover-read-configuration.md`.
7. Create `product/workflows/offline-capture-edit-sync.md` only if a deeper workflow document is needed beyond `product/user-stories/offline-capture-edit-sync.md`.
8. Create `product/user-experience/bowties-model.md` as the product mental-model document behind the current bowties stories.

### Phase 3: High-Risk Architecture Seams

9. ~~Create `product/architecture/lifecycle-and-state-ownership.md`.~~ Done.
10. ~~Create `product/architecture/naming-and-normalization.md`.~~ Done.
11. ~~Create `product/architecture/sync-panel-workflow.md`.~~ Done.

Also created: `product/user-experience/bowties-model.md` (Tier 2, item 5).

### Phase 4: Reorganize Existing Docs

12. ~~Review `docs/design/**` and classify each file as migrate, keep-as-design-input, or historical.~~ Done — all nine `docs/design/**` files now carry a status classification note.
13. Review `docs/technical/**` and classify each file as migrate, retain-as-reference, or move closer to owning code.
14. Update `docs/project/**` to point contributors to `product/` as the source of truth for current behavior and architecture.

### Phase 5: Future Product Areas

15. Promote guided-configuration and profile-system docs into `product/` only when the implementation is current enough to act as durable product truth.

## Verification

The plan is complete when:

- `product/` contains current docs for the major implemented workflows and architecture seams
- no critical current behavior lives only in a feature spec
- design and technical docs clearly state whether they are current durable truth, implementation reference, active design input, or historical
- contributor-facing project docs point to `product/` instead of older mixed documentation areas
- Copilot instruction files can rely on `product/` without depending on stale specs or ambiguous design notes

## Review Questions

Use these questions when executing the plan:

1. Is this document describing durable current truth, or only feature planning and design rationale?
2. If it is current truth, should it live in `product/`?
3. If it is implementation reference rather than product behavior, should it stay in `docs/technical/` or move closer to the owning code?
4. If it is future design, should it remain in `docs/design/` until implementation exists?
5. When migrating a document, are the current code and active tests being used to confirm the behavior before it is declared durable?