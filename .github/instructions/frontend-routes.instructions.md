---
applyTo: "app/src/routes/**"
description: "Use when editing Bowties Svelte route files. Routes should compose screens, manage visible page state, and delegate multi-step workflows instead of owning business sequencing."
---

# Frontend Routes

- Use `product/architecture/code-placement-and-ownership.md` when deciding whether logic belongs in a route or should move to a component, orchestrator, store, utility, or backend layer.
- Keep routes focused on screen composition, page-level state wiring, and user interaction entry points.
- Do not embed multi-step business workflows, protocol sequencing, or cross-store orchestration directly in route files when an orchestrator or focused store should own that behavior.
- When touching a route that already owns logic better placed elsewhere, move that logic toward the correct owner instead of extending the mixed boundary.
- Reuse shared helpers for Node ID normalization, naming fallback, and formatting instead of implementing route-local variants.
- Apply SOLID by keeping each route responsible for one screen-level composition concern.
- Apply DRY by delegating repeated workflow logic to orchestrators or shared helpers.
- Apply YAGNI by resisting generic route frameworks or helpers that do not remove a current duplication or bug.
- Apply TDD by adding or updating focused route or integration tests for user-visible flow changes before or alongside the implementation, and add missing regression coverage when a touched route lacked it.