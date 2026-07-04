---
description: Deprecated — merged into tdd-cycle. Do not invoke directly.
name: tdd-red-deprecated
---

# tdd-red — DEPRECATED

This agent has been retired. Red+Green cycles are now handled together in
batches of 1–3 behaviors by the [`tdd-cycle`](tdd-cycle.agent.md) worker,
invoked by the [`tdd-build`](tdd-build.agent.md) coordinator.

If something is still delegating to `tdd-red`, update it to use
`tdd-cycle` with a single-behavior batch.
