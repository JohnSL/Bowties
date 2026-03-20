/**
 * T041: connectionRequest.svelte.ts
 * Singleton store for config-first connection requests.
 *
 * TreeLeafRow calls requestConnection() when the user clicks "→ New Connection".
 * +page.svelte listens and switches to 'bowties' tab.
 * BowtieCatalogPanel listens and opens NewConnectionDialog with the appropriate prefill.
 */

import type { ElementSelection } from '$lib/types/bowtie';
import type { EventRole } from '$lib/types/nodeTree';

class ConnectionRequestStore {
  /** Pending connection request, or null when none. */
  pendingRequest = $state<{
    selection: ElementSelection;
    role: EventRole | 'Ambiguous';
  } | null>(null);

  /** Request a new connection from a leaf, switching to the bowties tab. */
  requestConnection(selection: ElementSelection, role: EventRole | 'Ambiguous'): void {
    this.pendingRequest = { selection, role };
  }

  /** Clear the pending request after it has been handled. */
  clearRequest(): void {
    this.pendingRequest = null;
  }
}

export const connectionRequestStore = new ConnectionRequestStore();
