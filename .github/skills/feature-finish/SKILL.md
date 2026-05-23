---
name: feature-finish
description: Graduation workflow for completing a feature. Reviews spec completion, updates durable product docs, enriches aiwiki/, captures deferrals as ideas, and verifies backlog. Does NOT commit or merge.
---

# Feature Finish

Run this skill after implementation is complete but before committing. It ensures that documentation, knowledge base, and backlog stay current with the code changes.

## Process

### 1. Identify the spec

Find the active spec directory under `specs/` for this feature. If there is no spec (ad-hoc work), note that and skip spec-related steps.

### 2. Diff summary

Review the working changes (`git diff --stat` and `git diff --name-only`) to understand what was touched. Summarize:
- Which layers were modified (routes, components, orchestrators, stores, utils, backend, lcc-rs)
- Which modules were added, changed, or removed
- Which tests were added or changed

### 3. Assess product/ impact

Check whether any user-visible behavior, workflow, or architecture ownership changed:
- **product/glossary.md**: Do any new terms need to be added? Were any existing terms redefined by this work?
- **product/architecture/**: Do code-placement rules or ownership boundaries need updating?
- **product/architecture/adr/**: Were architecture decisions made during this work that should be recorded?

Update the affected product/ files inline.

### 4. Assess aiwiki/ impact

Check whether the AI knowledge base needs updates for the touched modules:

- **aiwiki/owners.md**: Are all new or changed modules listed? Are test file mappings current? Are shared conventions updated if a new pattern was established?
- **aiwiki/flows.md**: Did any workflow's module participation change?
- **aiwiki/architecture-health.md**: Were any coupling risks, architecture debt items, or depth assessments affected?

Update the affected aiwiki/ files inline.

### 5. Consistency check

Verify no stale references:
- Grep for references to renamed or removed files in product/, aiwiki/, and .github/
- Check that any new shared logic added during the feature is documented in owners.md shared conventions section
- Check that integration boundaries in owners.md still match reality

### 6. Capture deferrals as ideas

Review conversations, TODOs-in-code, and spec notes for work that was deferred during implementation:
- Create `specs/ideas/<bucket>/<slug>.md` files for any deferred items using the standard format (title, areas, origin, status, date, prior work). Choose the bucket per `specs/ideas/README.md`: `features/` for user-facing capability, `refactors/` for code/architecture improvements, `docs/` for documentation work, `process/` for dev workflow/tooling
- Do not create ideas for trivial cleanup — only for items with enough substance to save future re-discovery

### 7. Backlog check

Read `specs/backlog.md` and update it:
- Mark resolved items as done
- Add newly revealed backlog items
- Update items whose scope or priority changed due to this work

### 8. Spec status

If this work has an active spec:
- Review whether all spec requirements were addressed
- Note any requirements that were descoped or deferred (should already be captured as ideas in step 6)

### 9. Summary

Output a concise summary:
- What product/ docs were updated and why
- What aiwiki/ docs were updated and why
- What ideas were captured
- What backlog items were resolved or added
- Any open items that still need attention

Do NOT commit, merge, or push. The user will review all changes first.
