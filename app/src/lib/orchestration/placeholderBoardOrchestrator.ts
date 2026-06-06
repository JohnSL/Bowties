/**
 * Placeholder board orchestrator (Spec 014 / S8.10).
 *
 * Owns the in-memory add/delete workflow for placeholder boards. After the
 * S8.10 factory pivot, adding a placeholder calls the backend
 * `add_placeholder_board` IPC which synthesizes a `SynthesizedNodeProxy`,
 * inserts it into the registry, and returns the minted `nodeKey`. The
 * orchestrator then reads the tree via the standard `get_node_tree` IPC
 * (which dispatches through the registry uniformly) and seeds the frontend
 * roster.
 *
 *   - `addPlaceholderBoard` — call the backend factory IPC, read the
 *     pre-built tree via `get_node_tree`, synthesize a `DiscoveredNode`
 *     from the bundled profile's manufacturer/model, and seed the roster.
 *   - `deletePlaceholderBoard` — gated by a frontend `confirm()` callback
 *     (FR-017a). On confirm, removes the placeholder from every in-memory
 *     store.
 *
 * Field edits — including the CDI User Name leaf, which is the only naming
 * surface for placeholders post-pivot — flow through the standard
 * `configChangesStore` path used by real nodes, so no per-field IPC lives
 * here anymore.
 */

import {
  addPlaceholderBoardIpc,
  getNodeTree,
  listBundledProfiles,
} from '$lib/api/layout';
import type { DiscoveredNode } from '$lib/api/tauri';
import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
import { configSidebarStore } from '$lib/stores/configSidebar';
import { get } from 'svelte/store';

export interface AddPlaceholderResult {
  /** The newly minted `placeholder:<uuidv4>` NodeKey. */
  nodeKey: string;
}

/**
 * Add a placeholder board to the active offline layout (in-memory only).
 *
 * Calls the backend `add_placeholder_board` factory IPC, which mints a
 * UUID key, builds the synthesized proxy (CDI, tree, SNIP), and registers
 * it in the backend registry. Then reads the pre-built tree via the
 * standard `get_node_tree` IPC and seeds the frontend roster.
 *
 * @throws if the factory IPC or tree fetch fails.
 */
export async function addPlaceholderBoard(params: {
  profileStem: string;
}): Promise<AddPlaceholderResult> {
  const { profileStem } = params;

  // Resolve manufacturer/model from the bundled-profile listing so the
  // sidebar's `{manufacturer} — {model}` fallback works immediately.
  const profiles = await listBundledProfiles();
  const profile = profiles.find((p) => p.stem === profileStem);
  if (!profile) {
    throw new Error(`UnknownBundledProfile: '${profileStem}' not in bundled profile listing`);
  }

  // Call the backend factory — mints UUID, builds proxy, registers in backend.
  const { nodeKey } = await addPlaceholderBoardIpc(profileStem);

  // Read the pre-built tree via the standard path (dispatches through the
  // registry uniformly — the factory cached the tree on the proxy).
  const tree = await getNodeTree(nodeKey);
  if (!tree) {
    throw new Error(`FactoryTreeMissing: factory registered proxy but getNodeTree returned null for '${nodeKey}'`);
  }

  // Synthesize a `DiscoveredNode`-shaped entry for the frontend roster.
  const nowIso = new Date().toISOString();
  const synthetic: DiscoveredNode = {
    node_id: [],
    alias: 0,
    snip_data: {
      manufacturer: profile.manufacturer,
      model: profile.model,
      hardware_version: '',
      software_version: '',
      user_name: '',
      user_description: '',
    },
    snip_status: 'Complete',
    connection_status: 'Unknown',
    last_verified: nowIso,
    last_seen: nowIso,
    cdi: null,
    pip_flags: null,
    pip_status: 'NotSupported',
  };

  nodeRoster.addPlaceholder({ nodeKey, profileStem, info: synthetic, tree });

  return { nodeKey };
}

/**
 * Remove a placeholder board from the in-memory roster, gated by a
 * frontend `confirm` callback (FR-017a). No IPC fires — the placeholder
 * never reaches disk until Save, and Save just won't see it anymore.
 *
 * Post-delete invariant: the sidebar selection no longer points at the
 * removed key. The orchestrator owns this because the placeholder
 * lifecycle spans multiple frontend stores; the route should not have
 * to know which stores need fixup after a delete.
 *
 * @returns `true` when the placeholder was removed; `false` when the user
 *          declined the confirmation or the key was not a known placeholder.
 */
export async function deletePlaceholderBoard(params: {
  nodeKey: string;
  confirm: () => Promise<boolean>;
}): Promise<boolean> {
  const { nodeKey } = params;
  if (!nodeRoster.has(nodeKey)) return false;
  const proceed = await params.confirm();
  if (!proceed) return false;

  nodeRoster.removePlaceholder(nodeKey);

  const sidebar = get(configSidebarStore);
  const selected = sidebar.selectedSegment?.nodeId ?? sidebar.selectedNodeId;
  if (selected === nodeKey) configSidebarStore.setSelectedNode(null);

  return true;
}
