# AI Workflow Guide

How to work with AI agents on Bowties. You own behavior and design decisions; the AI owns architecture research, implementation, and knowledge-base maintenance.

---

## Part 1: How To Use It

### Pick the right starting point

| I need to... | Use |
|--------------|-----|
| Fix a bug | `/bugfix` prompt |
| Make a small change | `/quickchange` prompt |
| Build a feature | Feature pipeline (see below) |
| Assess architecture for a feature | `/design` skill |
| Generate vertical slices | `/slices` skill |
| Implement slices with TDD | `/build` skill |
| See available work | `/whatsnext` prompt |
| Review architecture | `/improve-codebase-architecture` skill |
| Stress-test a plan | `/grill-with-docs` skill |
| Prepare a release | `/release-prepare` then `/release-publish` skills |

### Bug fix or quick change flow

These two prompts follow the same user flow:

1. **Invoke the prompt and describe the problem/change.** The AI researches the codebase and comes back with an analysis and proposed approach.
2. **Review the analysis.** The AI stops here. You can:
   - Approve: say "go" or "looks good"
   - Redirect: "wrong module — it's in X" or "also consider Y"
   - Reject: "don't do that, instead..."
3. **AI implements.** It writes tests, makes the change, and runs affected tests.
4. **Review the result.** Look at the code changes and test output. If something is wrong, tell the AI to adjust.
5. **Commit.** Ask the AI to commit with a message, or do it yourself.
6. **Merge to main.** Squash-merge the branch into main.

For non-trivial changes that touched multiple modules, run `/feature-finish` before step 5 — it checks for documentation gaps and captures deferred work. Commit its updates to the branch so they're included in the squash.

### Feature flow

For larger work that needs planning:

1. **`/specify`** — describe the feature. AI produces a spec. Review and refine.
2. **`/clarify`** — AI identifies underspecified areas. You answer targeted questions.
3. **`/plan`** — AI produces an implementation plan. Review and approve.
4. **`/design`** — AI assesses architecture and validates the **slice set** (the roadmap) against placement rules and ADRs. It does *not* drill slices into tasks. Review findings and trade-offs.
5. **`/slices`** — AI generates a slice **roadmap**: one card per slice (intent + boundary + HITL/AFK + acceptance criteria, plus an architecture note for slices that shift the design). The per-layer task breakdown is *not* written yet. Review granularity, acceptance criteria, and labels.
6. **`/build`** — AI appends a task breakdown to **one slice at a time**, implements it with TDD, then re-reads the roadmap and adjusts the next slice's card before expanding it. May span multiple sessions.
7. **`/feature-finish`** — AI audits docs, enriches KB, captures deferrals. Does NOT commit.
8. **Commit.** Review and commit to the branch.
9. **Merge to main.** Squash-merge the branch into main.
10. **`/spec-close`** — AI archives spec, extracts ideas. (After merge.)

Always run `/feature-finish` before merging so its updates are part of the squash — not a separate commit on main.

You can use `/grill-with-docs` between steps 2-3 to stress-test the design against the glossary and past ADRs.

### Vertical slices and just-in-time tasking

Work is cut into **vertical slices** — each slice cuts through every layer it needs *and* yields something you can see, do, or verify. A slice that is "just the store", "just the types", "all the backend", or "all the tests" is **horizontal** and is rejected at `/design`/`/slices` time. "Testable" here means *user-demoable*, not merely *test-covered*.

`/slices` writes a **roadmap** of slice cards — each carries what you review: the slice's intent, the layers it cuts, the **acceptance criteria** (what you'll be able to test/demo), and — for slices that introduce a new pattern or seam — a short **architecture note**. What it deliberately leaves out is the pivot-fragile per-layer **task breakdown** (`store…`, `backend…`, `lcc-rs…`); `/build` appends that to one card at a time, just before implementing it, and re-cuts the next slice after each one finishes. This keeps your review surface intact — you can still judge each slice's purpose, architectural impact, and testability up front — while mid-feature pivots stay cheap: changing your mind edits a card plus the single slice in flight, not a whole pre-written task list. Expect the AI to pause at each slice boundary, show you what it learned, and adjust the next card before continuing.

### Multi-session builds

Larger features may require multiple `/build` sessions. The AI tracks progress in `specs/<feature>/slices.md`: each roadmap card carries a status (`sketched` → `tasked` → `done`) and tasks are checked off as they complete. At the start of each session, it reads the roadmap, finds the next slice that isn't `done`, and picks up where the previous session left off.

Slices are classified as **HITL** (needs your pattern-level input), **AFK** (AI handles autonomously), or **REFACTOR** (no user-visible change; preserves an invariant). HITL slices present the architectural pattern question before implementing. AFK and REFACTOR slices proceed directly.

The AI judges how many slices fit in a session based on complexity. It always stops at a slice boundary — never mid-slice — so every session ends with tests passing.

### The TDD coordinator (optional, for long slices)

The red→green→refactor loop generates a lot of failing-test output and abandoned attempts that fill the chat window fast. For a long or test-heavy slice, the AI can run the loop through the **`tdd-build` coordinator agent**, which delegates each phase to a context-isolated worker (`tdd-red` writes one failing test, `tdd-green` writes the minimal code to pass, `tdd-refactor` cleans up) and returns just a summary per cycle — so your main conversation stays lean.

Model selection for the coordinator follows a fast-first rule: start workers on a faster model for routine red/green cycles and escalate only if confidence is low, diagnostics conflict, or the refactor phase hits an ambiguous seam question.

The coordinator runs **strictly inside an already-designed, already-tasked slice** — it never re-decides architecture or re-cuts slices. Its Refactor worker is bound to `architecture-first-fix`: if cleanup reveals a deeper seam problem (wrong layer, ADR conflict, broken invariant), it stops and surfaces options to you instead of patching through. For a small slice it's often cheaper to just run the loop inline; the coordinator is there when context is the constraint.

### When context gets long

Start a new conversation. The AI re-orients via `aiwiki/` in seconds — all knowledge is in files, not conversation history.

### Deciding what to work on

Run `/whatsnext` to see all open work items from `specs/backlog.md` and `specs/ideas/`, grouped by functional area (features, bugs, documentation, tooling, etc.). Each item shows its source (`backlog` or `idea`) and which spec it originated from. Use this between tasks to pick your next piece of work.

### Quick reference: what to say

| Situation | Say |
|-----------|-----|
| Approve the plan | "go", "looks good", "proceed" |
| Adjust scope | "also fix X" or "don't touch Y" |
| AI went off-track | "stop" — then redirect |
| Ready to commit | "commit" or "please commit with message..." |
| Want doc cleanup before commit | "/feature-finish" |
| AI duplicated logic | Point to the shared convention in `aiwiki/owners.md` |
| AI suggests a rejected approach | Point to the ADR in `product/architecture/adr/` |

---

## Part 2: How It Works

### The analysis phase

Both `/bugfix` and `/quickchange` begin with a research phase. The AI uses subagents to keep your context window lean:

- Searches `specs/ideas/` for prior work on the area
- Reads `aiwiki/owners.md` to identify the owning module and its tests
- Checks for existing shared logic to avoid duplication
- Verifies placement rules via `product/architecture/code-placement-and-ownership.md`
- Scans `product/architecture/adr/` for decisions that constrain the approach

Model selection for this phase is also fast-first: use a faster model for retrieval/mapping work (find, list, summarize, ownership/location checks), then escalate only when the first pass leaves unresolved ambiguity.

The AI outputs a structured summary and then **stops** for your approval.

### The implementation phase

After approval, the AI:
- Writes a test first (TDD) — the test should fail before the fix/change
- Implements the narrowest change that makes the test pass
- Runs affected tests using the test mapping from `aiwiki/owners.md`

For features, the `/build` skill implements one vertical slice at a time using red-green-refactor. Each slice cuts through all necessary layers and is independently testable. See `/build` skill docs for the full TDD workflow.

### The enrichment phase

After implementation, the AI:
- Updates `aiwiki/owners.md` if it discovered undocumented modules or conventions
- Updates `aiwiki/flows.md` if workflow participation changed
- Updates `product/` docs if user-visible behavior changed
- Checks `specs/backlog.md` for items resolved or revealed

### The `/feature-finish` skill

This is a graduation audit — run it after implementation, before committing. It:
- Reviews the diff to understand what layers were modified
- Checks if `product/` docs need updates (glossary, architecture, behavior)
- Checks if `aiwiki/` is current for touched modules
- Captures deferred work as `specs/ideas/` files
- Updates `specs/backlog.md`
- Outputs a summary of what was updated and why
- Does NOT commit — you review first

### Knowledge base maintenance

The KB grows incrementally during normal work. No batch maintenance needed.

**What the AI maintains** (with your review):
- `aiwiki/owners.md` — module inventory, test mapping, shared conventions
- `aiwiki/flows.md` — workflow module participation
- `aiwiki/architecture-health.md` — coupling risks and architecture observations
- `product/architecture/adr/` — architecture decisions when load-bearing

**What you maintain**:
- `product/glossary.md` — canonical terms (AI proposes, you approve during grilling)
- `product/` behavior docs — updated when user-visible behavior changes
- `specs/backlog.md` — shared future-work ledger

**When to check KB health**:
- If an AI session struggles to find the right modules, `aiwiki/` may be stale
- Periodically review `aiwiki/architecture-health.md` for accumulated observations
- Check `last-verified` dates in `aiwiki/` entries — anything older than 30 days is worth spot-checking

---

## Part 3: Feature Lifecycle (End-to-End)

How a feature flows from idea to main, and where each tool fits.

```
Brainstorm ─► /specify ─► /clarify ─► /plan ─► /design ─► /slices ─► /build ─► /feature-finish ─► Merge
                │                        │          │          │          │
                │                        │          │          │          ├─ TDD per slice (red→green→refactor)
                │                        │          │          │          ├─ Tasks one slice at a time (just-in-time tasking)
                │                        │          │          │          ├─ Multi-session via slices.md
                │                        │          │          │          └─ aiwiki/ enrichment
                │                        │          │          │
                │                        │          │          ├─ HITL/AFK labels
                │                        │          │          └─ Slice roadmap (cards w/ acceptance criteria)
                │                        │          │
                │                        │          └─ Architecture assessment
                │                        │             Slice set (roadmap) validated
                │                        │             Depth/locality/seam evaluation
                │                        │
                │                        └─ /grill-with-docs (optional stress-test)
                │
                └─ Spec requirements captured
```

### Phase-by-phase

| Phase | What happens | Who drives | Artifacts |
|-------|-------------|------------|-----------|
| **Brainstorm** | Discuss the feature idea | You | Conversation only |
| **Specify** | `/specify` — AI creates spec | You describe, AI writes | `specs/NNN-feature/spec.md` |
| **Clarify** | `/clarify` — AI asks targeted questions | AI asks, you answer | Updated `spec.md` |
| **Plan** | `/plan` — AI creates implementation plan | AI proposes, you approve | `specs/NNN-feature/plan.md` |
| **Design** | `/design` — AI assesses architecture, validates the slice set (roadmap) | AI analyzes, you decide on trade-offs | Architecture Assessment in `plan.md` |
| **Slices** | `/slices` — AI generates the slice **roadmap** (one card per slice: intent, boundary, acceptance criteria, arch note) | AI proposes, you review granularity + acceptance criteria | `specs/NNN-feature/slices.md` |
| **Build** | `/build` — AI tasks and implements one slice at a time with TDD (multi-session) | AI codes, you review HITL slices | Code, tests, aiwiki/ enrichment |
| **Finish** | `/feature-finish` — AI audits docs and KB | AI proposes, you review | product/, aiwiki/, ideas updates |
| **Merge** | Squash-merge branch to main | You | Clean history on main |
| **Close spec** | `/spec-close` — AI archives spec, extracts ideas | AI proposes, you confirm | Spec moved to `specs/archive/`, ideas captured |

### For smaller work

| Work type | Flow |
|-----------|------|
| Bug fix | `/bugfix` → review analysis → approve → AI fixes → optional `/feature-finish` → merge |
| Quick change | `/quickchange` → review analysis → approve → AI implements → optional `/feature-finish` → merge |
| Feature | `/specify` → `/clarify` → `/plan` → `/design` → `/slices` → `/build` (multi-session) → `/feature-finish` → merge |

Run `/feature-finish` before merge whenever the change touched multiple modules or changed behavior. Skip it for trivial single-file fixes.

### Between features

Run `/whatsnext` to see all open work from `specs/backlog.md` and `specs/ideas/`, grouped by area.
