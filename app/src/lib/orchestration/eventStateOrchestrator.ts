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

interface ChannelResolutionBinding {
  kind: 'connectorInput' | 'lampRow';
  connector?: string;
  input?: number;
  rowOrdinal?: number;
}

interface ChannelResolutionRequest {
  channelId: string;
  nodeKey: string;
  binding: ChannelResolutionBinding;
  role: 'producer' | 'consumer';
  /** State-name → leaf ordinal (producerLeafIndex / consumerLeafIndex). */
  leafIndexMap: Record<string, number>;
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
 * sourced from the channel's `style` field via the style registry. Spec 018
 * / S5 (D6) generalises the IPC payload to (binding, role, leafIndexMap)
 * so consumer-side lamp-indicator channels resolve through the same shape-
 * agnostic backend resolver as producer-side block-occupancy channels.
 * Channels whose style is unknown to the registry are silently skipped.
 *
 * @returns Map from `channelId` → state-name → eventId. State names vary by
 * role: `block-occupancy` produces `{occupied, clear}`; `lamp-indicator`
 * produces `{lit, unlit}`.
 */
export async function resolveChannelEventIds(
  channels: InformationChannel[],
): Promise<ReadonlyMap<string, Record<string, string>>> {
  const requests: ChannelResolutionRequest[] = [];
  for (const ch of channels) {
    const mapping = getStyleEventMapping(ch.style);
    if (!mapping) continue;

    const role: 'producer' | 'consumer' = ch.role === 'lamp-indicator' ? 'consumer' : 'producer';
    const leafIndexMap: Record<string, number> = {};
    for (const [state, entry] of Object.entries(mapping)) {
      const idx = role === 'consumer' ? entry.consumerLeafIndex : entry.producerLeafIndex;
      if (idx === undefined) continue;
      leafIndexMap[state] = idx;
    }
    if (Object.keys(leafIndexMap).length === 0) continue;

    let binding: ChannelResolutionBinding;
    if (ch.binding.kind === 'connectorInput') {
      binding = {
        kind: 'connectorInput',
        connector: ch.binding.connector,
        input: ch.binding.input,
      };
    } else if (ch.binding.kind === 'lampRow') {
      binding = {
        kind: 'lampRow',
        rowOrdinal: ch.binding.rowOrdinal,
      };
    } else {
      continue;
    }

    requests.push({
      channelId: ch.id,
      nodeKey: ch.binding.nodeKey,
      binding,
      role,
      leafIndexMap,
    });
  }

  const results = await invoke<ChannelResolutionResult[]>('resolve_channel_event_ids', {
    requests,
  });

  const resolved = new Map<string, Record<string, string>>();
  for (const r of results) {
    resolved.set(r.channelId, r.eventIds);
  }
  return resolved;
}
