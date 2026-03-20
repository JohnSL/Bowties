<!--
  BowtieCatalogPanel — in-page tab panel for the bowtie catalog.

  Rendered inside +page.svelte when activeTab === 'bowties'.
  Replaces the former /bowties route page, preserving all catalog display
  logic without full-page navigation (FR-003, FR-010, SC-004).
-->

<script lang="ts">
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { editableBowtiePreviewStore } from '$lib/stores/bowties.svelte';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { connectionRequestStore } from '$lib/stores/connectionRequest.svelte';
  import { setModifiedValue } from '$lib/api/config';
  import BowtieCard from '$lib/components/Bowtie/BowtieCard.svelte';
  import EmptyState from '$lib/components/Bowtie/EmptyState.svelte';
  import NewConnectionDialog from '$lib/components/Bowtie/NewConnectionDialog.svelte';
  import AddElementDialog from '$lib/components/Bowtie/AddElementDialog.svelte';
  import RoleClassifyPrompt from '$lib/components/Bowtie/RoleClassifyPrompt.svelte';
  import type { ElementSelection, EventIdResolution, PreviewBowtieCard } from '$lib/types/bowtie';
  import type { BowtieCard as BowtieCardType, EventSlotEntry } from '$lib/api/tauri';
  import { findLeafByAddress, findLeafByPath } from '$lib/types/nodeTree';
  import { generateFreshEventIdForNode } from '$lib/utils/eventIds';

  // Optional: event ID hex to scroll to and highlight (FR-009)
  let { highlightedEventIdHex = null }: { highlightedEventIdHex?: string | null } = $props();

  // Store access
  let catalog = $derived(bowtieCatalogStore.catalog);
  let readComplete = $derived(bowtieCatalogStore.readComplete);
  let preview = $derived(editableBowtiePreviewStore.preview);
  let previewCards = $derived(preview.bowties);

  // ── New Connection Dialog state ──────────────────────────────────────────
  let showNewConnectionDialog = $state(false);
  let prefillProducer = $state<ElementSelection | null>(null);
  let prefillConsumer = $state<ElementSelection | null>(null);

  // ── Add Element Dialog state (T030) ─────────────────────────────────────
  let addElementDialog = $state<{
    visible: boolean;
    role: 'Producer' | 'Consumer';
    card: PreviewBowtieCard | null;
  }>({ visible: false, role: 'Producer', card: null });

  // ── Delete Confirmation Dialog state (T032) ──────────────────────────────
  let deleteConfirmDialog = $state<{
    visible: boolean;
    card: PreviewBowtieCard | null;
    pendingEntry: EventSlotEntry | null;
  }>({ visible: false, card: null, pendingEntry: null });

  // ── Remove Confirmation Dialog state (Issue 2) ───────────────────────────
  let removeConfirmDialog = $state<{
    visible: boolean;
    card: PreviewBowtieCard;
    entry: EventSlotEntry;
  } | null>(null);

  // ── Classify-before-connect state (T041) ────────────────────────────────
  let classifyBeforeConnect = $state<{ selection: ElementSelection } | null>(null);

  /** Convert a PreviewBowtieCard to the BowtieCard shape expected by the BowtieCard component. */
  function toBowtieCard(p: PreviewBowtieCard): BowtieCardType {
    return {
      event_id_hex: p.eventIdHex,
      event_id_bytes: p.eventIdBytes,
      producers: p.producers,
      consumers: p.consumers,
      ambiguous_entries: p.ambiguousEntries,
      name: p.name ?? null,
      tags: p.tags,
      state: p.state === 'active' ? 'Active' : p.state === 'incomplete' ? 'Incomplete' : 'Planning',
    };
  }

  // Scroll to highlighted card when it becomes available (FR-009)
  $effect(() => {
    if (highlightedEventIdHex) {
      const id = highlightedEventIdHex;
      requestAnimationFrame(() => {
        const el = document.querySelector(`[data-event-id="${CSS.escape(id)}"]`);
        el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      });
    }
  });

  // T041: Watch for pending connection requests and open the dialog
  $effect(() => {
    const req = connectionRequestStore.pendingRequest;
    if (!req) return;

    if (req.role === 'Ambiguous') {
      // Show classify prompt before opening dialog
      classifyBeforeConnect = { selection: req.selection };
    } else if (req.role === 'Producer') {
      prefillProducer = req.selection;
      prefillConsumer = null;
      showNewConnectionDialog = true;
    } else {
      prefillProducer = null;
      prefillConsumer = req.selection;
      showNewConnectionDialog = true;
    }
    connectionRequestStore.clearRequest();
  });

  /**
   * Handle new connection creation from the dialog.
   * Sets modified values on tree leaves and metadata in bowtieMetadataStore.
   */
  function handleNewConnection(
    producer: ElementSelection | null,
    consumer: ElementSelection | null,
    name: string,
    resolution: EventIdResolution,
  ): void {
    showNewConnectionDialog = false;
    prefillProducer = null;
    prefillConsumer = null;

    if (!producer || !consumer) return;

    const eventIdHex = resolution.eventIdHex;

    // Create pending edit for the side(s) that need writing
    if (resolution.writeTo === 'consumer' || resolution.writeTo === 'both') {
      setEventIdOnLeaf(consumer, eventIdHex);
    }
    if (resolution.writeTo === 'producer' || resolution.writeTo === 'both') {
      setEventIdOnLeaf(producer, eventIdHex);
    }

    // Track bowtie metadata
    bowtieMetadataStore.createBowtie(eventIdHex, name || undefined);
  }

  /**
   * Set a modified event ID value on a leaf via the Rust tree.
   */
  function setEventIdOnLeaf(
    element: ElementSelection,
    eventIdHex: string,
  ): void {
    const tree = nodeTreeStore.getTree(element.nodeId);
    if (!tree) {
      console.warn('[BowtieCatalogPanel] setEventIdOnLeaf: tree not found for node', element.nodeId);
      return;
    }

    const leaf = findLeafByAddress(tree, element.address);
    if (!leaf) {
      console.warn('[BowtieCatalogPanel] setEventIdOnLeaf: leaf not found at address', element.address, 'in node', element.nodeId);
      return;
    }

    // Parse event ID hex string to bytes
    const eventIdBytes = eventIdHex.split('.').map(h => parseInt(h, 16));

    setModifiedValue(element.nodeId, element.address, element.space, {
      type: 'eventId',
      bytes: eventIdBytes,
      hex: eventIdHex,
    });
  }

  // ── T030: Add element ──────────────────────────────────────────────────

  function openAddElement(card: PreviewBowtieCard, role: 'Producer' | 'Consumer') {
    addElementDialog = { visible: true, role, card };
  }

  function handleAddElement(selection: ElementSelection) {
    const card = addElementDialog.card;
    addElementDialog = { visible: false, role: 'Producer', card: null };
    if (!card) return;
    setEventIdOnLeaf(selection, card.eventIdHex);
  }

  // ── T031: Remove element ───────────────────────────────────────────────

  function handleRemoveElement(card: PreviewBowtieCard, entry: EventSlotEntry) {
    // Issue 2: always show confirmation before removing any entry
    removeConfirmDialog = { visible: true, card, entry };
  }

  function confirmRemove() {
    if (!removeConfirmDialog) return;
    const { card, entry } = removeConfirmDialog;
    removeConfirmDialog = null;

    const isLastProducer = card.producers.length === 1 && entry.role === 'Producer';
    const isLastConsumer = card.consumers.length === 1 && entry.role === 'Consumer';
    const willBecomeEmpty =
      (isLastProducer && card.consumers.length === 0) ||
      (isLastConsumer && card.producers.length === 0) ||
      (isLastProducer && card.consumers.length === 1) ||
      (isLastConsumer && card.producers.length === 1);

    if (willBecomeEmpty) {
      // T032: show confirmation before deleting the last element(s)
      deleteConfirmDialog = { visible: true, card, pendingEntry: entry };
      return;
    }

    doRemoveElement(card, entry);
  }

  function doRemoveElement(card: PreviewBowtieCard, entry: EventSlotEntry) {
    const tree = nodeTreeStore.getTree(entry.node_id);
    if (!tree) {
      console.warn('[BowtieCatalogPanel] doRemoveElement: tree not found for node', entry.node_id);
      return;
    }

    // Find leaf by path (EventSlotEntry.element_path matches LeafConfigNode.path)
    const leaf = findLeafByPath(tree, entry.element_path);
    if (!leaf) {
      console.warn('[BowtieCatalogPanel] doRemoveElement: leaf not found for path', entry.element_path, 'in node', entry.node_id);
      return;
    }

    const newEventIdHex = generateFreshEventIdForNode(entry.node_id, tree);
    const newEventIdBytes = newEventIdHex.split('.').map(h => parseInt(h, 16));

    setModifiedValue(entry.node_id, leaf.address, leaf.space, {
      type: 'eventId',
      bytes: newEventIdBytes,
      hex: newEventIdHex,
    });
  }

  // ── T032: Delete confirmation ──────────────────────────────────────────

  function confirmDeleteKeepPlanning() {
    const { card, pendingEntry } = deleteConfirmDialog;
    deleteConfirmDialog = { visible: false, card: null, pendingEntry: null };
    if (card && pendingEntry) doRemoveElement(card, pendingEntry);
  }

  function confirmDeleteBowtie() {
    const { card, pendingEntry } = deleteConfirmDialog;
    deleteConfirmDialog = { visible: false, card: null, pendingEntry: null };
    if (card && pendingEntry) {
      doRemoveElement(card, pendingEntry);
      bowtieMetadataStore.deleteBowtie(card.eventIdHex);
    }
  }

  function cancelDeleteConfirm() {
    deleteConfirmDialog = { visible: false, card: null, pendingEntry: null };
  }

  // ── T037: Reclassify role ──────────────────────────────────────────────

  function handleReclassifyConfirm(nodeId: string, elementPath: string[], role: 'Producer' | 'Consumer') {
    const key = `${nodeId}:${elementPath.join('/')}`;
    bowtieMetadataStore.classifyRole(key, role);
  }
</script>

<div class="bowties-panel">
  <!-- Panel header: stats summary + new connection button -->
  {#if catalog}
    <div class="panel-header">
      <button
        class="new-connection-btn"
        onclick={() => { showNewConnectionDialog = true; }}
        title="Create a new bowtie connection"
      >
        + New Connection
      </button>
      <span class="catalog-meta">
        {catalog.bowties.length} connection{catalog.bowties.length !== 1 ? 's' : ''}
        · {catalog.source_node_count} node{catalog.source_node_count !== 1 ? 's' : ''}
      </span>
    </div>
  {/if}

  <!-- Content area -->
  <div class="panel-content">
    {#if !readComplete}
      <div class="not-ready">
        <p>Bowties will be available after CDI reads complete.</p>
        <p class="hint">Discover nodes and read their configuration from the toolbar.</p>
      </div>

    {:else if previewCards.length === 0 && (!catalog || catalog.bowties.length === 0)}
      <EmptyState />

    {:else}
      <!-- FR-003, FR-010: scrollable list of bowtie cards with dirty indicators -->
      <div class="card-list" role="list" aria-label="Bowtie connections">
        {#each previewCards as previewCard (previewCard.eventIdHex)}
          <div role="listitem">
            <BowtieCard
              card={toBowtieCard(previewCard)}
              highlighted={highlightedEventIdHex === previewCard.eventIdHex}
              isDirty={previewCard.isDirty}
              dirtyFields={previewCard.dirtyFields}
              newEntryKeys={previewCard.newEntryKeys}
              onAddProducer={() => openAddElement(previewCard, 'Producer')}
              onAddConsumer={() => openAddElement(previewCard, 'Consumer')}
              onRemoveElement={(entry) => handleRemoveElement(previewCard, entry)}
              onReclassifyRole={(nodeId, elementPath, role) => handleReclassifyConfirm(nodeId, elementPath, role)}
            />
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- New Connection dialog -->
<NewConnectionDialog
  visible={showNewConnectionDialog}
  onConfirm={handleNewConnection}
  onCancel={() => { showNewConnectionDialog = false; prefillProducer = null; prefillConsumer = null; }}
  {prefillProducer}
  {prefillConsumer}
/>

<!-- T030: Add Element dialog -->
<AddElementDialog
  visible={addElementDialog.visible}
  role={addElementDialog.role}
  bowtieName={addElementDialog.card?.name ?? addElementDialog.card?.eventIdHex ?? ''}
  onConfirm={handleAddElement}
  onCancel={() => { addElementDialog = { visible: false, role: 'Producer', card: null }; }}
/>

<!-- Issue 2: Remove confirmation overlay (shown before any entry removal) -->
{#if removeConfirmDialog?.visible}
  <div class="confirm-overlay">
    <div class="confirm-dialog">
      <h3 class="confirm-title">Remove entry?</h3>
      <p class="confirm-body">
        Remove <strong>{removeConfirmDialog.entry.element_label}</strong> from
        <strong>{removeConfirmDialog.card.name ?? removeConfirmDialog.card.eventIdHex}</strong>?
        A new unique event ID will be written to this slot on
        <strong>{removeConfirmDialog.entry.node_name}</strong>.
        {#if (removeConfirmDialog.entry.role === 'Producer' ? removeConfirmDialog.card.producers.length : removeConfirmDialog.card.consumers.length) === 1}
          This is the last entry — the bowtie will be marked as Planning.
        {/if}
      </p>
      <div class="confirm-actions">
        <button class="btn btn-secondary" onclick={() => { removeConfirmDialog = null; }}>Cancel</button>
        <button class="btn btn-danger" onclick={confirmRemove}>Remove</button>
      </div>
    </div>
  </div>
{/if}

<!-- T032: Delete confirmation overlay -->
{#if deleteConfirmDialog.visible}
  <div class="confirm-overlay">
    <div class="confirm-dialog">
      <h3 class="confirm-title">Remove last element?</h3>
      <p class="confirm-body">
        Removing this entry will make the bowtie incomplete or empty.
      </p>
      <div class="confirm-actions">
        <button class="btn btn-secondary" onclick={cancelDeleteConfirm}>Cancel</button>
        <button class="btn btn-warning" onclick={confirmDeleteKeepPlanning}>
          Keep as planning
        </button>
        <button class="btn btn-danger" onclick={confirmDeleteBowtie}>
          Delete bowtie
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- T041: Classify-before-connect overlay -->
{#if classifyBeforeConnect}
  <div class="confirm-overlay">
    <div class="confirm-dialog">
      <h3 class="confirm-title">Classify event slot role</h3>
      <p class="confirm-body">
        This slot's role is ambiguous. Is it a producer or consumer?
      </p>
      <RoleClassifyPrompt
        elementName={classifyBeforeConnect.selection.elementPath.at(-1) ?? 'event slot'}
        onClassify={(role) => {
          const sel = classifyBeforeConnect!.selection;
          const key = `${sel.nodeId}:${sel.elementPath.join('/')}`;
          bowtieMetadataStore.classifyRole(key, role);
          if (role === 'Producer') {
            prefillProducer = sel;
            prefillConsumer = null;
          } else {
            prefillProducer = null;
            prefillConsumer = sel;
          }
          classifyBeforeConnect = null;
          showNewConnectionDialog = true;
        }}
        onCancel={() => { classifyBeforeConnect = null; }}
      />
    </div>
  </div>
{/if}

<style>
  .bowties-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 16px;
    border-bottom: 1px solid #e5e7eb;
    background: #fff;
    flex-shrink: 0;
  }

  .new-connection-btn {
    padding: 4px 12px;
    font-size: 0.82rem;
    font-weight: 500;
    color: #fff;
    background: #2563eb;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .new-connection-btn:hover {
    background: #1d4ed8;
  }

  .new-connection-btn:active {
    background: #1e40af;
  }

  .catalog-meta {
    font-size: 0.78rem;
    color: #6b7280;
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    background: #f9fafb;
  }

  .not-ready {
    text-align: center;
    padding: 48px 24px;
    color: #6b7280;
  }

  .not-ready .hint {
    font-size: 0.85rem;
    margin-top: 8px;
    color: #9ca3af;
  }

  .card-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 900px;
    margin: 0 auto;
  }

  .confirm-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 900;
  }

  .confirm-dialog {
    background: #fff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
    padding: 20px 24px;
    width: 400px;
    max-width: 95vw;
  }

  .confirm-title {
    margin: 0 0 10px;
    font-size: 0.95rem;
    font-weight: 600;
    color: #1f2937;
  }

  .confirm-body {
    margin: 0 0 16px;
    font-size: 0.85rem;
    color: #6b7280;
    line-height: 1.5;
  }

  .confirm-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    flex-wrap: wrap;
  }

  .btn {
    padding: 6px 14px;
    font-size: 0.82rem;
    font-weight: 500;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.15s;
  }

  .btn-secondary {
    color: #374151;
    background: #fff;
    border-color: #d1d5db;
  }

  .btn-secondary:hover {
    background: #f9fafb;
  }

  .btn-warning {
    color: #92400e;
    background: #fef3c7;
    border-color: #fcd34d;
  }

  .btn-warning:hover {
    background: #fde68a;
  }

  .btn-danger {
    color: #fff;
    background: #dc2626;
    border-color: #dc2626;
  }

  .btn-danger:hover {
    background: #b91c1c;
  }
</style>
