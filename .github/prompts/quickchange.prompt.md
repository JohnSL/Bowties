---
description: "Make a focused change with visible pre-implementation analysis, TDD, duplication prevention, and knowledge base enrichment."
---

## Pre-Implementation Analysis (output this BEFORE coding)

Use the Explore subagent for research steps to keep the main context window lean.

1. **Prior work**: Search `specs/ideas/` for idea files with matching area tags. Surface any relevant prior analysis or deferred decisions.
2. **Affected layers**: Read `aiwiki/owners.md` to identify which modules and layers this change touches. Note the test file(s) for each.
3. **Shared logic check**: Check `aiwiki/owners.md` shared conventions section — does relevant shared logic already exist? Will this change create a new pattern that should be shared?
4. **Placement check**: Verify new or moved logic goes in the right layer per `product/architecture/code-placement-and-ownership.md`.
5. **ADR check**: Scan `product/architecture/adr/` for decisions that constrain this change.

Output the analysis as a structured summary with your proposed change approach.

**STOP here and wait for user sign-off before implementing.**

## Implementation (after approval)

6. **TDD**: Add or update a focused test around the behavior seam first, then implement the smallest change that makes it pass.
7. **No duplication**: Reuse existing shared helpers rather than creating local variants. If new shared logic is needed, add it to the appropriate utils/ or shared module.
8. **Run affected tests**: Use the test mapping from `aiwiki/owners.md` to identify and run all tests that cover the changed modules.

## Post-Implementation

9. **Enrich aiwiki/**: If the change revealed a module, convention, or flow not listed in `aiwiki/`, add it.
10. **Update product/ docs**: If the change affects user-visible behavior or ownership, update the relevant product/ doc.
11. **Backlog check**: Review `specs/backlog.md` — does this change resolve or reveal a backlog item?
