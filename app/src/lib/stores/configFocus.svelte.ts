/**
 * configFocus.svelte.ts
 * Singleton store for navigating from a bowtie entry back to a specific field
 * in the configuration view.
 *
 * BowtieCard's ElementEntry calls focusConfigField() when the user clicks the
 * "← Config" link. +page.svelte listens via `navigationRequest` and switches to
 * the 'config' tab, expanding the node and selecting the correct segment.
 * TreeLeafRow listens via `leafFocusRequest` and scrolls to + focuses the
 * matching input when it becomes visible.
 *
 * The two signals have incompatible lifecycles:
 * - navigationRequest is cleared immediately by +page.svelte (single-tick consumer).
 * - leafFocusRequest is cleared by TreeLeafRow after scroll+focus side-effects
 *   are queued (after the component mounts).
 */

class ConfigFocusStore {
  /**
   * Navigation request consumed by +page.svelte.
   * Cleared immediately after the page effect reads it (single-tick consumer).
   */
  navigationRequest = $state<{ nodeId: string; elementPath: string[] } | null>(null);

  /**
   * Leaf-focus request consumed by TreeLeafRow on mount.
   * Cleared by TreeLeafRow after scroll+focus side-effects are queued.
   */
  leafFocusRequest = $state<{ nodeId: string; elementPath: string[] } | null>(null);

  /**
   * Navigate to a specific config field.
   * Sets both signals; each consumer is responsible for clearing its own.
   */
  focusConfigField(nodeId: string, elementPath: string[]): void {
    this.navigationRequest = { nodeId, elementPath };
    this.leafFocusRequest  = { nodeId, elementPath };
  }

  /** Clear the navigation signal (called by +page.svelte). */
  clearNavigation(): void { this.navigationRequest = null; }

  /** Clear the leaf-focus signal (called by TreeLeafRow). */
  clearLeafFocus(): void { this.leafFocusRequest = null; }

  /** @deprecated Use clearNavigation() or clearLeafFocus() */
  clearFocus(): void {
    this.navigationRequest = null;
    this.leafFocusRequest  = null;
  }
}

export const configFocusStore = new ConfigFocusStore();
