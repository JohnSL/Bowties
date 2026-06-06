# Decompose `+page.svelte` God-Orchestrator

- **Areas**: architecture, routes, cleanup, orchestration
- **Origin**: spec 014 architectural review (placeholder save / sidebar-disappear bugs, 2026-05-25)
- **Status**: deferred
- **Date**: 2026-05-25

`app/src/routes/+page.svelte` is ~2000 lines long and owns reactive state for nodes, tabs, dialogs, menus, connection lifecycle, layout open/close prompts, refresh coordination, and discovery callbacks — far past what `.github/instructions/frontend-routes.instructions.md` allows for a route file (routes should "compose screens, manage visible page state, and delegate multi-step workflows instead of owning business sequencing"). The placeholder-on-empty-layout sidebar-disappear bug (a page-local `nodes` $state array diverging from `nodeInfoStore`) is the visible symptom; the underlying instance is that ~12 mutator sites all touch the same page-level state because there is no smaller owner to delegate to.

Several concerns currently fused into the route are each large enough to be their own orchestrator or view-model:

- **Connection lifecycle** — connect/disconnect orchestration, online/offline transitions, USB enumeration, transport selection
- **Layout open/close UX** — open-recent menu, "unsaved changes — discard?" prompts, layout-close cleanup fan-out
- **Tab/dialog visibility** — active tab, modal open state, focus management
- **Refresh + rediscovery** — manual refresh, discovery-result handling, captureProgress coordination
- **Menu wiring** — native menu enable bits, menu-event listeners, menu-driven workflow dispatch

Each of these has its own multi-step async character and its own lifecycle-sensitive transitions. They are textbook orchestrator material per `product/architecture/code-placement-and-ownership.md`.

## Prior Work

- **Assessment**: The route is the single biggest shallow-modules instance in the frontend. Every cross-cutting bug (the placeholder bugs are the most recent; spec 013 had several) eventually traces back to *something* in `+page.svelte` not being updated by *something else* in `+page.svelte`. The five concerns above each have a clean seam.
- **Specific candidates**:
  - `screenRosterOrchestrator` (or similar) — owns the page-level visible-nodes derivation; replaces the page's `nodes` $state with a derived view backed by S8.7's roster source-of-truth. **Subsumed by S8.7 if option (A) lands.**
  - `connectionLifecycleOrchestrator` — owns connect/disconnect sequencing, USB enumeration, transport selection. Today's logic is split between `+page.svelte` and `connectionStore`.
  - `layoutLifecycleOrchestrator` — owns open/close/discard prompts and the layout-close cleanup fan-out (currently a ~30-line block that has to be kept in sync with every store that holds layout-scoped state).
  - `menuOrchestrator` — owns native menu enable bits and event-listener wiring. Currently a ~150-line section of `+page.svelte` with implicit dependencies on connection state, layout state, and selected node.
  - View-model per visible screen — once the orchestrators above own the workflows, each visible screen could read from a thin view-model rather than from raw stores. Lower priority than the orchestrators.
- **Dead code candidates**: `+page.svelte` likely contains several stale branches from the configuration-modes refactor (specs 010-013) that no longer fire. A pass for unreachable `{#if}` arms + unused callback props should be part of any decomposition slice.
- **Blocked by**: S8.6 / S8.7 / S8.8 first deepen the underlying stores (single CDI artifact resolver, single node-roster source, store-owned save-time partition) so the page has clean dependencies to orchestrate. Decomposing the page before those land would just split a tangle of fan-out into smaller tangles.
- **Risk**: Medium. The route file is load-bearing — every visible flow passes through it. Decomposition needs to land slice-by-slice, with one orchestrator extracted per PR, end-to-end tests gating each step. Doing it as one big refactor would be high-risk and is not recommended.
