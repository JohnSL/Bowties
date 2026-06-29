/**
 * Event State Orchestrator — subscribes to lcc-event-state Tauri events
 * and records them in the event state store.
 *
 * Lifecycle:
 * - `startListening()` → subscribes to backend PCER events, returns teardown
 * - Call teardown on disconnect (clears store + removes listener)
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { eventStateStore } from '$lib/stores/eventState.svelte';
import type { InformationChannel } from '$lib/api/channels';
import { getStyleEventMapping } from '$lib/utils/channelStyles';

interface EventStatePayload {
  eventId: string;
  timestamp: string;
}

interface ChannelResolutionRequest {
  channelId: string;
  nodeKey: string;
  connector: string;
  input: number;
  eventMapping: Record<string, number>;
}

interface ChannelResolutionResult {
  channelId: string;
  eventIds: Record<string, string>;
}

/**
 * Start listening for lcc-event-state events from the backend.
 * Records each event in the event state store.
 *
 * @returns A teardown function that removes the listener and clears the store.
 */
export async function startEventStateListening(): Promise<UnlistenFn> {
  const unlisten = await listen<EventStatePayload>('lcc-event-state', (event) => {
    const { eventId, timestamp } = event.payload;
    // Convert ISO timestamp to epoch-ms for fast numeric comparison
    const ms = new Date(timestamp).getTime();
    eventStateStore.record(eventId, ms);
  });

  return () => {
    unlisten();
    eventStateStore.clear();
  };
}

/**
 * Resolve event IDs for a batch of channels via the backend.
 *
 * Spec 018 / S2 (ADR-0013): the producer/consumer event-leaf mapping is
 * sourced from the channel's `style` field via the style registry, not
 * from a single per-call mapping argument. Channels whose style is unknown
 * to the registry, or whose binding does not address a per-input target
 * (i.e. not `connectorInput`), are silently skipped.
 *
 * @param channels - The channels to resolve
 * @returns Map from channelId → { occupied?: eventId, clear?: eventId }
 */
export async function resolveChannelEventIds(
  channels: InformationChannel[],
): Promise<ReadonlyMap<string, { occupied?: string; clear?: string }>> {
  const requests: ChannelResolutionRequest[] = [];
  for (const ch of channels) {
    if (ch.binding.kind !== 'connectorInput') continue;
    const mapping = getStyleEventMapping(ch.style);
    if (!mapping) continue;
    const flatMapping: Record<string, number> = {};
    for (const [state, entry] of Object.entries(mapping)) {
      flatMapping[state] = entry.producerLeafIndex;
    }
    requests.push({
      channelId: ch.id,
      nodeKey: ch.binding.nodeKey,
      connector: ch.binding.connector,
      input: ch.binding.input,
      eventMapping: flatMapping,
    });
  }

  const results = await invoke<ChannelResolutionResult[]>('resolve_channel_event_ids', {
    requests,
  });

  const resolved = new Map<string, { occupied?: string; clear?: string }>();
  for (const r of results) {
    resolved.set(r.channelId, {
      occupied: r.eventIds['occupied'],
      clear: r.eventIds['clear'],
    });
  }
  return resolved;
}
