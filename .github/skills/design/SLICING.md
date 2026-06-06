# Vertical Slice Planning

How to divide a feature into vertical, testable slices for TDD implementation.

Adapted from the tracer-bullet methodology. Each slice is a thin vertical cut through ALL integration layers end-to-end — not a horizontal slice of one layer.

## What Makes a Vertical Slice

A vertical slice:

1. **Cuts through all necessary layers** — from user-visible UI through backend to protocol library, as needed. Not every slice touches every layer, but each delivers a complete path.
2. **Is independently testable** — after implementing this slice, you can write a test that exercises the complete path and verify it passes.
3. **Is independently demoable** — you can show the slice working without needing other slices to be done. The user or product manager can verify the acceptance criteria.
4. **Delivers a narrow but complete behavior** — one user-visible outcome, not "half of two outcomes."

### Demo Gate

Every slice must answer: **"What can the user see or do after this slice that they couldn't before?"**

If the answer is "nothing new is visible," the slice is not vertical — it is either:
- A horizontal foundation that should be folded into the first downstream slice that produces a visible outcome, OR
- A legitimate `[REFACTOR]` slice (see below) whose acceptance criteria describe what invariant is preserved, not what's new.

### Stub-and-Widen (Preferred S1 Pattern)

When a feature needs backend foundation before UI can render, the first slice should **stub the backend** and wire the full vertical path:

1. S1 stands up a hardcoded/minimal backend that returns a plausible response, wires it through IPC, and renders it in the UI. The user can demo the UI.
2. S2 replaces the hardcoded backend with real implementation. The UI doesn't change; the data becomes real.
3. Subsequent slices widen the behavior.

This is the opposite of building a complete backend first and wiring the UI last. The stub is typically 5–10 lines of hardcoded return values — trivial throwaway cost compared to the integration bugs caught by forcing the full vertical path in S1.

The stub also defines the **IPC contract from the consumer's perspective** before the backend is built. The backend implementation then conforms to a contract already proven to work end-to-end.

## Anti-Pattern: Horizontal Slicing

Do NOT slice by layer:
- "First build all the stores" → nothing testable until components exist
- "First build the backend commands" → nothing testable until the frontend calls them
- "First build all the tests" → tests verify imagined behavior, not actual
- "First build the schema types, then the resolver, then the layout types, then the UI" → five slices of backend-only work before anything is demoable; integration assumptions go unvalidated until the end

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

## HITL vs AFK vs REFACTOR Classification

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

**REFACTOR** (or **MIGRATION**): The slice produces **no user-visible change**. It restructures internals, migrates data formats, or reduces architectural debt while preserving existing behavior. Acceptance criteria describe what invariant is preserved, not what's new.

Examples:
- Re-expressing a profile under a new schema with identical runtime behavior
- Collapsing parallel code paths into a single unified path
- Renaming internal identifiers to match updated vocabulary

**Rule of thumb**: The first slice in a new area is usually HITL. Subsequent slices in the same area are usually AFK. Slices that exist only to pay down debt or migrate formats are REFACTOR.

## Slice Ordering Principles

1. **Integration-risk-first**: The riskiest assumption is almost always "do these layers integrate correctly?" — not "is the schema shape right?" Schema shape is cheap to change; integration shape is expensive. The first slice should prove the integration path end-to-end with the thinnest possible backend (see Stub-and-Widen above).

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
