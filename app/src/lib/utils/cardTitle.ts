/**
 * FR-007: Card title resolution algorithm
 *
 * Derives a human-readable title for an ElementCard based on:
 *  - The CDI group name (always available)
 *  - The instance index (for replicated groups)
 *  - The user's assigned name from the node's live config values
 *
 * RQ-002: A user name is treated as absent when:
 *  - No string field named "User Name" (or fallback "Name") exists in `fields`
 *  - The config value for that field is not yet loaded
 *  - The stored string is empty, whitespace-only, or consists only of null bytes
 */

import type { CardField } from '$lib/stores/configSidebar';
import type { ConfigValueWithMetadata } from '$lib/api/types';
import { getCacheKey } from '$lib/api/types';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CardTitleParams {
  /** Raw CDI group name */
  cdGroupName: string;
  /** True when group.replication > 1 */
  isReplicated: boolean;
  /** 1-based instance number; null for non-replicated groups */
  instanceIndex: number | null;
  /** All leaf fields in the card, from get_card_elements */
  fields: CardField[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Find the best candidate field for a user-supplied name.
 *
 * Priority order:
 *  1. First string field whose name is "User Name" (case-insensitive)
 *  2. First string field whose name is "Name" (case-insensitive)
 */
function findNameField(fields: CardField[]): CardField | null {
  const lower = (s: string) => s.toLowerCase();
  const stringFields = fields.filter(f => f.dataType === 'string');

  const exact = stringFields.find(f => lower(f.name) === 'user name');
  if (exact) return exact;

  const fallback = stringFields.find(f => lower(f.name) === 'name');
  return fallback ?? null;
}

/**
 * Extract a meaningful string value from configValues for a given field.
 *
 * Returns null when:
 *  - No entry in the map for this (nodeId, elementPath)
 *  - The value type is not String
 *  - The string is empty after stripping null bytes and trimming whitespace (RQ-002)
 */
function resolveUserName(
  field: CardField,
  nodeId: string,
  configValues: Map<string, ConfigValueWithMetadata>,
): string | null {
  const key = getCacheKey(nodeId, field.elementPath);
  const entry = configValues.get(key);
  if (!entry) return null;

  const cv = entry.value;
  if (cv.type !== 'String') return null;

  // Strip null bytes (RQ-002) then trim whitespace
  const cleaned = cv.value.replace(/\x00/g, '').trim();
  return cleaned.length > 0 ? cleaned : null;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Resolve the display title for an ElementCard (FR-007).
 *
 * Title format:
 *  - replicated + named     → "Yard Button (Line 3)"
 *  - replicated + unnamed   → "Line 3 (unnamed)"
 *  - non-replicated + named → "Yard Button (Port I/O)"
 *  - non-replicated + unnamed → "Port I/O"
 */
export function resolveCardTitle(
  card: CardTitleParams,
  nodeId: string,
  configValues: Map<string, ConfigValueWithMetadata>,
): string {
  const { cdGroupName, isReplicated, instanceIndex, fields } = card;

  // Attempt to find a user-supplied name
  const nameField = findNameField(fields);
  const userName = nameField ? resolveUserName(nameField, nodeId, configValues) : null;

  if (isReplicated) {
    const label = `${cdGroupName} ${instanceIndex}`;
    return userName ? `${userName} (${label})` : `${label} (unnamed)`;
  } else {
    return userName ? `${userName} (${cdGroupName})` : cdGroupName;
  }
}
