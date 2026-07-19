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
  // Spec 018 / S5 — consumer-side mapping. Lit/unlit map to the Direct Lamp
  // Control row's two EventId consumer leaves (Lamp On is leaf #0, Lamp Off
  // is leaf #1). If the Signal-LCC CDI ever flips ordering, flip these.
  'single-led-direct-lamp': {
    lit: { consumerLeafIndex: 0 },
    unlit: { consumerLeafIndex: 1 },
  },
  // Spec 020 / S3 — 2-LED bicolor signal aspect. Each lamp row has 2 consumer
  // event leaves (On, Off). Two rows (red LED, green LED) are concatenated:
  // row 0 → leaves 0,1 (redOn, redOff), row 1 → leaves 2,3 (greenOn, greenOff).
  // The signal-aspect state is derived from the most-recent On/Off events.
  '2-led-bicolor-aspect': {
    redOn: { consumerLeafIndex: 0 },
    redOff: { consumerLeafIndex: 1 },
    greenOn: { consumerLeafIndex: 2 },
    greenOff: { consumerLeafIndex: 3 },
  },
});

/**
 * Return the producer/consumer event-leaf mapping for the given style id,
 * or `undefined` when the style is unknown to the registry.
 */
export function getStyleEventMapping(styleId: string): StyleEventMapping | undefined {
  return STYLE_EVENT_MAPPINGS[styleId];
}

/**
 * Number of Direct Lamp Control rows a style claims. `single-led-direct-lamp`
 * claims 1 row; `2-led-bicolor-aspect` claims 2 consecutive rows. Returns 0
 * for unknown or non-lamp styles.
 */
const STYLE_ROW_COUNTS: Readonly<Record<string, number>> = Object.freeze({
  'single-led-direct-lamp': 1,
  '2-led-bicolor-aspect': 2,
});

export function getStyleRowCount(styleId: string): number {
  return STYLE_ROW_COUNTS[styleId] ?? 0;
}
