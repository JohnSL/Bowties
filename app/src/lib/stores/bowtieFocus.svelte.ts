/**
 * bowtieFocus.svelte.ts
 * Singleton store for bowtie focus/navigation from the config page.
 *
 * EventSlotRow and TreeLeafRow call focusBowtie() when the user clicks a
 * "Used in: …" link. +page.svelte listens and switches to the 'bowties' tab.
 * BowtieCatalogPanel watches focusRequest (an object with a nonce) so that
 * re-clicking the same event always triggers the scroll effect, even when the
 * event ID hasn't changed (FR-008, FR-009).
 */

class BowtieFocusStore {
  /**
   * The current focus request. Using an object (not just a string) ensures that
   * calling focusBowtie() with the same ID still triggers reactive $effects,
   * because the object reference always changes.
   */
  focusRequest = $state<{ id: string; nonce: number } | null>(null);

  /** Convenience getter — returns just the event ID hex, or null when none. */
  get highlightedEventIdHex(): string | null {
    return this.focusRequest?.id ?? null;
  }

  /** Focus a bowtie by event ID hex, switching the view to the bowties tab. */
  focusBowtie(eventIdHex: string): void {
    this.focusRequest = { id: eventIdHex, nonce: Date.now() };
  }

  /** Clear the focused bowtie (e.g. on tab change or explicit dismiss). */
  clearFocus(): void {
    this.focusRequest = null;
  }
}

export const bowtieFocusStore = new BowtieFocusStore();
