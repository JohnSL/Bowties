/**
 * Event State Store — session-scoped, transient, channel-unaware.
 *
 * Records every PCER event received from the LCC bus as eventId → timestamp.
 * Channel state is derived at read-time by comparing timestamps of a channel's
 * known event IDs (occupied vs clear).
 */

/** Map from event ID (16-char hex) to epoch-ms of last occurrence. */
class EventStateStore {
  private _events = $state<Map<string, number>>(new Map());

  /** Record an event occurrence. */
  record(eventId: string, timestampMs: number): void {
    // Always overwrite — we only care about the most recent occurrence.
    this._events = new Map(this._events).set(eventId, timestampMs);
  }

  /** Get the last-seen timestamp for an event ID, or undefined if never seen. */
  lastSeen(eventId: string): number | undefined {
    return this._events.get(eventId);
  }

  /** Reactive snapshot of all recorded events. */
  get events(): ReadonlyMap<string, number> {
    return this._events;
  }

  /** Number of distinct events recorded. */
  get size(): number {
    return this._events.size;
  }

  /** Clear all event state (e.g. on disconnect). */
  clear(): void {
    this._events = new Map();
  }
}

export const eventStateStore = new EventStateStore();
