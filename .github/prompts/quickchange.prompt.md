---
description: "Make a focused change with visible pre-implementation analysis, TDD, duplication prevention, and knowledge base enrichment."
---

**First action**: Use `manage_todo_list` to create a todo from all 11 steps below. Update status as you work. Do not mark the task complete until all items including post-implementation are done.

## Pre-Implementation Analysis (output this BEFORE coding)

Use the Explore subagent for research steps to keep the main context window lean.

1. **Prior work**: Search open GitHub issues labeled `kind/idea` filtered by the relevant `area/*` labels (`gh issue list --repo JohnSL/Bowties --label kind/idea --state open`) for prior analysis or deferred decisions. Also glance at any residual `specs/ideas/**` files until migration completes.
2. **Affected layers**: Read `aiwiki/owners.md` to identify which modules and layers this change touches. Note the test file(s) for each.
3. **Shared logic check**: Check `aiwiki/owners.md` shared conventions section — does relevant shared logic already exist? Will this change create a new pattern that should be shared?
4. **Placement check**: Verify new or moved logic goes in the right layer per `product/architecture/code-placement-and-ownership.md`.
5. **ADR check**: Scan `product/architecture/adr/` for decisions that constrain this change.
6. **Options and sign-off**: Load and follow the `architecture-first-fix` skill. It defines the option format (seam + ADRs + named principle: DRY / SOLID / YAGNI / Depth / Locality / ADR-compliance + tradeoff), the stopgap rule, and the stop-and-wait gate. Present the structured pre-implementation summary together with the options.

**STOP here and wait for user sign-off before implementing.**

## Implementation (after approval)

6. **TDD**: Add or update a focused test around the behavior seam first, then implement the smallest change that makes it pass.
7. **No duplication**: Reuse existing shared helpers rather than creating local variants. If new shared logic is needed, add it to the appropriate utils/ or shared module.
8. **Run affected tests**: Use the test mapping from `aiwiki/owners.md` to identify and run all tests that cover the changed modules.

## Post-Implementation (you are NOT done — complete these before summarizing)

9. **Enrich aiwiki/**: If the change revealed a module, convention, or flow not listed in `aiwiki/`, add it.
10. **Update product/ docs**: If the change affects user-visible behavior or ownership, update the relevant product/ doc.
11. **Backlog check**: Review `specs/backlog.md` — does this change resolve or reveal a backlog item?
