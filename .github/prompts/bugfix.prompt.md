---
description: "Fix a bug with root cause analysis, TDD regression encoding, impact-aware testing, and knowledge base enrichment."
---

**First action**: Use `manage_todo_list` to create a todo from all 8 steps below. Update status as you work. Do not mark the task complete until all items including post-implementation are done.

## Pre-Implementation Analysis (delegated)

Do **not** read source files, ADRs, `aiwiki/`, or search GitHub issues in
the main conversation. Delegate the entire analysis to the
[`change-analyze`](../agents/change-analyze.agent.md) subagent.

1. **Invoke `change-analyze`** with:
   - **Mode**: `bugfix`.
   - **Problem statement**: the user's bug description, plus any
     reproduction hints or observed vs. expected behavior.
   - **Seam context**: pass whatever the user has already given you
     (mentioned modules, files, prior guesses). Otherwise leave empty and
     the subagent will discover it.
   - **User context**: any explicit hint from the user (e.g. "must be a
     type-level fix", "release pressure — a stopgap is acceptable").

   Expect back one structured block containing: Seam summary, Symptom vs.
   root cause, Impact radius, Risk, Options (with required `Regression
   class prevented:` fields), Recommendation, and an Investigation audit
   (hypotheses ruled out, ADRs scanned, options rejected by self-check,
   prior-work issues, assumptions).

2. **Validate the returned analysis** using the caller validation gate in the
  `architecture-first-fix` skill. Re-invoke `change-analyze` with a correcting
  note if any check fails.

3. **Present the options block to the user** (omit the audit from the
   default presentation; keep it in your own context for follow-up
   questions). Present in this order: Seam summary → Symptom vs. root
   cause → Impact radius → Risk → Options → Recommendation.

**STOP here and wait for user sign-off before implementing.**

## Implementation (after approval)

Delegate the Red+Green loop to the `tdd-cycle` subagent. Do **not** write
tests or production code inline in the main conversation.

4. **Compose the behavior list and invoke `tdd-cycle`**: from the accepted
   option, list the 1–3 behaviors to encode (typically: regression
   behavior + optional invariant test + optional adjacent-consistency
   test). Invoke `tdd-cycle` with:
   - **Task title**: the bug title.
   - **Acceptance context**: "the regression no longer reproduces and the
     named invariant(s) hold."
   - **Behaviors**: the ordered list you composed.
   - **Test location + framework** per behavior: from the Seam summary's
     test-files list, mapped to Bowties testing context (Vitest for
     frontend, `cargo test` for backend/lcc-rs).
   - **Expected file surface** per behavior: exact test files, production
     owner files, and explicitly allowed generated output or documentation.
   - **Risk note** if the accepted option touches a risky seam (auto-narrow
     to 1 behavior).

   Split invocations at module-owner, toolchain, generator, and generated-output
   boundaries. Do not combine them into one cycle batch.

   Use `tdd-cycle`'s audit summary to conserve context; do not replay full test
   output unless verification fails. The caller still verifies the actual diff,
   aggregate tests, and diagnostics before reporting success.

5. **Handle escalation**: if `tdd-cycle` returned an
   `architecture-first-fix` escalation, invoke `change-analyze` with mode
   `mid-slice-escalation` and the worker's structured evidence. Validate its
   return as in step 2, present the options to the user, and wait for a choice
   before re-invoking the cycle. Do not patch through.

   After every successful `tdd-cycle` invocation, compare the actual
   baseline-relative changed-file set with that batch's expected surface.
   Resolve any mismatch before invoking the next batch.

6. **Verify repository state and select validation** (main-window, small):
   inspect the baseline-relative Bowties changed-file set and confirm it
   matches the worker report and accepted scope. Select validation using
   `product/quality/testing-strategy.md`. For a source-changing bugfix, run the
   full Bowties test suite, not just tests mapped to the changed module, then
   check current diagnostics. Do not run application tests when the task
   changes only documentation/planning content. Never execute tooling in a
   reference repository. Also scan touched source files for dead code, stale
   imports, or pattern non-conformance introduced by the fix — fix in place if
   narrow and testable.

## Post-Implementation (you are NOT done — complete these before summarizing)

7. **Enrich aiwiki/ and product/ docs**: If the fix revealed a module,
   convention, or flow not listed in `aiwiki/`, add it. If it changes
   user-visible behavior or ownership, update the relevant `product/` doc.
8. **Backlog check**: Review `specs/backlog.md` — does this fix resolve or
   reveal a backlog item?
