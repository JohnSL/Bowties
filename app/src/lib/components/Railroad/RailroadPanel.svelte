<script lang="ts">
  import FacilitiesSection from '$lib/components/Facilities/FacilitiesSection.svelte';
  import ChannelsPanel from './ChannelsPanel.svelte';
  import type { NodeDisplayParts } from '$lib/utils/nodeDisplayName';
  import type { NodeConfigTree } from '$lib/types/nodeTree';

  let {
    nodeName,
    nodeParts,
    resolvedEventIds,
    daughterboardName,
    usedBy,
    nodeTree,
  }: {
    nodeName: (nodeKey: string) => string;
    /** Resolve structured display parts for a node key. */
    nodeParts?: (nodeKey: string) => NodeDisplayParts;
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
    nodeTree?: (nodeKey: string) => NodeConfigTree | undefined;
  } = $props();
</script>

<div class="railroad-panel" data-testid="railroad-panel">
  <FacilitiesSection
    {resolvedEventIds}
    {nodeTree}
  />
  <ChannelsPanel {nodeName} {nodeParts} {resolvedEventIds} {daughterboardName} {usedBy} {nodeTree} />
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
