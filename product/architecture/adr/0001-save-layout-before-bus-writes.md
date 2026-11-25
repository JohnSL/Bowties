# Save layout file before writing config to bus

Online saves currently write config values to the bus first, then save the layout file. This causes blank bowties during save (bus writes trigger draft pruning, which switches the bowtie preview to a stale catalog before the layout file save rebuilds it) and leaves the app in an inconsistent state if the user cancels the Save dialog after writes have already been sent.

The fix is to reorder: save the layout file first (with pending changes staged as offline changes), then write to the bus, then update the layout file with the results. This reuses the existing offline changes infrastructure — no new persistence mechanisms needed. The layout file captures user intent before any irreversible bus communication, cancel is clean (nothing sent to bus), and crash recovery comes free (offline changes in the layout file are detected on next launch).

## Considered Options

- **Option A: "Save-in-progress" flag** — force slow path during save. Rejected: doesn't help on cancel, and the flag is a symptom patch.
- **Option B: Rebuild catalog right after writeModifiedValues** — ensures fresh catalog before draft pruning flips the path. Viable but has a timing race during the async rebuild window, and doesn't address cancel.
- **Option C: Suppress draft pruning during save** — fragile around error paths and cancel.
- **Option E: Catalog-freshness guard** — architecturally clean but complex state tracking. Over-engineered for this problem.
- **Three-phase reorder (chosen):** Eliminates the category of bugs by fixing the ordering. Reuses existing offline changes infrastructure.

## 2026-11-22 extension: facilities reach hardware through Save

Automatic Block Signals (Spec 020) is the first facility that generates
Signal LCC logic (mast, Rule to Aspect, conditionals). It reaches the
board through this ADR's route, as Block Indicator already does:
`facilityOrchestrator` stages the compiled values as unsaved configuration
edits through `configEditor.applyEdit`. Save writes the layout file first
and then, when connected, writes those values to the bus. An offline Save
stores them as Offline Changes, which the Sync Panel writes after
reconnecting.

Considered and rejected for the first applied release: a separate Apply
action with an immutable reviewed proposal, snapshot generations and its
own recovery engine (Spec 020 plan, 2026-11-07 assessment). It would be a
second write path beside Save and the Sync Panel, and a large new seam
before any signal lights. Revisit only if multi-node partial failure
cannot be reported truthfully through the existing writers. Readback
verification extends those writers rather than replacing them.

## 2026-11-23 extension: every Save writes configuration through Pending Changes (Spec 020 E3b-1)

**Problem.** A connected Save reached the bus through a second path: the draft mirror (`configDraftMirrorOrchestrator` → `set_modified_value`) filled a `modified_value` layer on the node tree, and Phase 2 (`write_modified_values`) wrote those leaves. Drafts became rows only on an offline Save, so edits made offline and saved after connecting could be lost, and a failed write left no row behind.

**Decision.**
- Every Save, connected or not, records configuration drafts as Pending Change rows before the Save IPC (`pendingChangesStore.recordDraftsToBackend`, one ID-preserving `set_pending_change` upsert per draft). Drafts are cleared only after the Save IPC succeeds.
- `save_layout_with_bus_writes` takes the change IDs this Save recorded. Phase 1 saves the layout with the rows; Phase 2 (connected) runs `bowties-core` `sync::writer::write_recorded_changes` through the Sync Panel's `LivePort` adapter. The Save writes only those rows (D1, as JMRI CdiPanel "Save Changes"); rows recorded earlier, skipped in the Sync Panel or left failed belong to the Sync Panel. The result reports written / failed / read-only / waiting counts and errors. Save never opens the Sync Panel.
- Each recorded field is written once, then each written node gets one Update Complete, as before E3b-1. There is no pre-read and no readback: the bench run showed the Signal LCC restarting about a second after Update Complete, and readback timed out. Verification (readback timing relative to Update Complete, waiting for a restarting board, pre-read, refused values) belongs to Spec 020 E4.
- The shared planner `sync::read_plan::plan_reads` (at most 64 bytes, gaps of 8 bytes or less merged, as JMRI `RangeCacheUtil` / `MemorySpaceCache`) is used by CDI reads and is where E4 readback will batch.
- Results are row statuses: written → row removed, leaf and snapshot baseline updated; write error → `failed` with the reason; 0x1083 → row cleared, field read-only. The Pending Change row is the only record of a write result (no leaf write state).
- `apply_write_effects` (Save and Sync Panel) emits `node-tree-updated` for every node whose tree it changed, and a connected Save clears drafts only after those trees are reloaded (`saveLayoutOrchestrator` `reloadNodeTrees`), so a field shows the committed value and the next edit starts from it.
- A row the Sync Panel finds already on the board is committed like a write (leaf and snapshot baseline, through `apply_write_effects`) and counted in the panel's "already on the board" notice; it is never dropped silently.

**Considered and rejected.** Keeping `replace_pending_changes` as the Save flush: it regenerates every ID and resets statuses, so a Save could not name the rows it recorded and would drop earlier failed rows. Writing every waiting row on Save: it would bypass the Sync Panel's conflict review for offline rows.

**Invariants added.**
- One writer sends configuration to the bus: `sync::writer` (`apply_changes` for the Sync Panel, `write_recorded_changes` for Save). No other code path calls memory write for configuration values.
- A connected Save writes only the rows it recorded in the same Save.
- Until E4, "written" means the board acknowledged the write, not that a readback confirmed it.

## 2026-11-24 extension: a facility's outcome on each board (Spec 020 E5)

**Problem.** A facility's signals may be on two boards, and one board's writes can fail while the other's succeed. The facility card showed "Wired" from its slot bindings alone, so a facility whose rows were still waiting read as complete.

**Decision.**
- A Pending Change row records the facility that staged it: optional `origin { facilityId }` (`bowties-core/src/layout/pending_changes.rs`, serde default, not written when absent; schema version unchanged). The facility staging pipeline (`facilityOrchestrator` `stageComposedEdits` / `applyConfigEdits`, for create, edit, release and removal) tags its drafts; `configChangesStore` keeps the tag beside the draft and `recordDraftsToBackend` sends it. A hand edit of the same field records the row with no origin (the upsert takes the incoming origin).
- Each board's outcome is derived from the rows: `utils/facilityWriteOutcome.ts` groups a facility's rows by node (waiting with a reason, not yet written) through `isWaitingPendingChange`; `effectiveLayoutStore.facilityWriteOutcome` adds the board name. A written row is removed, so a board with nothing waiting has no line. The facility card shows "Waiting" and one line per board while any row waits. `facilityStatus` (slot bindings, which gates bowtie composition) is unchanged.
- The writer is unchanged: it carries on past a failed row and sends Update Complete to each board it wrote. Partial failure is reported truthfully through the rows, so the separate Apply path stays rejected.

**Considered and rejected.** Matching rows to the facility's claimed slots (misses release edits outside the slots and needs CDI address resolution); per-board counts in the writer result (a status beside the rows, lost on reopen).

**Invariants added.**
- A facility's write outcome is read from rows whose `origin` names it; no other record of it exists.
