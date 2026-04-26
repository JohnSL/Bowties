---
applyTo: "app/src/lib/stores/**"
description: "Use when editing Bowties frontend stores. Stores own durable frontend state and deterministic transitions, not scattered workflow sequencing."
---

# Frontend Stores

- Use `product/architecture/code-placement-and-ownership.md` when deciding whether logic belongs in a store or should instead live in an orchestrator, utility, route, or backend layer.
- Stores own durable frontend state, derived state, and deterministic transitions.
- Keep store APIs explicit and predictable so routes, components, and orchestrators can depend on them safely.
- Do not spread the same state transition logic across multiple stores or duplicate normalization rules inside store methods.
- Keep workflow sequencing in orchestrators unless a store is the clearly documented owner of that transition.
- Apply SOLID by giving each store one coherent state ownership role.
- Apply DRY by centralizing transition helpers and normalization rules in one place.
- Apply YAGNI by avoiding overly generic store frameworks or abstractions that do not reduce current complexity.
- Apply TDD by adding or updating focused store tests for state transitions and regression cases.