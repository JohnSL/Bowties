# AI Workflow Guide

How to work with AI agents on Bowties. You own behavior and design decisions; the AI owns architecture research, implementation, and knowledge-base maintenance.

---

## Part 1: How To Use It

### Pick the right starting point

| I need to... | Use |
|--------------|-----|
| Fix a bug | `/bugfix` prompt |
| Make a small change | `/quickchange` prompt |
| Build a feature | SpecKit pipeline (see below) |
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

### Feature flow (SpecKit pipeline)

For larger work that needs planning:

1. **`/specify`** — describe the feature. AI produces a spec. Review and refine.
2. **`/plan`** — AI produces an implementation plan. Review and approve.
3. **`/tasks`** — AI breaks the plan into ordered tasks. Review task list.
4. **`/implement`** — AI works through tasks, stopping for approval at key points.
5. **`/feature-finish`** — AI audits docs, enriches the knowledge base, captures deferrals. Does NOT commit.
6. **Commit.** Review the feature-finish updates and commit them to the branch.
7. **Merge to main.** Squash-merge the branch into main.

Always run `/feature-finish` before merging so its updates are part of the squash — not a separate commit on main.

You can use `/grill-with-docs` between steps 1-2 to stress-test the design against the glossary and past ADRs.

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

The AI outputs a structured summary and then **stops** for your approval.

### The implementation phase

After approval, the AI:
- Writes a test first (TDD) — the test should fail before the fix/change
- Implements the narrowest change that makes the test pass
- Runs affected tests using the test mapping from `aiwiki/owners.md`

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
Brainstorm ─► /specify ─► /plan ─► /tasks ─► /implement ─► /feature-finish ─► Merge ─► /spec-close
                │                                │               │                         │
                │                                │               ├─ product/ updated        ├─ spec archived
                │                                │               ├─ aiwiki/ enriched        └─ ideas extracted
                │                                │               └─ ideas captured
                │                                │
                │                                └─ aiwiki/ read for orientation
                │                                   specs/ideas/ checked for prior work
                │                                   pre-implementation analysis shown
                │
                └─ /grill-with-docs (optional stress-test)
```

### Phase-by-phase

| Phase | What happens | Who drives | Artifacts |
|-------|-------------|------------|-----------|
| **Brainstorm** | Discuss the feature idea | You | Conversation only |
| **Specify** | `/specify` — AI creates spec | You describe, AI writes | `specs/NNN-feature/spec.md` |
| **Plan** | `/plan` — AI creates implementation plan | AI proposes, you approve | `specs/NNN-feature/plan.md` |
| **Tasks** | `/tasks` — AI breaks plan into ordered tasks | AI proposes, you review | `specs/NNN-feature/tasks.md` |
| **Implement** | `/implement` — AI works through tasks (multiple sessions) | AI codes, you review | Code, tests, aiwiki/ enrichment |
| **Finish** | `/feature-finish` — AI audits docs and KB | AI proposes, you review | product/, aiwiki/, ideas updates |
| **Merge** | Squash-merge branch to main | You | Clean history on main |
| **Close spec** | `/spec-close` — AI archives spec, extracts ideas | AI proposes, you confirm | Spec moved to `specs/archive/`, ideas captured |

### For smaller work

| Work type | Flow |
|-----------|------|
| Bug fix | `/bugfix` → review analysis → approve → AI fixes → optional `/feature-finish` → merge |
| Quick change | `/quickchange` → review analysis → approve → AI implements → optional `/feature-finish` → merge |

Run `/feature-finish` before merge whenever the change touched multiple modules or changed behavior. Skip it for trivial single-file fixes.

### Between features

Run `/whatsnext` to see all open work from `specs/backlog.md` and `specs/ideas/`, grouped by area.
