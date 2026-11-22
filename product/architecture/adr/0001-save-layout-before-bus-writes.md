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
