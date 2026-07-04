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

  export interface CandidateRow {
    nodeKey: string;
    nodeName: string;
    rowOrdinal: number;
    rowLabel: string;
  }

  export interface CandidateGroup {
    nodeKey: string;
    nodeName: string;
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
    /**
     * Role the slot requires. Always `'lamp-indicator'` in S5; carried for
     * documentation + future filter hooks.
     */
    requiredRole: 'lamp-indicator';
    /**
     * Style that the new channel will adopt. Always `'single-led-direct-lamp'`
     * in S5; carried for parity with `requiredRole`.
     */
    requiredStyle: 'single-led-direct-lamp';
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
      <ul class="acp-list" role="radiogroup" aria-label="Lamp row candidates">
        {#each filteredGroups as group (group.nodeKey)}
          <li
            class="acp-group-header"
            data-testid="lamp-group-header"
            data-node-key={group.nodeKey}
          >
            {group.nodeName}
          </li>
          {#each group.rows as row (rowKey(row))}
            {@const key = rowKey(row)}
            <li class="acp-list-item">
              <label class="acp-row" class:selected={selectedKey === key}>
                <input
                  type="radio"
                  name="add-channel"
                  value={key}
                  checked={selectedKey === key}
                  onchange={() => (selectedKey = key)}
                  data-testid="lamp-row-radio"
                  data-row-key={key}
                />
                <span class="acp-name">{row.rowLabel}</span>
              </label>
            </li>
          {/each}
        {/each}
      </ul>
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
  .acp-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 18rem;
    overflow-y: auto;
    border: 1px solid var(--fluent-neutralStroke2, #e2e2e2);
    border-radius: 4px;
  }
  .acp-list-item {
    margin: 0;
  }
  .acp-group-header {
    list-style: none;
    margin: 0;
    padding: 0.4rem 0.6rem;
    background: var(--fluent-neutralBackground3, #f7f7f7);
    border-bottom: 1px solid var(--fluent-neutralStroke2, #e2e2e2);
    color: var(--fluent-neutralForeground1);
    font-weight: 600;
    font-size: var(--fluent-fontSizeBase200);
    position: sticky;
    top: 0;
  }
  .acp-row {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    cursor: pointer;
    border-bottom: 1px solid var(--fluent-neutralStroke2, #f0f0f0);
  }
  .acp-row:last-child {
    border-bottom: none;
  }
  .acp-row:hover {
    background: var(--fluent-neutralBackground1Hover, #f5f5f5);
  }
  .acp-row.selected {
    background: var(--fluent-neutralBackground1Selected, #eef);
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
