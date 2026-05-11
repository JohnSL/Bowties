# specs/ideas/

Structured prior-work cache for deferred ideas. Each file captures analysis and decisions made during planning sessions so they can be reused when relevant work begins.

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

Use consistent tags for discoverability: `documentation`, `cleanup`, `skills`, `instructions`, `aiwiki`, `ci`, `enforcement`, `spec-lifecycle`, `workflows`, `dark-factory`

## Status Values

- **deferred** — recognized as valuable but not in current scope
- **exploring** — being actively investigated
- **superseded** — replaced by another idea or implementation (note replacement)
