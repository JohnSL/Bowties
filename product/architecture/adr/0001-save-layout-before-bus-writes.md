# Save layout file before writing config to bus

Online saves currently write config values to the bus first, then save the layout file. This causes blank bowties during save (bus writes trigger draft pruning, which switches the bowtie preview to a stale catalog before the layout file save rebuilds it) and leaves the app in an inconsistent state if the user cancels the Save dialog after writes have already been sent.

The fix is to reorder: save the layout file first (with pending changes staged as offline changes), then write to the bus, then update the layout file with the results. This reuses the existing offline changes infrastructure — no new persistence mechanisms needed. The layout file captures user intent before any irreversible bus communication, cancel is clean (nothing sent to bus), and crash recovery comes free (offline changes in the layout file are detected on next launch).

## Considered Options

- **Option A: "Save-in-progress" flag** — force slow path during save. Rejected: doesn't help on cancel, and the flag is a symptom patch.
- **Option B: Rebuild catalog right after writeModifiedValues** — ensures fresh catalog before draft pruning flips the path. Viable but has a timing race during the async rebuild window, and doesn't address cancel.
- **Option C: Suppress draft pruning during save** — fragile around error paths and cancel.
- **Option E: Catalog-freshness guard** — architecturally clean but complex state tracking. Over-engineered for this problem.
- **Three-phase reorder (chosen):** Eliminates the category of bugs by fixing the ordering. Reuses existing offline changes infrastructure.
