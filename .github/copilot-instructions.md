# Bowties Copilot Instructions

These instructions are the always-on implementation contract for Bowties.

## Source Of Truth

- Treat `product/` as the durable source of truth for current product behavior, workflows, architecture boundaries, and testing strategy once that folder exists.
- Treat `product/architecture/adr/` as the record of accepted and rejected architecture decisions. Check before proposing an approach that may have been previously evaluated.
- Treat `aiwiki/` as the AI-audience code-level navigation layer (WHERE things live, HOW they connect). It supplements `product/` (WHAT the product does, WHY).
- Treat `docs/user/**` as end-user documentation and `docs/project/**` as developer-process documentation, not as the canonical source for current architecture.
- Treat `specs/**` as feature-scoped planning and build artifacts. Treat `specs/archive/**` as historical only.
- Treat `specs/ideas/` as structured prior-work cache with area tags. Check for relevant prior analysis before starting new work.
- If older design or technical notes conflict with current code, active non-archived specs, or the durable product docs, the durable product docs and current code win.
- Precedence: `product/` + current code > `aiwiki/` > `specs/` > older docs.

## Engineering Defaults

- Apply SOLID as concrete ownership rules: keep modules focused, prefer one clear owner for each workflow, and avoid mixing UI rendering, workflow sequencing, state mutation, backend domain logic, and protocol logic in the same file.
- Apply DRY by reusing shared helpers for normalization, fallback rules, formatting, and translation logic instead of creating local variants.
- Apply YAGNI by preferring the smallest explicit abstraction that solves the current problem. Do not add generic frameworks or speculative layers without multiple real call sites.
- Apply TDD for production behavior changes: add or update a focused test around the behavior seam first when practical, then implement the smallest change that makes it pass.
- When fixing a regression, encode the regression as a behavior contract in tests and update the durable product docs if the user-visible behavior or ownership rule is part of the fix.

## Where Logic Goes

- Use `product/architecture/code-placement-and-ownership.md` as the durable decision rule for where new logic belongs.
- Put screen composition and visible page state in `app/src/routes/**`.
- Put rendering, emitted intent, and minimal display-only derivation in `app/src/lib/components/**`.
- Put multi-step async workflows, lifecycle transitions, and cross-store coordination in `app/src/lib/orchestration/**`.
- Put durable frontend state and deterministic transitions in `app/src/lib/stores/**`.
- Put pure normalization, formatting, comparison, and translation helpers in `app/src/lib/utils/**`.
- Put IPC boundaries, authoritative app state, file/layout persistence, and backend workflow coordination in `app/src-tauri/src/**`.
- Put reusable LCC/OpenLCB protocol behavior, transport rules, and wire-level parsing or encoding in `lcc-rs/**`.
- If a rule would matter to other LCC/OpenLCB consumers, prefer `lcc-rs`. If it exists only because of Bowties UI or app workflow, keep it out of `lcc-rs`.

## Frontend Boundaries

- In `app/src/routes/**`, routes compose screens, coordinate visible page state, and delegate multi-step workflows.
- In `app/src/lib/components/**`, components render state and emit intent. Keep async sequencing and lifecycle-sensitive workflows out of components.
- In `app/src/lib/orchestration/**`, orchestrators own multi-step async workflows, lifecycle transitions, and cross-store coordination.
- In `app/src/lib/stores/**`, stores own durable frontend state and deterministic transitions.
- In `app/src/lib/utils/**`, shared helpers own normalization, formatting, and reusable translation logic.

## Backend Boundaries

- In `app/src-tauri/src/**`, command modules own IPC boundaries and error translation. Deeper backend modules own workflow and state coordination.
- Keep backend domain logic independent of presentation concerns. Do not shape backend logic around incidental UI structure when a domain-oriented design is clearer.
- Keep protocol-specific behavior in `lcc-rs` or focused backend adapters instead of scattering protocol rules across app code.

## Protocol Library Boundaries

- In `lcc-rs/**`, prioritize protocol correctness, transport clarity, public API stability, and test coverage over app-specific convenience shortcuts.
- Avoid leaking Bowties UI or app workflow assumptions into the protocol library.
- When implementing LCC/OpenLCB protocol behavior, consult `OpenLCB_Java/` and `JMRI/` in this workspace as reference implementations for expected protocol behavior and usage patterns.

## Change Discipline

- Prefer existing shared helpers, stores, orchestrators, and backend services over adding parallel variants.
- Name the owner of each new workflow or lifecycle transition explicitly in code and tests.
- Update tests and the durable product docs together when intentional behavior changes land.
- After finishing a code change, check `specs/backlog.md` and update it when the work resolves, changes, or newly reveals a backlog item. Keep that file current as the shared future-work ledger.
- If a change touches a risky seam such as lifecycle ownership, Node ID normalization, naming fallback, or sync-trigger behavior, keep the change narrow and validate it with the closest focused test.

## Copilot Knowledge Base

Read-order for orientation on an unfamiliar area:

1. `aiwiki/owners.md` summary section — which layer owns what, key shared logic pointers.
2. Drill into the relevant `aiwiki/owners.md` layer section — module purposes, test files, shared conventions.
3. `aiwiki/flows.md` — which modules participate in the workflow you are touching.
4. `product/architecture/code-placement-and-ownership.md` — placement rules for new logic.
5. `product/glossary.md` — canonical terminology and avoid-lists.
6. `product/architecture/adr/` — past architecture decisions and rejected approaches.
7. `specs/ideas/` — prior analysis and deferred work tagged by area.

Enrich `aiwiki/` as you work: add modules, conventions, flows, or architecture observations you discover that are not yet documented. The knowledge base grows incrementally during feature work, not in batch passes.

## Pre-Implementation Checks

Before implementing a change, verify:

1. Check `aiwiki/owners.md`: does this logic already exist? Which layer owns it?
2. Check shared conventions in `aiwiki/owners.md`: is there already a pattern for this?
3. Check `product/architecture/code-placement-and-ownership.md`: is this the right layer?
4. Check `product/architecture/adr/`: has this approach been evaluated or rejected before?
5. Check `specs/ideas/`: is there prior analysis for this area?
6. Identify affected tests from `aiwiki/owners.md` test mapping.
7. If adding shared logic, update `aiwiki/owners.md` so the next session finds it.
8. Prefer refactoring for depth over expedient shortcuts that create shallow modules.

## Post-Work Enrichment

After completing a change:

- Update `aiwiki/owners.md` for any modules, conventions, or test files you added or changed.
- Update `aiwiki/flows.md` if the change affects workflow module participation.
- Note architecture risks or coupling observations in `aiwiki/architecture-health.md`.
- Write an ADR in `product/architecture/adr/` when an architecture decision was made or an approach was rejected for load-bearing reasons.
- If you discover a module, convention, or flow not listed in `aiwiki/`, add it before completing the change.