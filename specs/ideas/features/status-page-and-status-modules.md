# Status Page With Profile-Driven Status Modules

- **Areas**: layout, orchestration, backend, stores, profiles, polling
- **Origin**: brainstorm conversation (2026-05-23) about TCS LT-50 throttle, CS-105 command station, and booster status visibility
- **Status**: deferred
- **Date**: 2026-05-23

A new "Status" surface in Bowties that shows live information about "power" devices on the layout (TCS LT-50 throttle, CS-105 command station, boosters) and any other node type that exposes useful runtime data. Power devices expose track voltage, current, status, temperature, and per-locomotive statistics as read-only CDI fields, plus power-on / power-off producer events. The status page is populated by per-node-type **status modules** defined declaratively in the existing `profiles/<model>/` bundle (alongside `recipes.yaml`, `field-descriptions.yaml`, `event-roles.json`, etc.). Profiles for new device types are author-time artifacts intended to be contributed via PR, not authored by end users.

## Prior Work

### Options considered

- **Option A — Declarative status module in profile YAML, fixed widget catalog.** Add `status.yaml` to each profile folder. Schema lists *sources* (CDI paths with poll cadence, event IDs, SNIP fields) and *cards* that bind sources to a built-in widget catalog (numeric readout, gauge with min/max/thresholds, on/off LED, enum label, counter, sparkline, event-indicator). Status page matches connected nodes to profiles by SNIP manufacturer+model and renders the profile's cards.
- **Option B — Imperative per-device code modules (TS/Rust plugins).** Each device type ships custom render + polling code. Maximum flexibility, hostile to PR contribution, heavy maintenance, multiplies lifecycle surface.
- **Option C — Generic user-built dashboard, no profile required.** User picks any read-only CDI field on any node and drops it on a dashboard. No curated experience for known devices; every user redoes the work.
- **Option D — CDI XML annotations / sidecar overlay.** Bowties-specific attributes on CDI. Pollutes vendor data or requires a parallel CDI overlay harder to author than YAML. Can't easily express cross-field cards.
- **Option E — Hybrid A + C.** Profile provides curated cards; same widget catalog also powers a user-editable blank dashboard for unknown nodes. Best of both, larger surface, needs user-card persistence story (per-layout vs. per-app).

### Decision: Option A

Reasoning:
- Matches the existing `profiles/<model>/` PR-driven contribution model already used for recipes and field descriptions.
- One backend polling/coalescing orchestrator owns all reads — testable, fits `frontend-orchestration` + `src-tauri` boundaries, keeps `lcc-rs` free of UI concerns.
- Fixed widget catalog keeps the frontend declarative.
- E is a clean extension if user-built dashboards become a real ask, but invoking YAGNI: ship curated profile-driven cards first and reconsider later. No part of A precludes E.

### Author-time vs end-user

Profile authoring is explicitly a contributor workflow. Initial path is hand-edited YAML plus a `STATUS-AUTHORING.md` doc sibling to the existing profile-extraction skills. An in-app capture wizard (riding on the existing CDI viewer to let a contributor tick read-only fields, pick a widget, set thresholds, and export a YAML snippet) is a plausible follow-up once the schema has settled, but not on the critical path.

### Data acquisition

Power devices surface most of their interesting state via read-only CDI memory reads, which are slow and datagram-based. The status orchestrator must:

- Share infrastructure with the existing CDI read pipeline.
- Coalesce reads to the same node and respect existing backoff.
- Support per-field poll cadence (voltage every few seconds, statistics every tens of seconds, temperature in between).
- Pause polling when the node is away / not connected.
- Subscribe to producer events (power-on / power-off) separately via the existing event pipeline; the status page reflects event-driven state changes without a poll.

Other potential sources to keep in mind when shaping the schema: SNIP fields (already cached), PIP-derived capability flags, and traction-protocol queries for throttles. The schema should allow a card source to name its acquisition kind, not assume CDI everywhere.

### Open questions to resolve when this becomes a spec

- **Poll cadence policy** — profile suggests defaults; should the user be able to override per-card or globally? Lean: profile suggests, user can override at layout level.
- **Profile match key** — SNIP manufacturer+model only, or also software/hardware version constraints? Versioned variants may be needed once devices ship firmware changes that alter CDI layout.
- **Read-only first** — the initial status page should not send commands (no emergency-stop, no power-off button). Adding control affordances is a separate decision with its own safety considerations.
- **Layout integration** — the status page is a layout-scoped surface (only nodes on the active connection are shown). It does not need to persist anything per layout in the first cut; the profile bundle is the only source of truth for what to display.
- **Widget catalog scope** — start small: numeric, gauge, on/off LED, enum label, counter, event-indicator. Sparkline / history is tempting but implies time-series storage; defer unless a concrete card needs it.

### Non-goals

- No user-authored cards in the first cut (that is the deferred Option E extension).
- No control / write actions from the status page.
- No history retention beyond what a widget needs for its current render.
- No per-locomotive throttle UI — locomotive statistics from the command station are presentation-only here; throttle work lives in spec 011.

### Related work and references

- `profiles/tower-lcc/` and `profiles/async-blink/` — existing per-model profile bundle layout this idea extends.
- Spec 008 (guided configuration) and the `profile-*` skills — established the profile authoring pipeline this would plug into.
- Spec 011 (consist throttle) — separate; status page only consumes throttle telemetry, does not own throttle control.
- `product/architecture/code-placement-and-ownership.md` — status orchestrator belongs in `app/src/lib/orchestration/**`; polling/IPC in `app/src-tauri/src/**`; widget components in `app/src/lib/components/**`; profile YAML schema and loader in a shared utils/profile layer.
