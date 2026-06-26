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
import type { EventMappingEntry } from '$lib/types/connectorProfile';

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
 * @param channels - The channels to resolve
 * @param eventMapping - The profile-declared event mapping (state → producerLeafIndex)
 * @returns Map from channelId → { occupied?: eventId, clear?: eventId }
 */
export async function resolveChannelEventIds(
  channels: InformationChannel[],
  eventMapping: Record<string, EventMappingEntry>,
): Promise<ReadonlyMap<string, { occupied?: string; clear?: string }>> {
  // Convert eventMapping to the flat form the backend expects (state → leafIndex)
  const flatMapping: Record<string, number> = {};
  for (const [state, entry] of Object.entries(eventMapping)) {
    flatMapping[state] = entry.producerLeafIndex;
  }

  const requests: ChannelResolutionRequest[] = channels.map((ch) => ({
    channelId: ch.id,
    nodeKey: ch.hardwareRef.nodeKey,
    connector: ch.hardwareRef.connector,
    input: ch.hardwareRef.input,
    eventMapping: flatMapping,
  }));

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
