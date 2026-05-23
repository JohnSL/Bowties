---
description: "Fix a bug with root cause analysis, TDD regression encoding, impact-aware testing, and knowledge base enrichment."
---

**First action**: Use `manage_todo_list` to create a todo from all 13 steps below. Update status as you work. Do not mark the task complete until all items including post-implementation are done.

## Pre-Implementation Analysis (output this BEFORE coding)

Use the Explore subagent for research steps to keep the main context window lean.

1. **Prior work**: Search open GitHub issues labeled `kind/idea` filtered by the relevant `area/*` labels (`gh issue list --repo JohnSL/Bowties --label kind/idea --state open`) for prior analysis. Also glance at any residual `specs/ideas/**` files until migration completes.
2. **Owner identification**: Read `aiwiki/owners.md` to find which module owns the affected behavior. Identify the test file(s) for that module.
3. **Shared logic check**: Check `aiwiki/owners.md` shared conventions section — is there existing shared logic relevant to this bug? Avoid reimplementing what already exists.
4. **Placement check**: Verify the fix belongs in the identified module per `product/architecture/code-placement-and-ownership.md`.
5. **ADR check**: Scan `product/architecture/adr/` for decisions that constrain the fix approach.
6. **Root cause analysis**: Distinguish the _symptom_ from the _cause_. Trace the bug to the point where the contract or invariant was violated — not just where the wrong value surfaces. If the symptom and root cause are in different modules, identify both.
7. **Impact tracing**: Identify callers, subscribers, and downstream consumers of the code you plan to change. List modules that depend on the current behavior, even if it is buggy — they may have adapted to it.

Output the analysis as a structured summary with:
- Symptom vs. root cause (if different)
- Proposed fix approach (at the root cause, not the symptom)
- Impact radius: modules and flows affected by the change
- Risk assessment: what could break if the fix changes observable behavior

**STOP here and wait for user sign-off before implementing.**

## Implementation (after approval)

8. **Encode the regression**: Write a focused test that reproduces the bug (the test should fail before the fix). If the root cause reveals a missing invariant, encode that invariant as a separate test.
9. **Fix at the root cause**: Implement the fix at the actual point of failure, not a downstream workaround. Scan the touched files for adjacent consistency issues — missing pattern conformance, incomplete reset sequences, dead code — and fix them as part of the same change when the improvement is narrow and testable. Do not refactor unrelated modules.
10. **Run full test suite**: Run all tests, not just the ones mapped to the changed module. A bugfix that changes observable behavior can break consumers that the module-level mapping doesn't cover.

## Post-Implementation (you are NOT done — complete these before summarizing)

11. **Enrich aiwiki/**: If the fix revealed a module, convention, or flow not listed in `aiwiki/`, add it.
12. **Update product/ docs**: If the fix changes user-visible behavior or ownership, update the relevant product/ doc.
13. **Backlog check**: Review `specs/backlog.md` — does this fix resolve or reveal a backlog item?
