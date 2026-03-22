<!--
  T018: BowtieCard.svelte
  Renders a single bowtie card for one shared event ID (FR-002, FR-004, FR-005, FR-014).

  Props:
    card: BowtieCard — the card data to render

  Layout (FR-004):
    ┌──────────────────────────────────────────────────────────┐
    │ [card header: card.name ?? card.event_id_hex]             │
    ├──────────────────────────────────────────────────────────┤
    │  Producers column  │ → event_id │  Consumers column       │
    │  [ElementEntry]    │            │  [ElementEntry]          │
    │  [ElementEntry]    │            │  [ElementEntry]          │
    ├──────────────────────────────────────────────────────────┤
    │ [Ambiguous section — only when ambiguous_entries non-empty]│
    └──────────────────────────────────────────────────────────┘
-->

<script lang="ts">
  import type { BowtieCard as BowtieCardType, EventSlotEntry } from '$lib/api/tauri';
  import ElementEntry from './ElementEntry.svelte';
  import ConnectorArrow from './ConnectorArrow.svelte';
  import RoleClassifyPrompt from './RoleClassifyPrompt.svelte';
  import { isWellKnownEvent } from '$lib/utils/formatters';

  /** Write feedback state for a bowtie card (FR-030) */
  type WriteStatus = 'idle' | 'writing' | 'success' | 'error' | 'rolled-back' | 'rollback-failed';

  interface Props {
    card: BowtieCardType;
    highlighted?: boolean;
    dirtyFields?: Set<string>;
    isDirty?: boolean;
    writeStatus?: WriteStatus;
    writeError?: string | null;
    onRetry?: (() => void) | null;
    /** T030: callback to add a producer */
    onAddProducer?: (() => void) | null;
    /** T030: callback to add a consumer */
    onAddConsumer?: (() => void) | null;
    /** T031: callback to remove an element */
    onRemoveElement?: ((entry: EventSlotEntry) => void) | null;
    /** T037: callback to reclassify an ambiguous entry (includes chosen role) */
    onReclassifyRole?: ((nodeId: string, elementPath: string[], role: 'Producer' | 'Consumer') => void) | null;
    /** T042: callback to rename bowtie (eventIdHex, newName) */
    onRename?: ((eventIdHex: string, newName: string) => void) | null;
    /** T049: callback to add a tag */
    onAddTag?: ((eventIdHex: string, tag: string) => void) | null;
    /** T049: callback to remove a tag */
    onRemoveTag?: ((eventIdHex: string, tag: string) => void) | null;
    /** T049: all known tags for autocomplete suggestions */
    allTags?: string[] | null;
    /** Keys of newly-added entries for "new" badge display. */
    newEntryKeys?: Set<string> | null;
    /** Callback when the user selects (focuses) this card. */
    onSelect?: (() => void) | null;
  }

  let {
    card,
    highlighted = false,
    dirtyFields,
    isDirty = false,
    writeStatus = 'idle',
    writeError = null,
    onRetry = null,
    onAddProducer = null,
    onAddConsumer = null,
    onRemoveElement = null,
    onReclassifyRole = null,
    onRename = null,
    onAddTag = null,
    onRemoveTag = null,
    allTags = null,
    newEntryKeys = null,
    onSelect = null,
  }: Props = $props();

  let hasAmbiguous = $derived(card.ambiguous_entries.length > 0);

  // T033: card state for visual indicators
  let cardState = $derived(card.state?.toLowerCase() as 'active' | 'incomplete' | 'planning' | undefined);

  // Auto-dismiss success feedback after 3s
  let showSuccess = $state(false);
  $effect(() => {
    if (writeStatus === 'success') {
      showSuccess = true;
      const timer = setTimeout(() => { showSuccess = false; }, 3000);
      return () => clearTimeout(timer);
    } else {
      showSuccess = false;
    }
  });

  // T037: which ambiguous entry is being reclassified inline
  let reclassifyingEntry = $state<EventSlotEntry | null>(null);

  // T042: inline name editing
  let isEditingName = $state(false);
  let nameEditValue = $state('');

  function startRename() {
    nameEditValue = card.name ?? '';
    isEditingName = true;
  }

  function commitRename() {
    isEditingName = false;
    const trimmed = nameEditValue.trim();
    onRename?.(card.event_id_hex, trimmed);
  }

  function handleNameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') { isEditingName = false; }
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  // T049: tag management
  let newTag = $state('');
  let tagListId = $derived(`tag-list-${card.event_id_hex.replace(/\./g, '_')}`);

  function handleTagKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && newTag.trim()) {
      onAddTag?.(card.event_id_hex, newTag.trim());
      newTag = '';
    }
    if (e.key === 'Escape') { newTag = ''; }
  }
</script>

<div
  class="bowtie-card"
  class:highlighted
  class:is-dirty={isDirty}
  class:is-incomplete={cardState === 'incomplete' && !isWellKnownEvent(card.event_id_hex)}
  class:is-planning={cardState === 'planning'}
  aria-label="Bowtie card for event {card.event_id_hex}"
  data-event-id={card.event_id_hex}
>
  <!-- Header: name (if set) or event_id_hex as fallback (FR-014) -->
  <header class="card-header">
    <div class="card-title-area">
      {#if isEditingName}
        <input
          class="name-edit-input"
          type="text"
          bind:value={nameEditValue}
          onblur={commitRename}
          onkeydown={handleNameKeydown}
          use:focusInput
          aria-label="Edit connection name"
        />
      {:else}
        {#snippet titleContent()}
          {#if card.name}
            {card.name} <span class="event-id-suffix">({card.event_id_hex})</span>
          {:else}
            {card.event_id_hex}
          {/if}
          {#if isDirty}
            <span class="dirty-dot" title="Unsaved changes" aria-label="Unsaved changes">●</span>
          {/if}
        {/snippet}
        {#if !highlighted && !!onSelect}
          <button
            class="card-title card-title-selectable"
            onclick={onSelect}
            title="Click to focus this bowtie"
            onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSelect?.(); } }}
          >
            {@render titleContent()}
          </button>
        {:else}
          <h3 class="card-title">
            {@render titleContent()}
          </h3>
        {/if}
        {#if onRename && !isWellKnownEvent(card.event_id_hex)}
          <button
            class="rename-btn"
            onclick={startRename}
            title="Rename connection"
            aria-label="Rename connection"
          >✎</button>
        {/if}
      {/if}
    </div>
    <div class="header-badges">
      {#if dirtyFields && dirtyFields.size > 0}
        <span class="dirty-badge" aria-label="Modified fields: {[...dirtyFields].join(', ')}">
          modified
        </span>
      {/if}
      <!-- T029: Write operation feedback (FR-030) -->
      {#if writeStatus === 'writing'}
        <span class="write-status write-status-writing" aria-label="Writing to nodes">
          <span class="spinner"></span>
          Writing…
        </span>
      {:else if showSuccess}
        <span class="write-status write-status-success" aria-label="Write successful">
          ✓ Saved
        </span>
      {:else if writeStatus === 'error' || writeStatus === 'rollback-failed'}
        <span class="write-status write-status-error" aria-label="Write failed: {writeError ?? 'unknown error'}">
          ✗ {writeStatus === 'rollback-failed' ? 'Rollback failed' : 'Write failed'}
          {#if onRetry}
            <button class="retry-btn" onclick={onRetry} title="Retry write">Retry</button>
          {/if}
        </span>
      {:else if writeStatus === 'rolled-back'}
        <span class="write-status write-status-rolledback" aria-label="Write rolled back">
          ↺ Rolled back
        </span>
      {/if}
    </div>
  </header>

  <!-- Three-column layout: Producers | Arrow | Consumers (FR-004) -->
  <!-- Labels row -->
  <div class="labels-row">
    <div class="label-column">
      <span class="column-label producers-label">Producers</span>
    </div>
    <div class="label-spacer"></div>
    <div class="label-column">
      <span class="column-label consumers-label">Consumers</span>
    </div>
  </div>

  <!-- Entries and arrow row -->
  <div class="card-body">
    <!-- Producers entries -->
    <section class="column producers-column" aria-label="Producers">
      {#if card.producers.length === 0}
        {#if !isWellKnownEvent(card.event_id_hex)}
          <span class="empty-column-hint">⚠ No producers</span>
        {/if}
      {:else}
        {#each card.producers as entry (entry.node_id + entry.element_path.join('/'))}
          {@const isNew = newEntryKeys?.has(`${entry.node_id}:${entry.element_path.join('/')}`) ?? false}
          <div class="entry-row">
            <ElementEntry {entry} {isNew} />
            {#if onRemoveElement}
              <button
                class="remove-btn"
                onclick={() => onRemoveElement?.(entry)}
                title="Remove this producer"
                aria-label="Remove producer {entry.element_label}"
              >×</button>
            {/if}
          </div>
        {/each}
      {/if}
      {#if onAddProducer}
        <button class="add-element-btn add-element-btn--producer" onclick={onAddProducer}>
          + Add producer
        </button>
      {/if}
    </section>

    <!-- Centre connector arrow (FR-005) -->
    <ConnectorArrow />

    <!-- Consumers entries -->
    <section class="column consumers-column" aria-label="Consumers">
      {#if card.consumers.length === 0}
        {#if !isWellKnownEvent(card.event_id_hex)}
          <span class="empty-column-hint">⚠ No consumers</span>
        {/if}
      {:else}
        {#each card.consumers as entry (entry.node_id + entry.element_path.join('/'))}
          {@const isNew = newEntryKeys?.has(`${entry.node_id}:${entry.element_path.join('/')}`) ?? false}
          <div class="entry-row">
            <ElementEntry {entry} {isNew} />
            {#if onRemoveElement}
              <button
                class="remove-btn"
                onclick={() => onRemoveElement?.(entry)}
                title="Remove this consumer"
                aria-label="Remove consumer {entry.element_label}"
              >×</button>
            {/if}
          </div>
        {/each}
      {/if}
      {#if onAddConsumer}
        <button class="add-element-btn add-element-btn--consumer" onclick={onAddConsumer}>
          + Add consumer
        </button>
      {/if}
    </section>
  </div>

  <!-- Ambiguous entries section (only rendered when non-empty) -->
  {#if hasAmbiguous}
    <div class="ambiguous-section" aria-label="Unknown role entries">
      <h4 class="ambiguous-label">Unknown role — click to classify</h4>
      <div class="ambiguous-entries">
        {#each card.ambiguous_entries as entry (entry.node_id + entry.element_path.join('/'))}
          <div class="ambiguous-entry-row">
            {#if reclassifyingEntry === entry}
              <RoleClassifyPrompt
                elementName={entry.element_label}
                onClassify={(role) => {
                  onReclassifyRole?.(entry.node_id, entry.element_path, role);
                  reclassifyingEntry = null;
                }}
                onCancel={() => { reclassifyingEntry = null; }}
              />
            {:else}
              <button
                class="ambiguous-classify-btn"
                onclick={() => { reclassifyingEntry = entry; }}
                title="Click to classify this entry as Producer or Consumer"
                aria-label="Classify role for {entry.element_label}"
              >
                <span class="ambiguous-question">?</span>
                <ElementEntry {entry} />
              </button>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- T049: Tags section -->
  {#if (card.tags?.length ?? 0) > 0 || onAddTag}
    <div class="tags-section">
      {#each (card.tags ?? []) as tag (tag)}
        <span class="tag-chip">
          <span class="tag-text">{tag}</span>
          {#if onRemoveTag}
            <button
              class="tag-remove"
              onclick={() => onRemoveTag?.(card.event_id_hex, tag)}
              title="Remove tag: {tag}"
              aria-label="Remove tag: {tag}"
            >×</button>
          {/if}
        </span>
      {/each}
      {#if onAddTag}
        <div class="tag-input-wrapper">
          <input
            class="tag-input"
            type="text"
            placeholder="+ Add tag"
            bind:value={newTag}
            onkeydown={handleTagKeydown}
            list={tagListId}
            aria-label="Add tag"
          />
          {#if allTags && allTags.length > 0}
            <datalist id={tagListId}>
              {#each allTags.filter(t => !(card.tags ?? []).includes(t)) as suggestion}
                <option value={suggestion}></option>
              {/each}
            </datalist>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .bowtie-card {
    background: #ffffff;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    overflow: hidden;
    transition: box-shadow 0.15s ease;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  .bowtie-card.highlighted {
    box-shadow: 0 0 0 2px #0078d4;
  }

  .bowtie-card.is-dirty {
    border-color: #ca5010;
    box-shadow: 0 0 0 1px rgba(202, 80, 16, 0.2);
  }

  .dirty-dot {
    color: #ca5010;
    font-size: 0.6rem;
    vertical-align: super;
    margin-left: 4px;
  }

  .dirty-badge {
    font-size: 0.68rem;
    font-weight: 500;
    color: #ca5010;
    background: #fff4e6;
    padding: 1px 6px;
    border-radius: 3px;
    border: 1px solid #ffe0b2;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px 8px;
    border-bottom: 1px solid #d1d5db;
    background: #ffffff;
  }

  .card-title-area {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
  }

  /* T042: inline rename button and input */
  .rename-btn {
    flex-shrink: 0;
    background: none;
    border: none;
    color: #9ca3af;
    font-size: 0.85rem;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
    line-height: 1;
    transition: color 0.15s, background 0.15s;
  }

  .rename-btn:hover {
    color: #374151;
    background: #f3f4f6;
  }

  .name-edit-input {
    flex: 1 1 0;
    min-width: 0;
    font-size: 0.9rem;
    font-weight: 600;
    font-family: 'ui-monospace', monospace;
    color: #242424;
    border: 1px solid #0078d4;
    border-radius: 4px;
    padding: 2px 6px;
    outline: none;
    background: #fff;
  }

  .header-badges {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .card-title {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
    color: #242424;
    font-family: 'ui-monospace', monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1 1 0;
    min-width: 0;
  }

  .card-title-selectable {
    cursor: pointer;
    border-radius: 3px;
  }

  .card-title-selectable:hover {
    color: #0078d4;
  }

  .event-id-suffix {
    color: #6b7280;
    font-weight: 400;
    font-size: 0.82rem;
  }

  .card-body {
    display: flex;
    align-items: center;
    gap: 0;
    padding: 12px;
  }

  .labels-row {
    display: flex;
    align-items: flex-start;
    gap: 0;
    padding: 0 12px;
    padding-top: 8px;
    padding-bottom: 2px;
  }

  .label-column {
    flex: 1;
    display: flex;
    min-width: 0;
  }

  .label-spacer {
    flex-shrink: 0;
    width: 60px;
  }

  .column {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .column-label {
    margin: 0;
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 4px 8px;
    border-radius: 4px;
    display: inline-block;
    width: fit-content;
  }

  .producers-label {
    color: #0b6a0b;
    background: #dff6dd;
  }

  .consumers-label {
    color: #0078d4;
    background: #deecf9;
  }

  .ambiguous-section {
    border-top: 1px solid #d1d5db;
    padding: 10px 12px;
    background: #fdf8f4;
  }

  .ambiguous-label {
    margin: 0 0 8px;
    font-size: 0.78rem;
    font-weight: 600;
    color: #ca5010;
  }

  .ambiguous-entries {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  /* T029: Write operation feedback styles (FR-030) */
  .write-status {
    font-size: 0.72rem;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 3px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }

  .write-status-writing {
    color: #0078d4;
    background: #deecf9;
    border: 1px solid #b4d6fa;
  }

  .write-status-success {
    color: #0b6a0b;
    background: #dff6dd;
    border: 1px solid #b7e1cd;
  }

  .write-status-error {
    color: #a4262c;
    background: #fde7e9;
    border: 1px solid #f1bbbc;
  }

  .write-status-rolledback {
    color: #8a6d3b;
    background: #fcf8e3;
    border: 1px solid #f5e79e;
  }

  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid #b4d6fa;
    border-top-color: #0078d4;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .bowtie-card.is-incomplete {
    border-color: #f59e0b;
    box-shadow: 0 0 0 1px rgba(245, 158, 11, 0.2);
  }

  .bowtie-card.is-planning {
    border-color: #9ca3af;
    box-shadow: 0 0 0 1px rgba(156, 163, 175, 0.2);
    opacity: 0.85;
  }

  .empty-column-hint {
    font-size: 0.75rem;
    color: #d97706;
    font-style: italic;
    padding: 4px 6px;
    background: #fffbeb;
    border: 1px dashed #fcd34d;
    border-radius: 4px;
    text-align: center;
  }

  .entry-row {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    position: relative;
  }

  .remove-btn {
    position: absolute;
    top: 2px;
    right: 2px;
    flex-shrink: 0;
    background: none;
    border: none;
    color: #9ca3af;
    font-size: 0.9rem;
    font-weight: 700;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
    border-radius: 3px;
    transition: color 0.15s, background 0.15s;
  }

  .remove-btn:hover {
    color: #a4262c;
    background: #fde7e9;
  }

  .add-element-btn {
    margin-top: 6px;
    padding: 3px 10px;
    font-size: 0.75rem;
    font-weight: 500;
    border-radius: 4px;
    cursor: pointer;
    border: 1px dashed;
    background: transparent;
    transition: background 0.15s, border-color 0.15s;
    width: 100%;
    text-align: left;
  }

  .add-element-btn--producer {
    color: #0b6a0b;
    border-color: #a3cfb4;
  }

  .add-element-btn--producer:hover {
    background: #dff6dd;
    border-color: #0b6a0b;
  }

  .add-element-btn--consumer {
    color: #0078d4;
    border-color: #b4d6fa;
  }

  .add-element-btn--consumer:hover {
    background: #deecf9;
    border-color: #0078d4;
  }

  .ambiguous-entry-row {
    margin-bottom: 4px;
  }

  .ambiguous-classify-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    background: none;
    border: none;
    cursor: pointer;
    padding: 3px 4px;
    border-radius: 4px;
    text-align: left;
    transition: background 0.15s;
  }

  .ambiguous-classify-btn:hover {
    background: rgba(202, 80, 16, 0.06);
  }

  .ambiguous-question {
    font-size: 0.72rem;
    font-weight: 700;
    color: #d97706;
    background: #fef3c7;
    border: 1px solid #fde68a;
    border-radius: 50%;
    width: 16px;
    height: 16px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .retry-btn {
    margin-left: 4px;
    padding: 1px 6px;
    font-size: 0.68rem;
    font-weight: 600;
    color: #a4262c;
    background: #fff;
    border: 1px solid #a4262c;
    border-radius: 3px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .retry-btn:hover {
    background: #fde7e9;
  }

  /* T049: Tags section */
  .tags-section {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-top: 1px solid #e5e7eb;
    background: #fafafa;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 8px;
    font-size: 0.72rem;
    font-weight: 500;
    color: #374151;
    background: #e5e7eb;
    border-radius: 12px;
    border: 1px solid #d1d5db;
  }

  .tag-text {
    line-height: 1.4;
  }

  .tag-remove {
    background: none;
    border: none;
    color: #9ca3af;
    font-size: 0.8rem;
    cursor: pointer;
    padding: 0 1px;
    line-height: 1;
    border-radius: 50%;
    transition: color 0.15s;
  }

  .tag-remove:hover {
    color: #a4262c;
  }

  .tag-input-wrapper {
    position: relative;
  }

  .tag-input {
    font-size: 0.72rem;
    color: #374151;
    background: #fff;
    border: 1px dashed #d1d5db;
    border-radius: 12px;
    padding: 2px 10px;
    outline: none;
    width: 90px;
    transition: border-color 0.15s, width 0.15s;
  }

  .tag-input:focus {
    border-color: #0078d4;
    width: 130px;
  }

  .tag-input::placeholder {
    color: #9ca3af;
  }
</style>
