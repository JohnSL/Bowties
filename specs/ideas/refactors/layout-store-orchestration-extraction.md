# Extract Orchestration Logic from layout.svelte.ts

- **Areas**: architecture, cleanup, stores, orchestration
- **Origin**: spec 013 architecture assessment (F3)
- **Status**: deferred
- **Date**: 2026-05-17

`layout.svelte.ts` currently owns dialog opening (`open()`, `saveAs()`), IPC call sequencing, and recent-layout orchestration — concerns that belong in orchestrators per `product/architecture/code-placement-and-ownership.md`. Legacy file-mode methods (`openLayout`, `saveCurrentLayout`, `saveLayoutAs`) appear to be dead code since the offline directory flow in `+page.svelte` bypasses them.

## Prior Work

- **Assessment**: The store mixes state ownership (correct) with multi-step dialog + IPC sequencing (incorrect placement). The store should own `activeContext`, `filePath`, `isDirty`, `isBusy`, and connector selection mutations. Dialog sequencing, IPC calls, and `checkAndReopenRecent()` should move to orchestrators.
- **Dead code candidates**: `openLayout()`, `saveCurrentLayout()`, `saveLayoutAs()` — the legacy file-mode methods. Verify with usage search before removing.
- **Blocked by**: spec 013 slices S1/S6 will add a `startupOrchestrator.ts` and refactor the save flow to use `saveLayoutOrchestrator`. After those land, the remaining orchestration logic in the store becomes more obviously extractable.
- **Risk**: Low — the store's orchestration methods are self-contained and testable. Extraction is mechanical once the save and startup flows are cleaned up.
