/**
 * Spec 018 / S2 (ADR-0013) — frontend style registry.
 *
 * The channel's `style` field identifies a hardware-shape realisation
 * declared in profile YAML; the YAML carries the constraints (S3) and
 * pin claims, but the producer/consumer event-leaf mapping is the
 * authoritative property a style asserts. This registry mirrors that
 * mapping so callers like `resolveChannelEventIds` and the event-state
 * orchestrator can ask "for THIS style, which leaf index is which state?"
 * without keeping a channelType-keyed constant elsewhere in the code.
 *
 * S5 adds the `single-led-direct-lamp` style (consumer side); S3 may
 * relocate this registry behind a backend IPC if/when constraints follow.
 */

import type { EventMappingEntry } from '$lib/types/connectorProfile';

export type StyleEventMapping = Record<string, EventMappingEntry>;

const STYLE_EVENT_MAPPINGS: Readonly<Record<string, StyleEventMapping>> = Object.freeze({
  'bod-block-detector-input': {
    occupied: { producerLeafIndex: 0 },
    clear: { producerLeafIndex: 1 },
  },
});

/**
 * Return the producer/consumer event-leaf mapping for the given style id,
 * or `undefined` when the style is unknown to the registry.
 */
export function getStyleEventMapping(styleId: string): StyleEventMapping | undefined {
  return STYLE_EVENT_MAPPINGS[styleId];
}
