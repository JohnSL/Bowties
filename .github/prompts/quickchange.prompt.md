---
description: "Make a focused change with visible pre-implementation analysis, TDD, duplication prevention, and knowledge base enrichment."
---

**First action**: Use `manage_todo_list` to create a todo from all 7 steps below. Update status as you work. Do not mark the task complete until all items including post-implementation are done.

## Pre-Implementation Analysis (delegated)

Do **not** read source files, ADRs, `aiwiki/`, or search GitHub issues in
the main conversation. Delegate the entire analysis to the
[`change-analyze`](../agents/change-analyze.agent.md) subagent.

1. **Invoke `change-analyze`** with:
   - **Mode**: `quickchange`.
   - **Problem statement**: the behavior change the user wants, framed as
     the contract to establish (not "add feature X" but "the system should
     behave as Y when Z").
   - **Seam context**: any modules the user mentioned; otherwise empty.
   - **User context**: explicit hints from the user (e.g. "must reuse
     existing helper X", "should be a type-level change").

   Expect back: Seam summary, Options (with required `Regression class
   prevented:` fields), Recommendation, and an Investigation audit.

2. **Spot-check the audit** for red flags (stale assumptions, misapplied
   ADRs, options rejected by self-check that should have been presented).
   Re-invoke with a correcting note, or flag concerns in your presentation.

3. **Present the options block to the user** (omit the audit from the
   default presentation; keep it in your own context). Order: Seam summary
   → Options → Recommendation.

**STOP here and wait for user sign-off before implementing.**

## Implementation (after approval)

Delegate the Red+Green loop to the `tdd-cycle` subagent. Do **not** write
tests or production code inline in the main conversation.

4. **Compose the behavior list and invoke `tdd-cycle`**: from the accepted
   option, list the 1–3 behaviors this change establishes. Reuse existing
   shared helpers rather than creating local variants; if new shared logic
   is needed, name where it will live per the Seam summary. Invoke
   `tdd-cycle` with:
   - **Task title**: a short summary of the change.
   - **Acceptance context**: the behavior contract this change establishes.
   - **Behaviors**: the ordered list you composed.
   - **Test location + framework** per behavior: from the Seam summary's
     test-files list, mapped to Bowties testing context (Vitest for
     frontend, `cargo test` for backend/lcc-rs).
   - **Risk note** if the accepted option touches a risky seam.

   Work only from `tdd-cycle`'s returned audit summary. Do not re-read the
   full test output in the main conversation.

5. **Handle escalation**: if `tdd-cycle` returned an `architecture-first-fix`
   escalation, present its option draft to the user and wait for a choice
   before re-invoking. If the escalation's options need fuller analysis,
   re-invoke `change-analyze` with mode `mid-slice-escalation` and the
   escalation context.

6. **Run affected tests**: use the test mapping from the Seam summary to
   identify and run all tests that cover the changed modules (broader than
   `tdd-cycle`'s per-behavior suite run).

## Post-Implementation (you are NOT done — complete these before summarizing)

7. **Enrich aiwiki/, product/ docs, and check the backlog**: If the change
   revealed a module, convention, or flow not listed in `aiwiki/`, add it.
   If it affects user-visible behavior or ownership, update the relevant
   `product/` doc. Review `specs/backlog.md` — does this change resolve or
   reveal a backlog item?
