<script lang="ts">
  /**
   * AddChannelPicker — Spec 018 / S5.
   *
   * Modal picker for the consumer-side Add channel flow. Lists
   * unclaimed Direct Lamp Control rows on connected Signal-LCC nodes
   * (derived by `effectiveLayoutStore.eligibleLampRowsForStyle` and
   * supplied through `candidateGroups`). Rows are rendered under a
   * per-node section header so the user can tell which Signal-LCC a
   * row lives on without each row repeating the node name. On confirm
   * the caller dispatches `facilityOrchestrator.addChannelForSlot` —
   * this component emits intent only, never reaches the orchestrator
   * directly.
   *
   * Post-add user-configuration prerequisites (e.g. "the user must set
   * Lamp Selection to a pin before the lamp will light") are NOT
   * surfaced here; that concern belongs to a profile-language extension
   * tracked in `specs/backlog.md` under "Profile-declared
   * user-configuration prerequisites for consumer channels". Interim
   * discoverability lives in `docs/user/` and the release notes.
   */
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';
  import NodeLabel from '$lib/components/NodeLabel.svelte';
  import SingleSelectList from '$lib/components/SingleSelectList/SingleSelectList.svelte';
  import type { NodeDisplayParts } from '$lib/utils/nodeDisplayName';

  export interface CandidateRow {
    nodeKey: string;
    nodeName: string;
    rowOrdinal: number;
    rowLabel: string;
  }

  export interface CandidateGroup {
    nodeKey: string;
    nodeName: string;
    nodeParts?: NodeDisplayParts;
    rows: CandidateRow[];
  }

  let {
    slotLabel,
    requiredRole: _requiredRole,
    requiredStyle: _requiredStyle,
    candidateGroups,
    onConfirm,
    onCancel,
  }: {
    slotLabel: string;
    /** Role the slot requires (e.g. `'lamp-indicator'`, `'signal-aspect'`). */
    requiredRole: string;
    /** Style that the new channel will adopt (e.g. `'single-led-direct-lamp'`, `'2-led-bicolor-aspect'`). */
    requiredStyle: string;
    candidateGroups: CandidateGroup[];
    onConfirm: (lampRowNodeKey: string, rowOrdinal: number) => void;
    onCancel: () => void;
  } = $props();

  const dialogTitle = $derived(`Add channel to '${slotLabel}'`);

  let selectedKey = $state<string | undefined>(undefined);
  let searchText = $state('');

  const totalRowCount = $derived(
    candidateGroups.reduce((acc, g) => acc + g.rows.length, 0),
  );

  const filteredGroups = $derived.by(() => {
    const q = searchText.trim().toLowerCase();
    if (q.length === 0) return candidateGroups;
    const out: CandidateGroup[] = [];
    for (const group of candidateGroups) {
      const groupMatches = group.nodeName.toLowerCase().includes(q);
      const rows = groupMatches
        ? group.rows
        : group.rows.filter((row) => row.rowLabel.toLowerCase().includes(q));
      if (rows.length > 0) {
        out.push({ ...group, rows });
      }
    }
    return out;
  });

  const filteredRowCount = $derived(
    filteredGroups.reduce((acc, g) => acc + g.rows.length, 0),
  );

  /** Flat items list with group headers interleaved for SingleSelectList. */
  const listItems = $derived.by(() => {
    const out: { key: string; isHeader?: boolean }[] = [];
    for (const group of filteredGroups) {
      out.push({ key: `header:${group.nodeKey}`, isHeader: true });
      for (const row of group.rows) {
        out.push({ key: rowKey(row) });
      }
    }
    return out;
  });

  const confirmDisabled = $derived(selectedKey === undefined);

  function rowKey(row: CandidateRow): string {
    return `${row.nodeKey}|${row.rowOrdinal}`;
  }

  function findRow(key: string): CandidateRow | undefined {
    for (const group of candidateGroups) {
      const row = group.rows.find((r) => rowKey(r) === key);
      if (row) return row;
    }
    return undefined;
  }

  function handleConfirm() {
    if (selectedKey === undefined) return;
    const row = findRow(selectedKey);
    if (!row) return;
    onConfirm(row.nodeKey, row.rowOrdinal);
  }
</script>

<Dialog open width="md" ariaLabel={dialogTitle} initialFocus="first" onCancel={onCancel}>
  {#snippet title()}
    <DialogTitle>{dialogTitle}</DialogTitle>
  {/snippet}

  <form
    class="acp-form"
    onsubmit={(e) => {
      e.preventDefault();
      handleConfirm();
    }}
  >
    <input
      class="acp-search"
      type="search"
      placeholder="Search by node or row…"
      bind:value={searchText}
      aria-label="Filter lamp rows"
    />

    {#if filteredRowCount === 0}
      <p class="acp-empty">
        {#if totalRowCount === 0}
          No unclaimed Direct Lamp Control rows available. Connect a Signal LCC
          node or remove an existing lamp-indicator channel.
        {:else}
          No matching rows.
        {/if}
      </p>
    {:else}
      <SingleSelectList
        items={listItems}
        bind:selectedKey={selectedKey}
        ariaLabel="Lamp row candidates"
        name="add-channel"
      >
        {#snippet row(key)}
          {@const row = findRow(key)}
          {#if row}
            <span class="acp-name">{row.rowLabel}</span>
          {/if}
        {/snippet}
        {#snippet header(key)}
          {@const nodeKey = key.replace('header:', '')}
          {@const group = filteredGroups.find((g) => g.nodeKey === nodeKey)}
          {#if group}
            {#if group.nodeParts}
              <NodeLabel parts={group.nodeParts} orientation="inline" />
            {:else}
              {group.nodeName}
            {/if}
          {/if}
        {/snippet}
      </SingleSelectList>
    {/if}

    <button type="submit" class="acp-hidden-submit" tabindex="-1" aria-hidden="true"></button>
  </form>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" disabled={confirmDisabled} onclick={handleConfirm}>
        Confirm
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .acp-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  .acp-search {
    padding: 6px 10px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase300);
  }
  .acp-search:focus {
    outline: none;
    border-color: var(--fluent-strokeFocus2);
    box-shadow: 0 0 0 2px var(--fluent-strokeFocusHalo);
  }
  .acp-empty {
    color: var(--fluent-neutralForeground2);
    margin: 0.5rem 0;
    font-size: var(--fluent-fontSizeBase200);
  }
  .acp-name {
    font-weight: 600;
    color: var(--fluent-neutralForeground1);
  }
  .acp-hidden-submit {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    border: 0;
  }
</style>
