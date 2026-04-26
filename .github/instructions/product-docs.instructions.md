---
applyTo: "product/**"
description: "Use when editing Bowties durable product-behavior or architecture docs. Keep docs current, concise, behavioral, and explicit about ownership, testing, and architecture boundaries."
---

# Product Docs

Files in this scope are the durable product docs for current behavior, workflows, architecture boundaries, and testing strategy.

- Document the current truth about behavior, workflows, architecture boundaries, and testing strategy.
- Write for implementers and reviewers, not end users.
- Prefer current contracts, owners, and acceptance rules over history, design brainstorming, or changelog-style narration.
- Keep `product/architecture/code-placement-and-ownership.md` current when the repo-wide placement model changes.
- When describing architecture, name which layer owns the behavior: route, component, orchestrator, store, backend module, or protocol library.
- When describing a regression seam, state the protected behavior and the test surface that should guard it.
- Keep SOLID, DRY, YAGNI, and TDD concrete. Describe the actual ownership rule, shared helper, minimal abstraction, or expected test rather than repeating the acronym alone.
- If a document conflicts with current implementation, either update it or mark it as stale immediately.