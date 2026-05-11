# aiwiki/

Code-level navigation for AI agents working in the Bowties codebase.

## Purpose

Help AI agents quickly answer:
- **Where does X live?** → [owners.md](owners.md)
- **Does this already exist?** → [owners.md](owners.md) shared conventions section
- **What's involved in this workflow?** → [flows.md](flows.md)
- **Is there coupling or debt to watch?** → [architecture-health.md](architecture-health.md)

## Scope

aiwiki/ covers **WHERE** things live and **HOW** they connect. For **WHAT** the product does and **WHY**, see `product/`.

## Precedence

`product/ + code` > `aiwiki/` > `specs/`

If aiwiki/ contradicts code or product/ docs, trust the code and product/ docs. Fix the aiwiki/ entry.

## Format Rules

- One-line module purposes. No paragraphs.
- Test files listed inline with their module.
- Shared conventions name the canonical implementation file.
- Flows list participating modules, not full code-path traces.

## Enrichment Model

aiwiki/ grows incrementally during feature work. After touching a module:
1. Verify its entry in owners.md is accurate; update if not.
2. If a new module, convention, or flow was added, add it.
3. If coupling or debt was discovered, note it in architecture-health.md.

Staleness rule: if you discover something not listed here, add it before completing your change.
