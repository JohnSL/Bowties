# AI Workflow Evolution Plan

**Date:** 2026-06-07
**Status:** Plan — agreed direction, not yet implemented
**Owner:** John (workflow owner) + AI (implementation)
**Scope:** Evolve the Bowties AI-assisted development workflow so it (a) resists quick-fix architectural decay, (b) survives context limits during TDD, and (c) stops relying on the human to *remember* to trigger bookkeeping steps.

This plan consolidates a multi-session design conversation. It records the decisions, the reasoning, and the concrete work to do — so any fresh session can pick it up without re-deriving the analysis.

---

## 1. The problems we are solving

1. **Quick fixes over good fixes.** Copilot tends to choose the cheapest local change, causing slow architectural decay across a sequence of locally-reasonable patches.
2. **TDD burns context fast.** Auto-compaction is *disabled* (it lost load-bearing detail), so work continues in fresh sessions. Red→green→refactor accumulates failing-test output, stack traces, and abandoned attempts that fill the window quickly.
3. **The workflow is a document I must remember to obey.** Knowledge-base enrichment (`aiwiki/`, ADRs, `specs/backlog.md`) and `feature-finish` are steps that get forgotten because nothing *triggers* them. This is the headline problem.
4. **Going straight to `/speckit.specify` loses context.** Cold-starting the spec phase produces weaker specs than first brainstorming a proposal.

---

## 2. Decisions and rationale (what we concluded)

### 2.1 Stay native; do not adopt an external multi-agent framework

- VS Code now provides the multi-agent mechanics natively: **custom agents** (`.agent.md`), **subagents** (context-isolated delegation), **handoffs**, and **hooks**.
- The "team of DRY/SOLID/YAGNI specialists" idea is **over-engineered**. Those principles are one *review lens*, not separable workflows. Keep them as a single architecture-review perspective.
- **Squad** (bradygaster/squad) was evaluated and **rejected**: it is alpha, it duplicates the knowledge layer we already have (`aiwiki/`, ADRs, memory) with a generic `.squad/` store, and it adds a maintenance surface. Its one genuinely unique capability (autonomous "Ralph" issue-polling) is a *different goal* than the one we have.
- The quick-fix problem is mostly a **gating/prompting** problem, not an agent-count problem. The existing `architecture-first-fix` skill is the sharper instrument and stays the primary lever.

### 2.2 Context model: outer loop = files, inner loop = subagents

- **Across sessions**, the "coordinator" is **not an agent** — it is **file state on disk** (`slices.md`, `spec.md`, proposals, this plan). A fresh session reads the files and resumes. This is already how multi-session builds work and it is correct.
- **Within one session**, a **coordinator agent + subagents** isolates context: each subagent works in its own window and returns only a summary, so the main window grows by one summary per unit of work instead of by full intermediate noise.
- **Handoffs are same-session.** They are the *wrong* tool for phase transitions where a fresh window is wanted. Phase transitions stay as **new sessions**, re-grounded via files. (Fresh-session discipline is explicitly endorsed by published best-practice.)

### 2.3 Enforcement: convert advisory rules into deterministic hooks

- The durable fix for "I forget enrichment" is a **Stop-hook gate**: when the agent tries to finish, a cheap deterministic check runs and **blocks** the turn if production code changed but `aiwiki/` / `product/` / `specs/backlog.md` did not — feeding the agent the reason so it completes the bookkeeping.
- This is stronger than instructions, which are advisory and are exactly the layer that has been failing. Published guidance is explicit: move "must happen every time" items out of instruction files and into hooks.
- **Caveats:** VS Code agent-scoped hooks are **Preview** (`chat.useCustomAgentHooks`); a Stop hook is overridden after ~8 consecutive blocks; must check `stop_hook_active` to avoid loops; the predicate must be cheap and decisive (a file-diff check, **not** an LLM re-litigating quality every turn).
- Back the hook with a **CI / pre-commit staleness check** as a safety net, since the hook is Preview.

### 2.4 SpecKit: eject the front-half we use, drop the back-half we reject

- We use **only** `/speckit.specify`, `/speckit.clarify`, `/speckit.plan`. We deliberately **reject** `tasks`/`implement` because they produce **opportunistic implementations** — speckit has **no architecture-assessment seam** between planning and coding.
- **`/design` is our architecture firewall** — the seam speckit lacks. This is *why* abandoning speckit's back-half was correct, not just a preference.
- **Format note:** the install is already on the current `.agent.md` format (9 `.agent.md` + 9 `.prompt.md` stubs in `.github/`). Skills mode exists but is **not** an upgrade and is **not** needed.
- **Decision: eject (Option B).** Keep `specify`/`clarify`/`plan` as hand-owned files; delete the unused command pairs; stop running `specify init` (which would re-add them and clobber the constitution). Rationale: we use 1/3 of speckit and have replaced its back-half; carrying the full upgrade machinery to get occasional prompt tweaks isn't worth the recurring re-deletion + constitution-restore tax. **Principle: Locality / YAGNI.**
- **Constitution stays regardless.** `.specify/memory/constitution.md` (real, ratified, ~380 lines) is consumed by `specify`/`clarify`/`plan` (`IF EXISTS`). Ejecting makes it safe from `--force` overwrites. Keep the file; the regenerating *command* can be dropped.

### 2.5 Add a formal pre-spec "proposal" phase

- Hand-writing a `proposal.md` (problem / why / concept / validation case / non-goals / open design questions / success criteria / pointers) **before** `/speckit.specify` produces crisper specs and preserves context.
- This is also the **front-of-pipeline handoff file** that cross-session continuity research said was missing — the analogue of `slices.md` for the *thinking* phase.
- It is a **thinking-gate** that protects `specify` from a half-baked start, exactly as `design` protects `plan`.

### 2.6 TDD coordinator: adopt, but bind it to the architecture gate

- **Adopt** the official VS Code TDD coordinator example + the awesome-copilot `tdd-red/green/refactor` worker agents (native `.agent.md`, drop-in).
- **Critical guardrail:** the green phase is "minimal code to pass" — opportunistic *by design*. So:
  - Wire the **Refactor** worker to the **`architecture-first-fix`** skill (not the generic refactor agent).
  - Run the coordinator **strictly downstream of `/design`**, implementing *within* an already-gated slice — it never re-decides architecture per task.
- Subagents must **not** become a back door that smuggles speckit-style opportunism back in.

### 2.7 Thin vertical slices + just-in-time tasking

Two related slicing disciplines, both motivated by a real failure observed in `specs/bug-fixes/page-svelte-decomposition.md` (S1–S6 were planned in detail up front, then re-cut three times — S1/S2 re-slice, S3 "Option 2", S4 "Option 3" — each forcing a documented markdown revision).

- **Vertical, not horizontal, slices.** Each slice must (a) cut through all needed layers and (b) yield a behavior the **user can exercise/observe** when done — not a horizontal layer ("just the store", "just the types", "all the backend") that cannot be demoed alone. `/design` and `/slices` reject horizontal slices at design time. "Testable" means *user-demoable*, not merely *test-covered*.
- **Just-in-time tasking (one slice deep).** Do **not** expand all slices to task detail up front — that authors detail that learnings from earlier slices may invalidate. Plan **direction broadly, tasks narrowly**: `/slices` emits a thin roadmap (ordered slices: title + one-line intent + rough boundary + HITL/AFK, each `status: sketched`); `/build` expands **only the next slice** into tasks just before implementing it (`status: tasked` → `done`), then **re-reads the roadmap and adjusts the next slice's boundary** in light of what was learned.
- **Pivots cost a sentence, not a rewrite.** Because downstream slices stay one-liners, a mid-feature pivot edits the roadmap line + the one slice in flight — not a task list. The per-slice re-cut becomes the *expected* checkpoint, not an exception. **Principle: YAGNI applied to planning + cheap-pivot.**
- **Architecture firewall stays at the roadmap level, not the task level.** `/design` validates the *slice set and their seams* (so whole-feature cycle/depth problems are still caught) but does **not** drill any slice into tasks. This preserves the firewall's whole-feature view while deferring the pivot-fragile detail.

---

## 3. The target pipeline

```
brainstorm
   │
   └─►  PROPOSAL  (new: proposal.md via `propose` skill)   ◄── front-of-pipeline handoff file
            │   (fresh session)
            ▼
        speckit:  specify → clarify → plan      (hand-owned 3-command subset; constitution feeds all three)
            │   (fresh session)
            ▼
        ══ ARCHITECTURE FIREWALL ══
            │
        yours:   design → slices → build → feature-finish → merge
                  │        │        │
                  │        │        ├─ TDD coordinator + Red/Green/Refactor subagents (context isolation)
                  │        │        ├─ Refactor bound to architecture-first-fix
                  │        │        ├─ expands ONLY the next sketched slice into tasks (just-in-time)
                  │        │        ├─ re-reads roadmap + adjusts next slice after each slice (cheap pivot)
                  │        │        ├─ multi-session via slices.md (file = coordinator)
                  │        │        └─ Stop-hook enrichment gate fires before "done"
                  │        └─ thin roadmap: slice titles + one-line intent only (status: sketched)
                  └─ validates the slice SET + seams (firewall at roadmap level, not task level)

        Slice rule: vertical (cuts all layers, user-demoable), never horizontal.
```

Cross-cutting, every session:
- **Stop-hook gate** blocks completion if code changed without doc/KB enrichment.
- **CI/pre-commit staleness check** as the safety net.
- **`architecture-first-fix`** remains the quality gate for every bugfix/behavior change and mid-slice surprise.
- **Subagent model routing is fast-first.** Use a faster model for retrieval/mapping work; escalate to a stronger model only for unresolved ambiguity, conflicting diagnostics, or high-impact architecture synthesis.
- Phase transitions are **fresh sessions**, re-grounded from files — not handoffs.

---

## 4. Off-the-shelf vs. build (research-verified, 2026-06-07)

| Pattern | Off-the-shelf status | Decision |
|---|---|---|
| TDD coordinator + workers | Official VS Code coordinator example + awesome-copilot `tdd-red/green/refactor.agent.md` (34.6k★, maintained) | **Adopt**, trim issue-centric bits, bind Refactor → `architecture-first-fix` |
| Cross-session continuity | spec-kit (110k★) exists but overlaps our slice model | **Skip** — we already have it via `slices.md` |
| Doc-enrichment Stop gate | **No published doc-DoD hook exists.** Mechanism fully documented; script templates exist (disler/claude-code-hooks-mastery `stop.py` 3.7k★; awesome-copilot `governance-audit` hook) | **Hand-roll** the ~40-line predicate from those templates |
| Proposal / brainstorm phase | awesome-copilot `prd` skill (`SKILL.md`, MIT) + obra/superpowers `brainstorming` skill (220k★) | **Cannibalize** their discovery/non-goals/measurable-success scaffolding; build our own thin `propose` skill tuned to emit a spec-kit-ready brief |

---

## 5. Work plan (ordered by leverage)

### Workstream A — Enrichment Stop-gate (highest value: kills "I forget")
- [x] A1. Cheap staleness-check predicate — `.github/hooks/enrichment-classify.ps1` (shared classifier; deterministic git file diff, no LLM). Code roots: `app/src/`, `app/src-tauri/src/`, `lcc-rs/`, `bowties-core/`; doc signals: `aiwiki/`, `product/`, `specs/backlog.md`; test files excluded.
- [x] A2. Wired as a **workspace-level Stop hook** (`.github/hooks/enrichment-gate.json` → `enrichment-gate.ps1`), **not** agent-scoped. Rationale: agent-scoped `.agent.md` hooks only fire under that custom agent; normal/default-agent coding (the actual "I forget" case) needs a workspace hook, which also avoids the `chat.useCustomAgentHooks` flag. `stop_hook_active` loop-guard implemented; fails open.
- [x] A3. Safety net = **git pre-push hook only** (`.githooks/pre-push` bash wrapper → `enrichment-prepush.ps1`), reusing the shared classifier. No CI workflow this pass. Honours `[kb-skip:reason]` / `[kb-required]` override tags. Setup: `git config core.hooksPath .githooks`.
- [x] A4. Instructions point at the gate: `.github/copilot-instructions.md` Post-Work Enrichment notes the deterministic gate; `docs/project/development.md` documents setup + overrides.

### Workstream B — TDD coordinator with architecture guardrail
- [x] B1. Worker agents authored as Bowties-native, GitHub-issue-centric steps stripped: `.github/agents/tdd-red.agent.md`, `tdd-green.agent.md`, `tdd-refactor.agent.md`. Adapted to the slice model + Bowties testing context (Vitest / cargo), with placement-rule discipline.
- [x] B2. Coordinator `.github/agents/tdd-build.agent.md` created with `agents: ['tdd-red','tdd-green','tdd-refactor']`; scoped to run **inside an already-tasked, already-designed slice** (preconditions block it otherwise) and explicitly never re-decides architecture or re-cuts slices.
- [x] B3. Refactor worker bound to `architecture-first-fix`: it stops and surfaces options (principle named) when cleanup reveals a wrong-layer/ADR-conflict/broken-invariant seam problem, instead of patching.
- [x] B4. Operates within `slices.md` multi-session tracking: coordinator checks off tasks, sets `status: done`, updates the status line, writes session notes, and hands back to `/build` to task the next slice. Wired into the `build` skill's TDD Loop section as an optional context-saving delegation.

### Workstream F — Thin vertical slices + just-in-time tasking
- [x] F1. Added an explicit **Vertical-Slice Gate** to `SLICING.md` (design skill) and referenced it from the `design` and `slices` SKILLs: a slice is valid only if it cuts all needed layers **and** yields a user-exercisable behavior; horizontal slices ("just the store", "all the backend", "all the tests") are rejected and folded forward. "Testable" defined as *user-demoable*, not merely *test-covered*.
- [x] F2. `/slices` now emits a **two-tier file**: Tier 1 roadmap (an overview table + one **slice card** each — title, one-line intent, layer boundary, HITL/AFK, blocked-by, **acceptance criteria**, and an **architecture note** for HITL/new-seam slices; `status: sketched`) and Tier 2 per-layer task breakdown authored one slice at a time. Acceptance criteria + arch note stay in the roadmap (the slice's reviewable contract + impact); only the pivot-fragile per-layer task list is deferred. `SLICE-FORMAT.md` rewritten with the card model, status lifecycle, and a two-tier example. `slices/SKILL.md` updated to author the cards.
- [x] F3. `/build` updated with a **Just-In-Time Tasking** section: expands only the next `sketched` slice into tasks at slice start, implements it, then re-reads the roadmap and adjusts the next slice's boundary before expanding it. Per-slice re-cut documented as an expected checkpoint in both "Just-In-Time Tasking" and "After Each Slice". Session startup reads roadmap status instead of "next unchecked slice".
- [x] F4. `/design`'s architecture validation kept at the **roadmap (slice-set) level** — step 4 explicitly states it validates the slice set + seams and never drills a slice into tasks.
- [x] F5. Both the `slices` and `build` skills updated (plus `design/SLICING.md`, `design/SKILL.md`, `slices/SLICE-FORMAT.md`); reflected in `docs/project/ai-workflow-guide.md` (vertical-slice + just-in-time tasking discipline, status lifecycle, plus the TDD coordinator from Workstream B).

### Workstream C — Proposal phase
- [ ] C1. Create `specs/templates/proposal.template.md`, modeled on `specs/014-config-modes-placeholders/proposal-original.md` (proven structure): Problem (+concrete cases) / Concept / Feature / Validation Case / Migration / Non-Goals / Open Design Questions (deferred to specify+plan) / Success Criteria / Pointers.
- [ ] C2. Create a `propose` **skill** (`.github/skills/propose/SKILL.md`): short discovery interview (seeded from awesome-copilot `prd` skill), section-by-section sign-off (superpowers `brainstorming`), enforce **measurable** success criteria + **explicit** non-goals, delegate codebase context to an `Explore` subagent, and **end by emitting a self-contained brief sized to paste into `/speckit.specify`**.
- [ ] C3. Keep it a **skill, not an agent** (phase transition = fresh session, not handoff).

### Workstream D — SpecKit eject
- [ ] D1. Read the bodies of `specify`/`clarify`/`plan` agents to confirm none invokes a to-be-deleted command via script (not just handoffs).
- [ ] D2. Delete both files (`.agent.md` + `.prompt.md`) for: `tasks`, `implement`, `analyze`, `checklist`, `taskstoissues` (and the `constitution` *command*, keeping the file).
- [ ] D3. Strip dangling `handoffs` from the kept agents (e.g. `speckit.plan` → `speckit.tasks`/`speckit.checklist`) so no button points at a deleted agent.
- [ ] D4. Keep `.specify/templates/{spec,plan}-template.md`, `.specify/memory/constitution.md`, and the scripts the kept agents call; remove now-unused templates (`tasks-template.md`, `checklist-template.md`).
- [ ] D5. Stop running `specify init`. Note in the workflow guide that speckit is now a **hand-owned 3-command subset**, not a managed install.

### Workstream E — Documentation
- [ ] E1. Update `docs/project/ai-workflow-guide.md` to add the **Proposal** phase, the SpecKit eject note, the **fresh-session-not-handoff** rule for phase transitions, and the **vertical-slice + just-in-time-tasking** discipline.
- [ ] E2. Note that enrichment is now **hook-enforced**, not memory-dependent.
- [ ] E3. Capture any deferred items in `specs/backlog.md` and (with confirmation) as `kind/idea` issues.
- [x] E4. Document subagent model routing defaults (fast-first with escalation criteria) in `.github/copilot-instructions.md`, `docs/project/ai-workflow-guide.md`, and the build/TDD execution docs.

---

## 6. Sequencing recommendation

1. **A (Stop-gate)** first — it directly solves the headline "I forget" problem and is small.
2. **C (Proposal phase)** next — low-risk, immediately useful, formalizes a habit already proven valuable.
3. **F (Vertical slices + just-in-time tasking)** — update `slices`/`build` skills before B, so the coordinator is built against the two-tier roadmap model.
4. **B (TDD coordinator)** — higher value but more moving parts; do after the gate (A) and the slice model (F) exist so it inherits both.
5. **D (SpecKit eject)** — mechanical; do when convenient, mind the handoff coupling.
6. **E (Docs)** — fold in as each workstream lands (and the Stop-gate will start enforcing this anyway).

---

## 7. Risks and guardrails

- **Hook Preview instability** — `chat.useCustomAgentHooks` may change; the CI/pre-commit twin (A3) is the durable fallback.
- **Stop-hook nag loops** — enforce `stop_hook_active` guard; keep the predicate a fast file diff.
- **TDD subagent opportunism** — mitigated by Refactor→`architecture-first-fix` and running downstream of `/design`.
- **SpecKit re-init regression** — ejecting + a guide note prevents a future `specify init` from undoing D2–D4.
- **Proposal becoming a parallel spec** — keep `propose` thin; its only job is to be high-quality *input to* `/speckit.specify`.
- **Roadmap drifting from reality** — the per-slice re-read in `/build` keeps the thin roadmap current; only the next slice is ever tasked, so drift is cheap to correct.
- **Over-engineering the tooling** — apply YAGNI to the workflow itself: no swarm of micro-agents, no Squad, no spec-kit re-adoption.

---

## 8. What we explicitly decided NOT to do

- Do **not** adopt Squad or any external agent-team framework.
- Do **not** build separate DRY / SOLID / YAGNI agents — one architecture-review lens.
- Do **not** use handoffs for phase transitions (they are same-session; we want fresh windows).
- Do **not** re-adopt spec-kit `tasks`/`implement` (the opportunism source).
- Do **not** adopt spec-kit for continuity (the slice model already covers it).
- Do **not** rely on instruction files for must-happen enrichment — use the deterministic hook.
- Do **not** plan all slices to task detail up front (Option A) — it authors pivot-fragile detail; task one slice at a time.
- Do **not** drop the slice roadmap entirely (Option C) — `/design` needs the whole slice set to validate seams.
- Do **not** accept horizontal slices — every slice must be vertical and user-demoable.
```
