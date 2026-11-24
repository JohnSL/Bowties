/**
 * syncReviewRequest.svelte.ts
 * Intent signal: "open the Sync Panel to review waiting Pending Changes".
 *
 * Deeply nested components (the toolbar's waiting line, a field's "Not written"
 * marker) emit this request instead of owning the Sync Panel. +page.svelte
 * consumes it and runs the same entry point as File > Sync to Bus.
 * Same shape as `configFocusStore.navigationRequest`: a single-tick signal the
 * consumer clears.
 */

class SyncReviewRequestStore {
  /** Set by a request; the page effect reacts and clears it. */
  pending = $state(false);

  request(): void { this.pending = true; }

  clear(): void { this.pending = false; }
}

export const syncReviewRequestStore = new SyncReviewRequestStore();
