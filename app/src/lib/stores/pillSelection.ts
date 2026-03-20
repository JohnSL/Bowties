/**
 * Persists pill (instance) selector positions across navigation.
 *
 * Keyed by `nodeId:firstSiblingPath` — stable identifier for a replicated
 * group set. Using the first sibling's full path ensures uniqueness within
 * the tree regardless of which instance is currently selected.
 *
 * Why a separate store: `selectedInstanceIndex` lives in TreeGroupAccordion,
 * which is destroyed when the user navigates to another view and back.
 * Storing it here lets the component restore its last position on remount.
 */
import { writable } from 'svelte/store';

/** Map from stable group key → selected instance index (0-based). */
export const pillSelections = writable<Map<string, number>>(new Map());

/**
 * Persist the selected instance index for a replicated group.
 * @param key  `nodeId:firstSiblingPath` for the replicated set
 * @param index  0-based index of the selected sibling
 */
export function setPillSelection(key: string, index: number): void {
  pillSelections.update(m => {
    m.set(key, index);
    return new Map(m);
  });
}
