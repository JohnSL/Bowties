---
applyTo: "app/src/lib/utils/**"
description: "Use when editing Bowties frontend utility modules. Shared helpers should own normalization, formatting, and reusable translation logic with clear tests and no hidden workflow side effects."
---

# Frontend Utils

- Use `product/architecture/code-placement-and-ownership.md` when deciding whether logic belongs in a utility helper or should instead live in a store, orchestrator, component, route, backend module, or protocol library.
- Keep utility modules pure and focused on normalization, formatting, comparison, and value translation logic.
- Centralize shared rules such as Node ID normalization and display-name fallback here instead of duplicating them across routes, components, stores, or orchestrators.
- Do not hide workflow sequencing, store mutation, or backend calls inside utility helpers.
- Apply SOLID by keeping helpers focused and explicit.
- Apply DRY by making one helper the canonical owner of a shared rule.
- Apply YAGNI by resisting utility abstractions that guess at future reuse.
- Apply TDD by covering edge cases, canonicalization rules, and regression inputs with focused unit tests.