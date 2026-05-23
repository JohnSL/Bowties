---
agent: speckit.implement
---

**First action**: Use `manage_todo_list` to create a todo from all 8 Bowties-specific steps below (in addition to any task-level todos). Update status as you work. Do not mark the task complete until all items including post-implementation enrichment are done.

## Bowties Pre-Implementation Analysis (do this BEFORE executing any tasks)

Output the following analysis visibly before proceeding to task execution:

1. **Prior work**: Search `specs/ideas/**` (all bucket subfolders) for idea files with area tags matching this feature. Surface any relevant prior analysis or deferred decisions that affect scope.
2. **Existing logic check**: Read `aiwiki/owners.md` to verify whether logic needed for this feature already exists. Identify shared conventions, helpers, or modules that should be reused instead of reimplemented.
3. **Placement verification**: For each new module or significant logic change, verify correct layer placement per `product/architecture/code-placement-and-ownership.md`.
4. **ADR check**: Scan `product/architecture/adr/` for decisions that constrain the implementation approach.

## Bowties Post-Implementation Enrichment (you are NOT done — complete these before summarizing)

5. **Enrich aiwiki/owners.md**: Add any new modules, update test file mappings, and document any new shared conventions established during implementation.
6. **Enrich aiwiki/flows.md**: If workflow module participation changed, update the relevant flow entries.
7. **Record ADRs**: If architecture decisions were made during implementation (e.g., choosing one approach over alternatives for load-bearing reasons), record them in `product/architecture/adr/`.
8. **Backlog check**: Review `specs/backlog.md` — resolve completed items and add newly revealed items.
