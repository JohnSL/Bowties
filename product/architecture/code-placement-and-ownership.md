# Code Placement And Ownership

## Purpose

This document is the current rule set for deciding where new Bowties logic belongs.

Use it when deciding whether code should live in the frontend, the Tauri backend, or `lcc-rs`, and when choosing between routes, components, orchestrators, stores, and shared utilities inside the frontend.

## Core Rule

Put logic in the narrowest layer that can own it without leaking unrelated concerns across boundaries.

- Keep presentation concerns in the frontend.
- Keep authoritative application state, filesystem access, and app workflow coordination in the backend.
- Keep reusable LCC/OpenLCB protocol behavior in `lcc-rs`.
- Do not create parallel owners for the same workflow, normalization rule, or protocol rule.

## Placement Matrix

| Concern | Primary owner | Belongs elsewhere when... |
|---|---|---|
| Screen composition, visible page state, dialog/page coordination | `app/src/routes/**` | It becomes reusable rendering or a multi-step workflow |
| Rendering, local interaction, emitted intent, minimal display derivation | `app/src/lib/components/**` | It starts owning async sequencing, backend coordination, or durable state |
| Multi-step async workflows, lifecycle transitions, backend call sequencing, cross-store coordination | `app/src/lib/orchestration/**` | The logic is actually a pure state transition or a protocol/backend concern |
| Durable frontend state, deterministic state transitions, derived state | `app/src/lib/stores/**` | The logic becomes multi-step workflow sequencing or pure reusable helper logic |
| Pure normalization, formatting, comparison, parsing, translation helpers | `app/src/lib/utils/**` | The helper needs store mutation, backend calls, or protocol/runtime side effects |
| IPC boundaries, filesystem access, authoritative app state, layout persistence, node registry coordination, backend workflows | `app/src-tauri/src/**` | The behavior is actually reusable LCC/OpenLCB protocol logic |
| Protocol semantics, transport behavior, discovery rules, alias handling, frame/datagram parsing, reusable wire-level helpers | `lcc-rs/**` | The code only exists to shape Bowties application behavior or UI workflows |

## Frontend Placement Rules

### Routes

Use routes for:

- page composition
- visible page state
- URL, screen, and dialog coordination
- handing user intent to orchestrators or stores

Do not use routes for:

- multi-step backend workflows
- protocol sequencing
- duplicate normalization or fallback rules

### Components

Use components for:

- rendering
- local interaction handling
- emitting intent events
- small display-only derivations

Do not use components for:

- retry loops
- lifecycle orchestration
- cross-store workflow sequencing
- backend ownership decisions

### Orchestrators

Use orchestrators for:

- discovery, connect, replay, apply, discard, and similar multi-step workflows
- lifecycle-sensitive transitions
- ordering backend calls and store updates
- enforcing workflow guard conditions

Do not use orchestrators for:

- owning durable state that should live in stores
- reimplementing pure normalization helpers

### Stores

Use stores for:

- durable frontend state
- deterministic transitions
- derived values needed by the UI

Do not use stores for:

- broad async workflow sequencing unless the store is the explicitly documented owner
- scattered copies of the same transition logic

### Utils

Use utilities for:

- Node ID normalization
- display-name fallback rules
- formatting and comparison helpers
- pure translation logic reused across multiple layers

Do not use utilities for:

- backend calls
- store mutation
- hidden workflow side effects

## Backend Vs `lcc-rs`

Put code in the Tauri backend when it is Bowties application logic, including:

- IPC command handling
- app-level workflow and state coordination
- layout and file persistence
- backend ownership of node registries, proxies, or caches
- adapting protocol-library data into app-specific responses

Put code in `lcc-rs` when it expresses reusable protocol behavior, including:

- message/frame/datagram parsing or encoding
- alias and discovery semantics
- transport behavior
- reusable protocol data structures and helpers

If a rule would matter to another LCC/OpenLCB consumer, prefer `lcc-rs`.

If a rule exists only because of Bowties UI, layout model, or application workflow, keep it out of `lcc-rs`.

## Decision Questions

When placement is unclear, ask these in order:

1. Is this reusable LCC/OpenLCB protocol behavior rather than Bowties app behavior? If yes, put it in `lcc-rs`.
2. Is this authoritative app state, filesystem persistence, IPC behavior, or backend coordination? If yes, put it in `app/src-tauri/src/**`.
3. Is this a multi-step UI workflow spanning stores, backend calls, or lifecycle transitions? If yes, put it in `app/src/lib/orchestration/**`.
4. Is this durable frontend state or a deterministic transition? If yes, put it in `app/src/lib/stores/**`.
5. Is this page-level composition or visible screen state? If yes, put it in `app/src/routes/**`.
6. Is this rendering and emitted user intent? If yes, put it in `app/src/lib/components/**`.
7. Is this pure reusable normalization, formatting, or comparison logic? If yes, put it in `app/src/lib/utils/**`.

## Design Principles In Practice

- Apply SOLID by keeping one clear owner for each workflow, state transition, or protocol rule.
- Apply DRY by centralizing shared normalization, fallback, formatting, and sequencing rules.
- Apply YAGNI by introducing the smallest explicit abstraction that solves the current ownership problem.
- Apply TDD by adding or updating the closest focused test for the owning layer before or alongside the change.

## Review Triggers

Stop and re-check placement when a change:

- duplicates a normalization or fallback rule
- adds the same workflow logic in two layers
- moves backend or protocol behavior into UI code
- adds UI-specific assumptions into `lcc-rs`
- puts multi-step async sequencing into a route, component, or utility helper