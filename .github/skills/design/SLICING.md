# Vertical Slice Planning

How to divide a feature into vertical, testable slices for TDD implementation.

Adapted from the tracer-bullet methodology. Each slice is a thin vertical cut through ALL integration layers end-to-end — not a horizontal slice of one layer.

## What Makes a Vertical Slice

A vertical slice:

1. **Cuts through all necessary layers** — from user-visible UI through backend to protocol library, as needed. Not every slice touches every layer, but each delivers a complete path.
2. **Is independently testable** — after implementing this slice, you can write a test that exercises the complete path and verify it passes.
3. **Is independently demoable** — you can show the slice working without needing other slices to be done.
4. **Delivers a narrow but complete behavior** — one user-visible outcome, not "half of two outcomes."

## Anti-Pattern: Horizontal Slicing

Do NOT slice by layer:
- "First build all the stores" → nothing testable until components exist
- "First build the backend commands" → nothing testable until the frontend calls them
- "First build all the tests" → tests verify imagined behavior, not actual

## Bowties Layer Stack

When identifying which layers a slice touches, use this stack:

| Layer | Role |
|-------|------|
| Route | Screen composition, page state |
| Component | Rendering, intent emission |
| Orchestrator | Multi-step async workflow |
| Store | Durable frontend state |
| API | Tauri IPC bindings |
| Backend command | IPC boundary, error translation |
| Backend domain | Node registry, proxy, layout persistence |
| lcc-rs | Protocol semantics, transport, parsing |

A typical slice might touch: Route → Component → Store → API → Backend command → Backend domain. Not every slice needs all layers.

## HITL vs AFK Classification

**HITL** (Human-In-The-Loop): The slice introduces a **new architectural pattern**, creates a **new seam**, or involves a **design trade-off** that requires principle-level judgment. The user reviews the pattern choice before implementation begins.

Examples:
- First slice that establishes a new store ↔ orchestrator ↔ backend pattern
- Slice requiring a new IPC contract shape
- Slice where two viable architectural approaches exist and the choice is load-bearing

**AFK** (Away-From-Keyboard): The slice follows an **established pattern** within known boundaries. The AI implements autonomously.

Examples:
- Additional instances of a pattern established in a prior HITL slice
- Slice extending an existing store with a new field and derivation
- Slice adding a backend command that mirrors an existing one

**Rule of thumb**: The first slice in a new area is usually HITL. Subsequent slices in the same area are usually AFK.

## Slice Ordering Principles

1. **Risk-first**: Put the riskiest architectural assumptions in the earliest slices. If Slice 1 proves the architecture doesn't work, you discover it before investing in Slices 2-N.

2. **Dependencies**: If Slice B requires data or infrastructure from Slice A, Slice A goes first. Express this as "Blocked by: S{N}".

3. **HITL before AFK**: When a HITL slice establishes a pattern that AFK slices replicate, the HITL slice goes first.

4. **Thin before wide**: Start with the thinnest possible vertical path. Subsequent slices widen the behavior.

## Example Slice Breakdown

Feature: "User can view and edit node configuration"

**S1: User can see a node's CDI tree** (HITL — establishes the config read pattern)
- Layers: Route → Component → Orchestrator → Store → API → Backend → lcc-rs
- Test: Connecting to a node and opening config shows the CDI tree structure
- Acceptance: CDI tree renders with correct groups and fields

**S2: User can read a single config value** (AFK — extends S1's pattern)
- Layers: Component → Store → API → Backend → lcc-rs
- Blocked by: S1
- Test: Selecting a field shows its current value read from the node
- Acceptance: Field displays the baseline value from the node

**S3: User can edit a config value** (HITL — introduces the change tracking pattern)
- Layers: Component → Store
- Blocked by: S2
- Test: Editing a field shows the modified value and marks it as changed
- Acceptance: Changed field visually distinct, value reflected in store

**S4: User can write config changes to the node** (AFK — extends S3's write path)
- Layers: Orchestrator → Store → API → Backend → lcc-rs
- Blocked by: S3
- Test: Writing changes sends correct memory config datagrams and updates baseline
- Acceptance: Values persist on the node after write
