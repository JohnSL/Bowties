# Bowties Copilot Instructions

These instructions are the always-on implementation contract for Bowties.

## Source Of Truth

- Treat `product/` as the durable source of truth for current product behavior, workflows, architecture boundaries, and testing strategy once that folder exists.
- Treat `product/architecture/adr/` as the record of accepted and rejected architecture decisions. Check before proposing an approach that may have been previously evaluated.
- Treat `aiwiki/` as the AI-audience code-level navigation layer (WHERE things live, HOW they connect). It supplements `product/` (WHAT the product does, WHY).
- Treat `docs/user/**` as end-user documentation and `docs/project/**` as developer-process documentation, not as the canonical source for current architecture.
- Treat `specs/**` as feature-scoped planning and build artifacts. Treat `specs/archive/**` as historical only.
- Treat open GitHub issues labeled `kind/idea` as the prior-work cache for deferred and not-yet-scoped ideas. Search them by `area/*` labels before starting new work (`gh issue list --repo JohnSL/Bowties --label kind/idea --state open` or the GitHub MCP tools). Closed `kind/idea` issues are historical (adopted into a spec or rejected).
- Any residual files under `specs/ideas/**` are legacy pending migration to issues — read them if directly relevant, but new ideas go to issues, not files.
- If older design or technical notes conflict with current code, active non-archived specs, or the durable product docs, the durable product docs and current code win.
- Precedence: `product/` + current code > `aiwiki/` > `specs/` > older docs.

## Engineering Defaults

- Apply SOLID as concrete ownership rules: keep modules focused, prefer one clear owner for each workflow, and avoid mixing UI rendering, workflow sequencing, state mutation, backend domain logic, and protocol logic in the same file.
- Apply DRY by reusing shared helpers for normalization, fallback rules, formatting, and translation logic instead of creating local variants.
- Apply YAGNI by preferring the smallest explicit abstraction that solves the current problem. Do not add generic frameworks or speculative layers without multiple real call sites.
- Apply TDD for production behavior changes: add or update a focused test around the behavior seam first when practical, then implement the smallest change that makes it pass.
- When fixing a regression, encode the regression as a behavior contract in tests and update the durable product docs if the user-visible behavior or ownership rule is part of the fix.

## Context Conservation

Skills like `design`, `build`, and `architecture-first-fix` require reading many canonical files (aiwiki, ADRs, placement rules, glossary, GitHub issues). Gathering all of that in the main conversation burns context tokens that are better spent on decisions and implementation.

- **Delegate read-heavy context gathering to subagents.** When a skill step requires reading 3+ canonical files or searching GitHub issues, use an `Explore` subagent to fetch and summarize the results. Work from the subagent's structured summary in the main conversation.
- **Route subagent work by complexity.** Default to a faster model for retrieval and mapping tasks (search, ownership scans, file triage, status summaries). Escalate to a stronger model only for ambiguous root-cause analysis, cross-ADR trade-offs, or high-impact design synthesis. Prefer escalation after a fast first pass, not heavyweight-by-default.
- **Cache build checks per session.** On the first slice of a build session, run pre-implementation checks via subagent and store results in session memory (`/memories/session/`). Reference that cache for subsequent slices instead of re-running the same searches.
- **Deduplicate GitHub issue searches.** Search `kind/idea` issues once per session (or once per skill invocation), not once per step that mentions them. Reuse the results across steps.
- **Keep decision points in-band.** Do NOT delegate user-facing presentations to subagents: option presentation (architecture-first-fix), finding review (design), HITL slice decisions (build), and TDD implementation loops must stay in the main conversation where the user can interact.

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
- After finishing a code change, check `specs/backlog.md` and update it when the work resolves, changes, or newly reveals a backlog item. Keep that file current as the shared future-work ledger. Each entry should describe actionable remaining work — not a history of what was already done. Remove entries entirely when all their work is complete. When a spec exists for an item, the spec is the plan and the item should not appear in the backlog.
- If a change touches a risky seam such as lifecycle ownership, Node ID normalization, naming fallback, or sync-trigger behavior, keep the change narrow and validate it with the closest focused test.
- When touching a file, scan for dead code: unused functions, unreachable branches, stale imports, and commented-out blocks. Remove them as part of the change rather than leaving them to accumulate.
- When touching a file, look for adjacent consistency issues: missing pattern conformance (e.g. a callback missing from a reset function that sibling functions include), inconsistent naming, or shallow fixes that leave a deeper problem unaddressed. Fix them as part of the change when the improvement is narrow and testable.

## Copilot Knowledge Base

Read-order for orientation on an unfamiliar area:

1. `aiwiki/owners.md` summary section — which layer owns what, key shared logic pointers.
2. Drill into the relevant `aiwiki/owners.md` layer section — module purposes, test files, shared conventions.
3. `aiwiki/flows.md` — which modules participate in the workflow you are touching.
4. `product/architecture/code-placement-and-ownership.md` — placement rules for new logic.
5. `product/glossary.md` — canonical terminology and avoid-lists.
6. `product/architecture/adr/` — past architecture decisions and rejected approaches.
7. Open GitHub issues labeled `kind/idea` (filter by `area/*`) — prior analysis and deferred work. Legacy: any residual `specs/ideas/**` files.

Enrich `aiwiki/` as you work: add modules, conventions, flows, or architecture observations you discover that are not yet documented. The knowledge base grows incrementally during feature work, not in batch passes.

## Pre-Implementation Checks

Before implementing a change, verify:

1. Check `aiwiki/owners.md`: does this logic already exist? Which layer owns it?
2. Check shared conventions in `aiwiki/owners.md`: is there already a pattern for this?
3. Check `product/architecture/code-placement-and-ownership.md`: is this the right layer?
4. Check `product/architecture/adr/`: has this approach been evaluated or rejected before?
5. Check open GitHub issues labeled `kind/idea` filtered by the relevant `area/*` labels: is there prior analysis for this area? (Also glance at any residual `specs/ideas/**` files until migration completes.)
6. Identify affected tests from `aiwiki/owners.md` test mapping.
7. If adding shared logic, update `aiwiki/owners.md` so the next session finds it.
8. Prefer refactoring for depth over expedient shortcuts that create shallow modules.

## Architecture-First Default

For every bugfix or behavior change — whether triggered by a slash command (`/bugfix`, `/quickchange`, etc.) or a freeform chat request — load and follow the `architecture-first-fix` skill before editing code. The skill defines the procedure: identify the seam, distinguish symptom from root cause, present two or more options at the root cause with the principle at stake named explicitly (DRY / SOLID / YAGNI / Depth / Locality / ADR-compliance), and wait for the user to choose.

Exempt: trivial mechanical edits (typo, comment, import sort, formatting), and cases where the user has explicitly said "just patch it", "skip the architecture check", or equivalent.

The purpose of this rule is to prevent the slow architectural decay that results from a sequence of locally-reasonable patches at symptom sites. Treat "the cheapest local change" as a red flag, not a default.

## Post-Work Enrichment

After completing a change:

- Update `aiwiki/owners.md` for any modules, conventions, or test files you added or changed.
- Update `aiwiki/flows.md` if the change affects workflow module participation.
- Update `aiwiki/seams.md` if you added a Contributor or Consumer to a documented seam, introduced a new aggregate / single-source pattern, or audited an existing entry (bump its `Last-modified` on edits; bump `Last-audited` only after a full Owner/Contributors/Consumers re-grep).
- Note architecture risks or coupling observations in `aiwiki/architecture-health.md`.
- Prefer extending the relevant seam-scoped ADR in `product/architecture/adr/` with a dated section (`## YYYY-MM-DD extension: <short title>`). Write a new ADR only when the work introduces a genuinely new seam, reverses an existing commitment (mark the old ADR `superseded by ADR-NNNN`), or crosses a boundary the existing ADR explicitly excluded. See `product/architecture/adr/README.md`.
- If you discover a module, convention, or flow not listed in `aiwiki/`, add it before completing the change.

This step is enforced by a deterministic enrichment gate, not just this instruction: a VS Code Stop hook (`.github/hooks/enrichment-gate.json`) blocks the turn from finishing when production source changed in the working tree but no `aiwiki/`, `product/`, or `specs/backlog.md` file did, and a git pre-push hook (`.githooks/pre-push`) is the safety net. Both share `.github/hooks/enrichment-classify.ps1`. If the gate blocks and no enrichment genuinely applies, document why and finish; do not bypass it as a reflex. See `docs/project/development.md` for setup and override tags.

## Issue Capture Protocol

Deferred ideas, follow-up work, and out-of-scope improvements are captured as GitHub issues, not as files in the repo.

- **Never create a GitHub issue without explicit user confirmation.** When you identify something worth capturing, **propose** the issue: state the title, the labels you would apply (`kind/*` plus relevant `area/*`), and a draft body. Then stop and wait for the user to say "create it" (or equivalent).
- The same rule applies to issue edits: propose label changes, status changes, comments, and closures; do not perform them unprompted.
- Use the GitHub MCP tools or `gh issue create` once the user approves. The repo is `JohnSL/Bowties`.
- Follow the label taxonomy: one `kind/*`, one or more `area/*`, and a `status/*` only when status has actually been triaged (do not pre-apply `status/deferred` to fresh issues).
- Bugs use the `bug` label (not `kind/bug`). Use the bug issue template fields when proposing.