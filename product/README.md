# Product Docs

## Purpose

`product/` is the durable source of truth for current Bowties behavior, workflows, architecture boundaries, and testing strategy.

Use these documents to understand how Bowties should behave now, where logic should live now, and which tests protect the current product contracts.

## What Belongs Here

Put these kinds of documents in `product/`:

- current user-visible behavior
- active workflows and lifecycle rules
- current architecture boundaries and ownership rules
- current testing strategy and regression-protection rules
- current engineering principles as applied to Bowties

Do not use `product/` for:

- feature-planning artifacts that only matter while a feature is being built
- speculative future design notes that are not yet implemented or adopted
- end-user help content
- project process or release workflow documentation

## Source Of Truth Model

- `product/` holds the durable current truth.
- `specs/` holds feature-scoped planning, tasks, contracts, and temporary build artifacts.
- `docs/user/` holds end-user documentation.
- `docs/project/` holds contributor, release, and process documentation.
- `docs/design/` and `docs/technical/` may still contain useful material, but they are not the default source of truth for current behavior unless a `product/` document explicitly points to them.

When current code, active tests, and `product/` disagree with older design or technical notes, prefer current code and `product/`.

## Key Documents

### Architecture

- `product/architecture/code-placement-and-ownership.md` — repo-wide placement rules for frontend, backend, and `lcc-rs`
- `product/architecture/frontend-boundaries.md` — current frontend ownership boundaries and high-risk seams
- `product/architecture/lifecycle-and-state-ownership.md` — owner of each lifecycle transition and state machine
- `product/architecture/naming-and-normalization.md` — Node ID normalization and display-name fallback rules
- `product/architecture/sync-panel-workflow.md` — sync session building, row classification, apply semantics

### User Experience

- `product/user-experience/bowties-model.md` — core event abstraction and bowtie card model

### Quality

- `product/quality/engineering-principles.md` — Bowties-specific application of SOLID, DRY, YAGNI, and TDD
- `product/quality/testing-strategy.md` — test-layer strategy and regression-protection rules

### User Stories

- `product/user-stories/README.md` — validated current user-story index
- `product/user-stories/connect-discover-read-configuration.md` — current online read-and-browse workflow
- `product/user-stories/bowties-viewing-and-editing.md` — current supported bowties workflow
- `product/user-stories/offline-capture-edit-sync.md` — current offline capture, edit, and sync workflow

### Planning And Governance

- `product/current-docs-and-regression-plan.md` — governance and documentation handoff plan
- `product/documentation-gap-and-reorganization-plan.md` — current backlog for filling product-doc gaps and reorganizing older docs

## How To Use These Docs

1. Start with the placement and ownership docs before deciding where new logic belongs.
2. Use the workflow and user-story docs to confirm current expected behavior.
3. Use the testing strategy to decide which seam should get a new or updated test.
4. Update the corresponding `product/` doc whenever intentional behavior or ownership rules change.

## Maintenance Rules

- Keep documents concise, current, and behavioral.
- Name the owning layer for each workflow or rule.
- Prefer current contracts and acceptance rules over implementation history.
- When a regression exposes a missing contract, add that contract here once the intended behavior is clear.