# specs/ideas/

Structured prior-work cache for deferred ideas. Each file captures analysis and decisions made during planning sessions so they can be reused when relevant work begins.

## Buckets

Ideas are organized into four subfolders by the kind of work they represent. Pick the bucket that best fits the idea's primary nature; cross-cutting ideas should go where the *load-bearing* work lives.

- **`features/`** — user-facing product capability ideas. New behaviors, workflows, or mental-model changes the user will notice.
- **`refactors/`** — internal code and architecture improvements. Decompositions, extractions, restructurings, depth/locality fixes. No user-visible behavior change intended.
- **`docs/`** — documentation reorganization, migration, archival, and doc-adjacent CI/path updates.
- **`process/`** — developer workflow, skills, prompts, instructions, enforcement, CI policy, spec lifecycle.

When in doubt: if a user would care, it's `features/`. If only contributors would care about the code shape, it's `refactors/`. If it's about words on a page, it's `docs/`. If it's about how we work, it's `process/`.

## Scanning

Tools that scan ideas should recurse into all bucket subfolders (`specs/ideas/**/*.md`), excluding `README.md`.

## Format Convention

Each idea file follows this structure:

```md
# {Title}

- **Areas**: {comma-separated area tags for discoverability}
- **Origin**: {which spec, plan, or conversation produced this idea}
- **Status**: deferred | exploring | superseded
- **Date**: {YYYY-MM-DD}

{One-paragraph description of the idea.}

## Prior Work

{Reusable analysis, decisions, constraints, and relevant context discovered during planning.}
```

## Area Tags

Use consistent tags for discoverability: `documentation`, `cleanup`, `skills`, `instructions`, `aiwiki`, `ci`, `enforcement`, `spec-lifecycle`, `workflows`, `dark-factory`, `architecture`, `backend`, `stores`, `orchestration`, `layout`, `connection`, `startup`, `save-flow`, `testing`

## Status Values

- **deferred** — recognized as valuable but not in current scope
- **exploring** — being actively investigated
- **superseded** — replaced by another idea or implementation (note replacement)
