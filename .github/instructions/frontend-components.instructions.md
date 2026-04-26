---
applyTo: "app/src/lib/components/**"
description: "Use when editing Bowties Svelte components. Components should stay declarative, render state, and emit intent rather than owning multi-step async workflows or lifecycle sequencing."
---

# Frontend Components

- Use `product/architecture/code-placement-and-ownership.md` when deciding whether logic belongs in a component or should move to a route, orchestrator, store, utility, or backend layer.
- Keep components declarative. They should render state, derive minimal display values, and emit intent events.
- Do not move multi-step workflow sequencing, retry loops, backend coordination, or lifecycle orchestration into components when an orchestrator or store should own it.
- Prefer shared helpers for display-name fallback, formatting, normalization, and reusable translation logic.
- Apply SOLID by keeping each component focused on one rendering or interaction role.
- Apply DRY by extracting repeated rendering helpers or shared child components only when there are real duplicate call sites.
- Apply YAGNI by avoiding generic component abstractions that exist only for possible future reuse.
- Apply TDD by covering rendering and emitted intent behavior with focused component tests.