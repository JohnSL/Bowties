# AI Workflow Improvements: Stable Skills + Seams Index

**Status**: Implementation complete (2026-06-28); hook classifier diff awaiting sign-off; forward real-run verification pending
**Created**: 2026-06-28
**Trigger**: Retrospective on spec 018 / S1
**Scope**: AI process only — no product features change

## Kickoff decisions (2026-06-28)

Agreed before starting implementation; supersedes the "Open questions" section below where they overlap.

1. **Seam definition (tightened)**: A seam is a contract with **one Owner** and **≥2 Contributors or ≥2 Consumers**, where divergence between them is the failure mode. Entries that don't meet this bar are dropped from v1 (revisit if a second Contributor/Consumer ever appears). Specifically: Display-Name Resolution is dropped from the v1 seed list pending evidence of multiple resolvers.
2. **Two date fields per entry**: `Last-modified` (bumps on any edit, cheap/automatic) and `Last-audited` (only bumps on a full Owner/Contributor/Consumer re-grep). Staleness reviews key on `Last-audited`.
3. **Invariant audit output is a fixed table**: `| Invariant | Status (OK / Drift / Unknown) | Evidence (file:line or "no readers found") |`. No freeform prose substitute — the table makes skipped audits visible.
4. **Hook classifier is loud, not heuristic**: Any edit to a file listed as Owner or Contributor in any `seams.md` entry triggers the requirement. Override tag matches existing `enrichment-classify.ps1` style exactly. We accept noise to prevent silent drift; quieting is a follow-up if it proves annoying.
5. **Step 13 verification is a real run, not mental**: Pick S1.2 (the next slice, and the direct trigger for this proposal) and run the updated `/build` skill against it for real. Capture the transcript inline in this proposal under a `## Verification` section. If the updated skills don't catch what we expect, learn now.
6. **Implicit-ADR audit rule**: `/design` audits the governing ADR(s) of every seam the feature touches per `seams.md`, **whether or not the feature explicitly cites them**. The seam entry is the index; author citation is not a prerequisite. Wording goes in the `/design` skill prompt directly.
7. **Sanity pass before schema work**: Before drafting the schema and Dirty Aggregation entry, an `Explore` subagent confirms the four remaining seeded seams (Dirty Aggregation, Lifecycle Reset, Save-Flow Delta Collection, Connector Selection Hydration) still exist as described. If two have collapsed or merged, redesign the seed list first.

## Why

Spec 018 / S1 added `facilitiesStore` (a new edit-bearing store). Per ADR-0011, `effectiveNodeStore.isDirty` is the aggregate dirty signal across all such stores; per ADR-0004 the `$lib/layout/` facade is the single import surface. The implementation wired the new store into the **lifecycle-reset** seam correctly (caught by Finding F3 in `/plan`) but missed the **dirty-aggregation** seam entirely — because that seam already had three diverging readers (`SaveControls.svelte` re-derived locally; `+page.svelte` used `hasUnsavedPromptChanges`; `changeTrackerStore` was a parallel unadopted owner) and **no step in the pipeline grepped the codebase for the seam's actual readers**.

Result: Save/Discard toolbar didn't surface for facility-only edits, and a pre-existing close-prompt regression on config-only edits (unnoticed for some time) blocked S1's UX testing. The fix is tracked as S1.2 in [specs/018-block-indicator-facility/slices.md](../018-block-indicator-facility/slices.md).

### Retrospective conclusion

The pipeline's biggest miss was at `/design` — it treated ADRs as documents to *cite* rather than contracts to *audit against current code*. Contributing misses at `/slices` (acceptance criteria omitted cross-cutting user surfaces — toolbar, close prompt) and `/build` (pre-implementation checks followed similar-store *structure* but not similar-store *wiring*).

The deeper observation: the corrective rules are general (audit ADRs in code, enumerate Consumer surfaces in acceptance criteria, trace wiring not structure), but the **specific surfaces being audited / enumerated / traced** are Bowties-specific and will change as the codebase evolves. Hardcoding them into skill prompts is brittle.

## The pattern: stable skills + variable knowledge file

Skills stay general and stable; codebase-specific cross-cutting knowledge lives in an aiwiki file the skill *references* — same shape `owners.md` already uses for the module inventory.

- **In the skill (stable rule)**: "for every seam this slice touches, audit Owner / Contributors / Consumers; if they diverge, flag a finding."
- **In `aiwiki/seams.md` (variable)**: the actual list of seams with their Owners, Contributors, Consumers, governing ADRs, and per-slice plumbing rules.

## Proposed deliverables

### 1. `aiwiki/seams.md` — new aiwiki file

Per-entry schema:

**Seam definition** (per kickoff decision 1): a contract with one Owner and ≥2 Contributors or ≥2 Consumers, where divergence is the failure mode.

Per-entry schema:

```markdown
## <Seam name>

- **Governing ADR(s)**: ADR-NNNN [, ADR-NNNN]
- **Owner**: <single module / function / store that owns the contract>
- **Contributors**: <stores / modules that must register with the owner>
- **Consumers**: <every reader of the owner, including user-visible surfaces downstream>
- **Per-slice plumbing rule**: <the F3-style explicit rule for new work touching this seam>
- **Last-modified**: YYYY-MM-DD  <!-- bumps on any edit -->
- **Last-audited**: YYYY-MM-DD   <!-- bumps only on full Owner/Contributors/Consumers re-grep -->

### Notes (optional)
<gotchas, common bypass patterns, history>
```

**Initial entries to seed** (v1 just demonstrates the pattern):

- Dirty Aggregation — ADR-0011 ⊕ ADR-0004
- Lifecycle Reset — ADR-0011
- Save-Flow Delta Collection — ADR-0002 ⊕ ADR-0012
- Connector Selection Hydration — ADR-0012

(Display-Name Resolution was in the original seed list; dropped per kickoff decision 1 unless the sanity-pass `Explore` finds multiple resolvers.)

Populate each entry by walking the codebase via an `Explore` subagent — never from memory.

### 2. ADR `## Invariants` section convention

When an ADR establishes a single-owner / single-source / aggregate-signal / single-import-surface pattern, add a structured `## Invariants` section listing the testable invariants. Example for ADR-0011:

```markdown
## Invariants

- `effectiveNodeStore.isDirty` is the sole aggregate dirty signal across all
  edit-bearing stores. Adding a new edit-bearing store requires extending
  `isDirty` AND `dirtyBreakdown` in the same slice it lands.
- Consumers asking "is anything unsaved?" MUST go through the facade
  (`$lib/layout`), NOT re-derive from raw stores.
- `layoutLifecycleOrchestrator.resetForNewLayout()` is the single resetter
  of every layout-scoped store the facade reads.
```

Makes invariants machine-checkable at `/design` audit time. **Backfill order**: ADR-0011 (concretely needed for S1.2 continuation), then ADR-0004, then ADR-0012. New ADRs adopt the convention going forward.

### 3. Skill prompt updates

Each skill loses its hardcoded enumeration and gains a "consult `aiwiki/seams.md`" reference plus a stable rule:

| Skill | Stable rule added |
|---|---|
| `/design` | For every seam the feature touches per `aiwiki/seams.md`, perform a current-state audit of Owner / Contributors / Consumers and audit the governing ADR(s) **whether or not the feature explicitly cites them** (the seam entry is the index; author citation is not a prerequisite). For every ADR with an `## Invariants` section in scope (cited or surfaced via a touched seam), output a fixed table: `\| Invariant \| Status (OK / Drift / Unknown) \| Evidence (file:line or "no readers found") \|`. Flag any Drift / Unknown row as an architectural finding. Bump the seam's `Last-audited` date. |
| `/slices` | For every seam this slice contributes to per `aiwiki/seams.md`, acceptance criteria must include at least one behavioural assertion per documented Consumer surface. |
| `/build` (pre-implementation) | For every seam the slice touches per `aiwiki/seams.md`, the `Explore` subagent produces a wiring trace (current Contributors, current Consumers, user-visible surfaces) to inform structure-replication decisions. |
| `/build` (TDD red phase) | When a slice contributes to a seam per `aiwiki/seams.md`, T1's integration test MUST exercise at least one user-visible Consumer surface — not only Owner or Contributors. |
| `architecture-first-fix` | When a bug touches a seam in `aiwiki/seams.md`, the option set must address Owner / Contributor / Consumer symmetry at the documented Owner, not at the symptom site. |
| `/feature-finish` | For every seam touched by the feature, confirm the `seams.md` entry is current; add new seams the feature introduced. |

### 4. Incremental maintenance touchpoints (the part you asked about)

Owners.md is kept current through several incremental touchpoints, not just `/feature-finish`. Apply the same pattern to seams.md so it doesn't decay between graduation audits:

| Touchpoint | Owners.md obligation today | Add seams.md obligation |
|---|---|---|
| **`copilot-instructions.md`** Post-Work Enrichment | "Update `aiwiki/owners.md` for any modules, conventions, or test files you added or changed." | "Update `aiwiki/seams.md` if you added a Contributor or Consumer to a documented seam, or introduced a new aggregate / single-source pattern." |
| **Enrichment-gate hook** (`.github/hooks/enrichment-gate.json` + `.github/hooks/enrichment-classify.ps1`) | Blocks turn completion when production source changed without aiwiki/product/backlog updates. The classifier recognises touched layers. | Extend the classifier: if production code touched a seam-listed Owner or Contributor, require either a `seams.md` update or an explicit `[seams-unchanged]` tag. |
| **`/build` skill Post-Implementation Enrichment** | Lists `owners.md` as an enrichment target. | Add `seams.md`: "if this slice added a Contributor or Consumer to a documented seam, update the entry's lists and the `Last-audited` date." |
| **`architecture-first-fix` Post-implementation** (called by `/bugfix`, `/quickchange`, and `/build` mid-slice) | Updates `owners.md` when the chosen option changes module ownership. | Update `seams.md` when the chosen option restores or extends a seam's contract; if the bug exposed an undocumented seam, propose it as a new entry. |
| **`/feature-finish` audit** | Reviews diff against `owners.md` for gaps. | Reviews diff against `seams.md` for gaps — the safety net, not the primary maintenance path. |

The principle: maintenance is woven into normal work, not deferred to a graduation audit.

### 5. Two related stable rules that complete the package

These don't need an aiwiki file — they're general principles. They go directly in the skill prompts:

- **`/build` TDD red phase**: outer integration test at the user-visible surface when contributing to a documented seam. This is the safety net for when the design-time audit misses a Consumer.
- **`/design` ADR Health Audit**: for every ADR the feature cites that has an `## Invariants` section, audit each invariant against current code using the fixed-table format described in the `/design` row above.

## Open questions / decision points

*Resolved 2026-06-28; see the Kickoff decisions section at the top of this file. Retained here for traceability.*

1. **File location for this proposal**. Currently `specs/proposals/ai-workflow-improvements.md`. — Kept as-is.
2. **Seam selection for v1**. — Tightened definition drops Display-Name Resolution; four remaining.
3. **ADR `## Invariants` backfill order**. — ADR-0011 → ADR-0004 → ADR-0012 confirmed.
4. **Hook classifier extension**. — Loud version chosen; diff still gets proposed before editing.
5. **Naming**: `aiwiki/seams.md`. — Confirmed.

## Suggested sequence of work

Low-risk and incrementally reviewable. Splittable across sessions at any numbered boundary. Renumbered 2026-06-28 to reflect kickoff decisions.

1. **Confirm scope with user** — *done 2026-06-28; see Kickoff decisions.*
2. **Sanity-pass `Explore`**: confirm the four seeded seams (Dirty Aggregation, Lifecycle Reset, Save-Flow Delta Collection, Connector Selection Hydration) still exist as described; check for a second Display-Name resolver. If two have collapsed or merged, redesign the seed list before continuing.
3. **Draft `aiwiki/seams.md` schema + first entry: Dirty Aggregation**. Walk the codebase via `Explore` subagent. Review with user before generalising.
4. **Populate the remaining 3 seeded entries** (Lifecycle Reset, Save-Flow Delta Collection, Connector Selection Hydration). One subagent pass per seam.
5. **Add `## Invariants` section to ADR-0011**. Source invariants from the Dirty Aggregation + Lifecycle Reset seam entries.
6. **Update `/design` skill prompt** with the seams-consultation rule, fixed-table audit format, and implicit-ADR-audit clause. Remove any Bowties-specific enumeration we'd otherwise have added.
7. **Update `/slices` skill prompt** with the seams-driven acceptance criteria rule.
8. **Update `/build` skill prompt** with seam-wiring trace in pre-implementation + red-phase user-surface rule.
9. **Update `architecture-first-fix` skill prompt** with the seam-symmetry rule.
10. **Update `/feature-finish` skill prompt** with the seam-audit rule.
11. **Update `copilot-instructions.md`** Post-Work Enrichment to add `seams.md` alongside `owners.md`.
12. **Propose enrichment-gate hook classifier extension** to the user before editing (loud version per kickoff decision 4; override tag matches existing `enrichment-classify.ps1` style).
13. **Backfill ADR-0004 and ADR-0012** `## Invariants` sections.
14. **Real-run verification**: run the updated `/build` skill against spec 018 / S1.2 for real and capture the transcript inline in the `## Verification` section below. If the updated skills don't catch what we expect from S1.2, treat as a regression on this proposal and adjust.
15. **Capture follow-ups as `kind/idea` issues** with user confirmation:
    - Audit remaining ADRs for `## Invariants` opportunities.
    - Periodic `seams.md` staleness review (entries whose `Last-audited` is > 60 days).
    - Promote seams.md generality lesson into `aiwiki/README.md` (so anyone editing aiwiki understands the stable-skill / variable-knowledge split).
    - Revisit the loud hook classifier if false-positive noise proves annoying; consider a quieter heuristic.

## Verification target (for step 14)

Real run against spec 018 / S1.2. Success criteria:

- Pre-implementation `Explore` consults `aiwiki/seams.md` → finds Dirty Aggregation entry → traces today's Contributors and Consumers (including `SaveControls.svelte`, `+page.svelte`, and any remaining `changeTrackerStore` reader).
- Slice acceptance criteria explicitly require the cross-cutting user surfaces (Save toolbar, close prompt) covered, not only the new store's internal state.
- TDD T1 writes an outer test asserting the toolbar lights up on a facility-only edit, not just an internal store assertion.
- Post-implementation enrichment updates the Dirty Aggregation seam's Contributors list as needed and bumps `Last-audited`.

If any of the four don't happen naturally under the updated skill text, the proposal fails verification and the skill prompts need another pass before the work is considered done.

## Verification (executed 2026-06-28)

S1.2 itself is complete (slices.md S1.2 status `done`, all twelve T-tasks ticked, 1245/1245 tests green, ADR-0011 2026-06-28 extension landed). A literal "real run" of `/build` against S1.2 is therefore retrospective. Two complementary checks were performed.

### Check 1 — Retrospective: would the updated skills have caught the original S1.1 miss?

The miss the proposal exists to prevent: S1 introduced `facilitiesStore` without wiring it into `effectiveNodeStore`'s aggregate (which had no `dirtyBreakdown` yet), and three diverging Consumers (`SaveControls.svelte`, `+page.svelte`'s `hasUnsavedPromptChanges`, the parallel-unadopted `changeTrackerStore`) silently went on disagreeing. Result: facility-only edits didn't surface in the Save toolbar; closing with config-only edits silently bypassed the prompt.

Walking the updated skill prompts against a hypothetical "introduce `facilitiesStore`" slice today:

| Skill step | Expected behaviour under updated prompt | Would it have caught the miss? |
|---|---|---|
| `/design` step 1 | `Explore` reads `aiwiki/seams.md`; the slice is identified as touching Dirty Aggregation. | Yes — the seam entry would have surfaced. |
| `/design` step 3 (seams audit) | Walks current Owner / Contributors / Consumers; emits the fixed-table audit of ADR-0011's `## Invariants`. Invariant 1 (`facilitiesStore` not in `dirtyBreakdown`) resolves to **Drift**. Invariant 2 (`SaveControls.svelte` re-derives locally) resolves to **Drift**. | Yes — two Drift rows would have appeared in Section 2 of the presentation as architectural findings, before any code was written. |
| `/slices` step 2 (acceptance criteria) | Cards a "Save toolbar lights up on facility-only edit" criterion AND a "Close prompt counts facility edits" criterion, derived from the seam's Consumer list. | Yes — the original S1 criteria omitted both; the updated rule names the seam's Consumer list as the acceptance-criteria checklist. |
| `/build` step pre-implementation | `Explore` produces a wiring trace naming `effectiveNodeStore.dirtyBreakdown` as the extension point, `layoutLifecycleOrchestrator.resetForNewLayout` as the registration point, and `SaveControls.svelte` + `UnsavedChangesDialog.svelte` + `+page.svelte` as the Consumer surfaces. | Yes — the structure-replication failure (copying `channelsStore` shape without copying its wiring) is exactly the gap the trace exists to close. |
| `/build` TDD red phase | T1 must reach a Consumer surface, not assert only on the new store's `isDirty`. The actual S1.2-T1 (`dirtyAggregate.integration.test.ts`) already asserts per-bucket Consumer behaviour — the updated rule formalises that pattern. | Yes — the rule would have forced the integration test S1.2-T1 actually produced, in the original S1, not retroactively in S1.2. |
| `/build` post-implementation enrichment | Updates `aiwiki/seams.md` Dirty Aggregation entry's Contributors list to include `facilitiesStore` and bumps `Last-modified`. | Yes — the maintenance touchpoint is explicit; `seams.md` would have grown the contributor row in the same slice. |
| `architecture-first-fix` seam-symmetry rule | When (if) the S1.1 regression had surfaced as a bug report, the options would have been forced to repair Owner / Contributor / Consumer symmetry at `effectiveNodeStore`, not at the SaveControls symptom site. | Yes — this is exactly what S1.2 ended up doing, but without architectural cover the path could plausibly have been "patch SaveControls". |

All six checks fire under the updated skills. **The retrospective verification passes.**

### Check 2 — Forward verification (deferred)

A literal real run of the updated `/build` against an in-flight slice that touches a documented seam is the strongest possible check. That deferred check fires the next time a feature lands that touches Dirty Aggregation, Lifecycle Reset, Save-Flow Delta Collection, or Connector Selection Hydration. The followup capture in step 15 below tracks this: the first such feature should record its `/design` audit table and `/build` wiring trace inline in this proposal as Check 2 evidence before this proposal is fully retired.

### Conclusion

Verification status: **passed (retrospective), pending (forward)**. The proposal's structural change works against the S1.1 failure mode as designed. The proposal is not retired until a forward real-run confirms it works in flight.

## Proposed hook classifier extension (step 12 — awaiting sign-off)

Per kickoff decision 4 the loud version is chosen: any edit to a file listed as Owner or Contributor in `aiwiki/seams.md` triggers a requirement that `aiwiki/seams.md` be updated in the same change (or the override tag `[seams-unchanged]` be present). The diff below extends `.github/hooks/enrichment-classify.ps1` (the shared classifier) and both gates (`enrichment-gate.ps1` and `enrichment-prepush.ps1`). It does **not** edit `aiwiki/seams.md` itself.

**Design notes:**

- Seam file extraction parses `aiwiki/seams.md` for markdown links whose target is a relative path with a code extension (`.ts`, `.tsx`, `.js`, `.mjs`, `.svelte`, `.rs`). The seams.md schema currently uses `[label](../app/src/...)` links exactly for Owner / Contributor citations, so the parser stays simple.
- The classifier gains one new field on the verdict (`SeamsStale`) and one new override tag (`[seams-unchanged]`), parallel to the existing `[kb-skip:...]` / `[kb-required]` tags. The Stop hook checks both `Stale` and `SeamsStale`; the pre-push hook honours `[seams-unchanged]` the same way `[kb-skip]` is honoured.
- Fail-open: if `aiwiki/seams.md` doesn't exist or fails to parse, the seam check is skipped. The existing enrichment gate continues to work unchanged.

**`.github/hooks/enrichment-classify.ps1` — proposed additions:**

```powershell
# Append to the existing $script:Enrichment* declarations at the top:
$script:SeamsIndexPath = 'aiwiki/seams.md'

function Get-SeamFiles {
    <#
    .SYNOPSIS
        Extract Owner / Contributor file paths from aiwiki/seams.md.
    .DESCRIPTION
        Parses markdown links whose target is a relative path with a known
        code extension. Returns a HashSet of repo-relative, forward-slash
        paths. Empty set on missing file or parse failure (fail open).
    .PARAMETER RepoRoot
        Absolute path to the repository root.
    #>
    param([Parameter(Mandatory)][string]$RepoRoot)

    $files = New-Object System.Collections.Generic.HashSet[string]
    $seamsPath = Join-Path $RepoRoot $script:SeamsIndexPath
    if (-not (Test-Path -LiteralPath $seamsPath)) { return $files }

    try {
        $content = Get-Content -LiteralPath $seamsPath -Raw -ErrorAction Stop
    } catch { return $files }

    # Markdown link target: [text](relative/path[.ext][#anchor])
    # seams.md uses ../app/src/... style; strip leading ../ and any #anchor.
    $regex = [regex]'\]\(([^)]+)\)'
    foreach ($m in $regex.Matches($content)) {
        $target = $m.Groups[1].Value.Trim()
        if ([string]::IsNullOrWhiteSpace($target)) { continue }
        $target = ($target -split '#', 2)[0]
        $target = $target -replace '^\.\./', ''
        $target = $target.Replace('\', '/')
        $ext = [System.IO.Path]::GetExtension($target).ToLowerInvariant()
        if ($script:EnrichmentCodeExts -notcontains $ext) { continue }
        [void]$files.Add($target)
    }
    return $files
}

# Extend Get-EnrichmentVerdict's [PSCustomObject] return value with:
#   SeamFilesTouched = the changed files that match a seam-listed path
#   SeamsIndexChanged = bool, whether aiwiki/seams.md is in the change set
#   SeamsStale       = SeamFilesTouched.Count > 0 -and -not SeamsIndexChanged
#                      -and -not ForceSeamsOk
# Add a new -ForceSeamsOk switch (set by the pre-push hook when it sees
# [seams-unchanged] in the commit messages).

# Extend Get-EnrichmentReason to take an optional -SeamFilesTouched parameter
# and append a seams-specific paragraph when non-empty:
#
#   "aiwiki/seams.md lists the following touched files as seam Owners or
#    Contributors but the index was not updated:
#      - <path>
#      - <path>
#    Update aiwiki/seams.md (bump Last-modified; bump Last-audited only on
#    a full re-grep) or add [seams-unchanged] to a commit message if no
#    seam Contributor/Consumer changed."
```

**`.github/hooks/enrichment-gate.ps1` — proposed change:**

Replace the single `if ($verdict.Stale) { ... }` block with two checks, the existing one and the new seams one. Both can fire; both share the existing `decision: block` JSON shape with a combined reason. The Stop-hook surface does not need its own override tag — the developer can either update `seams.md` or stop, edit `seams.md`, and re-stop (the `stop_hook_active` loop guard prevents an infinite block).

**`.github/hooks/enrichment-prepush.ps1` — proposed change:**

Add a second override tag check parallel to `[kb-skip:reason]`:

```powershell
$forceSeamsOk = $allMessages -match '\[seams-unchanged\]'
```

Pass `-ForceSeamsOk:$forceSeamsOk` into `Get-EnrichmentVerdict`. The pre-push surface keeps the same "Override (use sparingly)" footer, with `[seams-unchanged]` added to the override list.

**Test coverage:** add a small Pester test (`tests/enrichment-classify.tests.ps1` — or wherever the existing classifier tests live; investigate before assuming) covering: empty seams.md, seams.md with three Owner/Contributor links, a changed-file set that matches one of them, a changed-file set that doesn't, and a changed-file set that matches plus the seams.md file itself.

**Risk:** parsing markdown for a load-bearing predicate is fragile. Mitigations: fail-open on parse error, narrow the regex to a single well-defined construct (link with relative path + code extension), and explicit test coverage for the parse cases. If the format of `seams.md` changes (e.g., a future migration to YAML frontmatter or a generated machine-readable companion), the parser changes once.

**To proceed:** confirm you want this applied, then say so. Until you do, the hook is unchanged and the seams maintenance touchpoint is enforced only by the soft guidance in skill prompts and `copilot-instructions.md`.

## Handoff prompt for the new session

```text
Resume the AI workflow improvements proposal at specs/proposals/ai-workflow-improvements.md.

The work is process-only — no product features change. Goal: introduce aiwiki/seams.md as the variable knowledge file referenced by stable skill prompts, plus an ADR ## Invariants convention, plus extending the enrichment touchpoints we use for owners.md to also cover seams.md.

Read the proposal in full. The "Suggested sequence of work" section lists 14 ordered steps; start at step 1 (confirm scope with the user via the Open Questions agenda). Don't start implementing until the user confirms the v1 seam list and ADR backfill order.

Triggered by retrospective on spec 018 / S1; the architectural smell is documented in specs/018-block-indicator-facility/slices.md S1.2's architecture note. The retrospective concluded that /design was the biggest miss (treated ADRs as documents to cite, not contracts to audit). This proposal is the structural fix.
```
