---
applyTo: "app/src/lib/orchestration/**"
description: "Use when editing Bowties frontend orchestrators. Orchestrators own multi-step async workflows, lifecycle-sensitive transitions, and cross-store coordination."
---

# Frontend Orchestration

- Use `product/architecture/code-placement-and-ownership.md` when deciding whether logic belongs in an orchestrator or should instead live in a route, store, backend module, or protocol library.
- Orchestrators are the primary owners of multi-step async workflows and lifecycle-sensitive transitions.
- Name the workflow that each orchestrator owns and keep its boundary explicit.
- Coordinate routes, components, stores, and backend calls here instead of spreading sequencing logic across those layers.
- Reuse shared normalization and translation helpers instead of re-encoding comparison rules inside a workflow.
- Apply SOLID by keeping one orchestrator focused on one coherent workflow or seam.
- Apply DRY by centralizing repeated workflow sequencing and guard conditions instead of duplicating them in routes or stores.
- Apply YAGNI by choosing explicit workflow steps over generic orchestration frameworks.
- Apply TDD by writing or updating focused workflow tests for lifecycle transitions, replay/apply flows, and regression seams before or alongside implementation.