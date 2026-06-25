import { listChannels, type InformationChannel } from '$lib/api/channels';

class ChannelsStore {
  /** Channels loaded from disk (baseline). */
  private _baseline = $state<InformationChannel[]>([]);
  /** Channels created in-memory since last save (drafts). */
  private _pendingCreations = $state<InformationChannel[]>([]);
  /** Pending renames: channel ID → new name. */
  private _pendingRenames = $state<Map<string, string>>(new Map());
  /** IDs of baseline channels pending deletion. */
  private _pendingDeletions = $state<Set<string>>(new Set());
  private _loading = $state(false);

  /** All channels: baseline (minus deletions) + pending creations, with pending renames applied. */
  get channels(): InformationChannel[] {
    const base = this._pendingDeletions.size > 0
      ? this._baseline.filter((ch) => !this._pendingDeletions.has(ch.id))
      : this._baseline;
    const raw = [...base, ...this._pendingCreations];
    if (this._pendingRenames.size === 0) return raw;
    return raw.map((ch) => {
      const newName = this._pendingRenames.get(ch.id);
      return newName !== undefined ? { ...ch, name: newName } : ch;
    });
  }

  get loading(): boolean {
    return this._loading;
  }

  get isEmpty(): boolean {
    return this.channels.length === 0;
  }

  /** Group channels by their channelType. */
  get grouped(): Map<string, InformationChannel[]> {
    const map = new Map<string, InformationChannel[]>();
    for (const ch of this.channels) {
      const group = map.get(ch.channelType) ?? [];
      group.push(ch);
      map.set(ch.channelType, group);
    }
    return map;
  }

  // ── ADR-0012: Draft lifecycle ───────────────────────────────────────────

  /** Whether there are unsaved channel changes (creations, renames, or deletions). */
  get isDirty(): boolean {
    return this._pendingCreations.length > 0 || this._pendingRenames.size > 0 || this._pendingDeletions.size > 0;
  }

  /** Count of pending channel edits (creations + renames + deletions). */
  get editCount(): number {
    return this._pendingCreations.length + this._pendingRenames.size + this._pendingDeletions.size;
  }

  /** Return the pending channels to persist at save time. */
  get pendingCreations(): InformationChannel[] {
    return this._pendingCreations;
  }

  /** Return the pending renames to flush at save time. */
  get pendingRenames(): Map<string, string> {
    return this._pendingRenames;
  }

  /** Return the IDs of baseline channels pending deletion for save flush. */
  get pendingDeletions(): Set<string> {
    return this._pendingDeletions;
  }

  /** Revert all pending channel edits (discard). */
  discard(): void {
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
  }

  /**
   * After a successful save, the backend returns the full channel list
   * which becomes the new baseline; pending edits are cleared.
   */
  hydrateBaseline(channels: InformationChannel[]): void {
    this._baseline = channels;
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
  }

  // ── Operations ──────────────────────────────────────────────────────────

  /** Load channels from backend. Called during layout open. */
  async loadChannels(): Promise<void> {
    this._loading = true;
    try {
      this._baseline = await listChannels();
      this._pendingCreations = [];
      this._pendingRenames = new Map();
      this._pendingDeletions = new Set();
    } finally {
      this._loading = false;
    }
  }

  /**
   * Add channels to the pending creations (in-memory only).
   * Called by the orchestrator's step 4 after auto-create.
   */
  addPendingChannels(channels: InformationChannel[]): void {
    this._pendingCreations = [...this._pendingCreations, ...channels];
  }

  /**
   * Rename a channel in-memory. Empty/whitespace-only names are rejected.
   * No-op if the new name equals the current effective name (ADR-0012).
   * If the new name matches the baseline name, the pending rename is removed
   * (the user reverted their edit).
   * Returns true if the rename was accepted, false if rejected or no-op.
   */
  renameChannel(id: string, newName: string): boolean {
    const trimmed = newName.trim();
    if (trimmed.length === 0) return false;
    // ADR-0012: suppress no-op renames against the effective view
    const effective = this.channels.find((ch) => ch.id === id);
    if (effective && effective.name === trimmed) return false;
    // If the user reverted to the baseline name, remove the pending rename
    const baseline = this._baseline.find((ch) => ch.id === id);
    if (baseline && baseline.name === trimmed) {
      if (this._pendingRenames.has(id)) {
        const next = new Map(this._pendingRenames);
        next.delete(id);
        this._pendingRenames = next;
      }
      return true;
    }
    this._pendingRenames = new Map(this._pendingRenames).set(id, trimmed);
    return true;
  }

  /**
   * Mark channels for deletion (in-memory only, per ADR-0012).
   * Channels that are pending creations are removed immediately;
   * baseline channels are tracked in _pendingDeletions for save flush.
   */
  deleteChannels(ids: string[]): void {
    const idSet = new Set(ids);
    // Remove any that are still in pending creations (never persisted)
    const removedFromCreations = this._pendingCreations.filter((ch) => idSet.has(ch.id));
    if (removedFromCreations.length > 0) {
      this._pendingCreations = this._pendingCreations.filter((ch) => !idSet.has(ch.id));
    }
    // Track baseline channels as pending deletions
    const baselineIds = this._baseline.filter((ch) => idSet.has(ch.id)).map((ch) => ch.id);
    if (baselineIds.length > 0) {
      this._pendingDeletions = new Set([...this._pendingDeletions, ...baselineIds]);
    }
    // Clean up any renames for deleted channels
    if (this._pendingRenames.size > 0) {
      const nextRenames = new Map(this._pendingRenames);
      for (const id of ids) {
        nextRenames.delete(id);
      }
      this._pendingRenames = nextRenames;
    }
  }

  /** Set channels directly (used after save hydration). */
  setChannels(channels: InformationChannel[]): void {
    this._baseline = channels;
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
  }

  /** Clear all channel state. Called on layout close. */
  reset(): void {
    this._baseline = [];
    this._pendingCreations = [];
    this._pendingRenames = new Map();
    this._pendingDeletions = new Set();
    this._loading = false;
  }
}

export const channelsStore = new ChannelsStore();
