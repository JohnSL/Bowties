/**
 * `$lib/layout` — the single import surface for layout-state reads and writes
 * from routes and components (ADR-0004).
 *
 * Components must not reach past this module into the four edit-layer stores
 * (`bowtieCatalogStore`, `bowtieMetadataStore`, `configChangesStore`,
 *  `layoutStore`). Those stores keep their current responsibilities but are
 * internal implementation details of the facade and the
 * `saveLayoutOrchestrator`.
 *
 * What lives here:
 *   - `effectiveLayoutStore`         — read model (effectiveBowties,
 *                                       effectiveRole, effectiveValue,
 *                                       slotsByRole, isSlotFree, preview).
 *   - `saveLayoutOrchestrated`       — multi-step save workflow.
 *   - edit-recording commands       — thin wrappers over `bowtieMetadataStore`
 *                                      and `configChangesStore` so callers
 *                                      never import those stores directly.
 *
 * What does NOT live here:
 *   - lifecycle transitions (open / close / disconnect) — `layout.svelte`
 *     still owns those for now; future slices may absorb them into a
 *     dedicated orchestrator.
 *   - rendering. The facade returns data; components render it.
 */

import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import type { TreeConfigValue } from '$lib/types/nodeTree';

// ── Read model ───────────────────────────────────────────────────────────────

export { effectiveLayoutStore } from './effectiveLayoutStore.svelte';

/**
 * `makeValueResolver(nodeId)` returns a `(leaf) → value` closure suitable for
 * passing into label/name helpers such as `buildElementLabel` and
 * `getInstanceDisplayName`. The closure routes through the same waterfall as
 * `effectiveLayoutStore.effectiveValue` (draft → offline pending → baseline)
 * so labels stay consistent with cards and pickers.
 *
 * Re-exported from the internal `displayResolution` helper so components have
 * a single import surface (`$lib/layout`) and never reach into `$lib/utils`
 * for resolution semantics.
 */
export { makeValueResolver } from '$lib/utils/displayResolution';

// ── Save workflow ────────────────────────────────────────────────────────────

export {
  saveLayoutOrchestrated,
  type SaveLayoutOrchestratedArgs,
  type SaveLayoutOrchestratedResult,
} from '$lib/orchestration/saveLayoutOrchestrator';

// ── Edit-recording commands ──────────────────────────────────────────────────

/** Record a pending bowtie deletion. Honoured immediately by the read model. */
export function recordBowtieDeletion(eventIdHex: string): void {
  bowtieMetadataStore.deleteBowtie(eventIdHex);
}

/** Record a pending role classification for a slot. */
export function recordRoleClassification(
  slotKey: string,
  role: 'Producer' | 'Consumer',
): void {
  bowtieMetadataStore.classifyRole(slotKey, role);
}

/** Record a pending config value draft for a single leaf. */
export function recordConfigDraft(editKey: string, value: TreeConfigValue): void {
  configChangesStore.set(editKey, value);
}
