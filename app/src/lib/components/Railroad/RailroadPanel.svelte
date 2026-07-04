<script lang="ts">
  import FacilitiesSection from '$lib/components/Facilities/FacilitiesSection.svelte';
  import ChannelsPanel from './ChannelsPanel.svelte';

  let {
    nodeName,
    resolvedEventIds,
    daughterboardName,
    usedBy,
    onSelectChannel,
    onAddChannel,
    onRemoveFromSlot,
  }: {
    nodeName: (nodeKey: string) => string;
    /**
     * Map from channelId to state-name → eventId (Spec 018 / S5 D6).
     * State names vary by channel role.
     */
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
    /**
     * Resolves the daughterboard display name for a (nodeKey, connector) pair —
     * used in `ChannelsPanel` group headers (e.g. "TowerLCC-1 · Connector A · BOD-8").
     */
    daughterboardName?: (nodeKey: string, connector: string) => string | undefined;
    /**
     * Resolves the facility-slot consumers of a channel for the "Used by"
     * column (Spec 018 / S4). Pass-through to `ChannelsPanel`.
     */
    usedBy?: (channelId: string) => ReadonlyArray<{ facilityName: string; slotLabel: string }>;
    /** Spec 018 / S4 — slot-binding intent emitters; pass-through to `FacilitiesSection`. */
    onSelectChannel?: (facilityId: string, slotLabel: string) => void;
    /** Spec 018 / S5 — consumer-side Add-channel intent emitter. */
    onAddChannel?: (facilityId: string, slotLabel: string) => void;
    onRemoveFromSlot?: (facilityId: string, slotLabel: string, currentChannelId: string) => void;
  } = $props();
</script>

<div class="railroad-panel" data-testid="railroad-panel">
  <FacilitiesSection
    {resolvedEventIds}
    {onSelectChannel}
    {onAddChannel}
    {onRemoveFromSlot}
  />
  <ChannelsPanel {nodeName} {resolvedEventIds} {daughterboardName} {usedBy} />
</div>

<style>
  .railroad-panel {
    padding: 1.25rem;
    overflow-y: auto;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
</style>
