---
description: "Fix a bug with visible pre-implementation analysis, TDD regression encoding, and knowledge base enrichment."
---

**First action**: Use `manage_todo_list` to create a todo from all 11 steps below. Update status as you work. Do not mark the task complete until all items including post-implementation are done.

## Pre-Implementation Analysis (output this BEFORE coding)

Use the Explore subagent for research steps to keep the main context window lean.

1. **Prior work**: Search `specs/ideas/` for idea files with matching area tags. Surface any relevant prior analysis.
2. **Owner identification**: Read `aiwiki/owners.md` to find which module owns the affected behavior. Identify the test file(s) for that module.
3. **Shared logic check**: Check `aiwiki/owners.md` shared conventions section — is there existing shared logic relevant to this bug? Avoid reimplementing what already exists.
4. **Placement check**: Verify the fix belongs in the identified module per `product/architecture/code-placement-and-ownership.md`.
5. **ADR check**: Scan `product/architecture/adr/` for decisions that constrain the fix approach.

Output the analysis as a structured summary with your proposed fix approach.

**STOP here and wait for user sign-off before implementing.**

## Implementation (after approval)

6. **Encode the regression**: Write a focused test that reproduces the bug (the test should fail before the fix).
7. **Fix narrowly, improve adjacently**: Implement the smallest change that makes the test pass. Then scan the touched files for adjacent consistency issues — missing pattern conformance, incomplete reset sequences, dead code — and fix them as part of the same change when the improvement is narrow and testable. Do not refactor unrelated modules.
8. **Run affected tests**: Use the test mapping from `aiwiki/owners.md` to identify and run all tests that cover the changed module, including cross-layer tests.

## Post-Implementation (you are NOT done — complete these before summarizing)

9. **Enrich aiwiki/**: If the fix revealed a module, convention, or flow not listed in `aiwiki/`, add it.
10. **Update product/ docs**: If the fix changes user-visible behavior or ownership, update the relevant product/ doc.
11. **Backlog check**: Review `specs/backlog.md` — does this fix resolve or reveal a backlog item?
