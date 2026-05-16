# Architecture Health

Coupling risks, depth assessments, and architecture debt discovered during feature work. This file grows incrementally — add entries as issues are found.

## Format

Each entry:
- **Area**: affected modules/layers
- **Risk**: what could go wrong
- **Evidence**: where the issue was observed
- **Suggested action**: fix or investigation needed

---

## Entries

### Reset callback consistency across layout orchestrator functions
- **Area**: `offlineLayoutOrchestrator.ts`, `+page.svelte`
- **Risk**: When a new reset function is added or an existing one is modified, it's easy to forget a callback (e.g. `resetSidebar` was missing from two of three reset paths). The set of stores that need clearing on layout transitions is implicit — there's no checklist or compile-time enforcement.
- **Evidence**: `resetLayoutStateForNoLayout` and `openOfflineLayoutWithReplay` both forgot to clear the config sidebar while `resetFreshLiveSessionState` included it. Fixed May 2026.
- **Suggested action**: When adding new store state that must be cleared on layout transitions, check all three reset functions and their tests. Consider adding a comment in the orchestrator listing the full set of reset paths for cross-reference.
