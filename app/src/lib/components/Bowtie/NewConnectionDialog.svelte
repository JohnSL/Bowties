<!--
  T025 + T026: NewConnectionDialog.svelte — Dual element picker dialog for
  creating a new bowtie connection.

  Props:
    visible: boolean — whether the dialog is shown
    onConfirm: callback when user creates a connection
    onCancel: callback when user cancels
    prefillProducer: optional pre-filled producer selection (config-first entry US3)
    prefillConsumer: optional pre-filled consumer selection (config-first entry US3)

  Layout (FR-034):
    ┌──────────────────────────────────────────────────────────┐
    │  New Connection                                          │
    ├──────────────────────────────────────────────────────────┤
    │  Name: [optional input field]                            │
    ├──────────────────────────────────────────────────────────┤
    │  ┌─ Producer ────────┐  ┌─ Consumer ────────┐           │
    │  │  [ElementPicker]  │  │  [ElementPicker]  │           │
    │  └───────────────────┘  └───────────────────┘           │
    ├──────────────────────────────────────────────────────────┤
    │  [Cancel]                        [Create Connection]     │
    └──────────────────────────────────────────────────────────┘
-->

<script lang="ts">
  import ElementPicker from './ElementPicker.svelte';
  import type { ElementSelection, EventIdResolution } from '$lib/types/bowtie';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { isPlaceholderEventId } from '$lib/utils/eventIds';

  interface Props {
    visible: boolean;
    onConfirm: (producer: ElementSelection | null, consumer: ElementSelection | null, name: string, resolution: EventIdResolution) => void;
    onCancel: () => void;
    /** Pre-filled producer selection (for config-first entry, US3/T040) */
    prefillProducer?: ElementSelection | null;
    /** Pre-filled consumer selection (for config-first entry, US3/T040) */
    prefillConsumer?: ElementSelection | null;
  }

  let {
    visible,
    onConfirm,
    onCancel,
    prefillProducer = null,
    prefillConsumer = null,
  }: Props = $props();

  let name = $state('');
  let producerSelection = $state<ElementSelection | null>(null);
  let consumerSelection = $state<ElementSelection | null>(null);

  // Apply prefills when they change
  $effect(() => {
    if (prefillProducer) producerSelection = prefillProducer;
  });
  $effect(() => {
    if (prefillConsumer) consumerSelection = prefillConsumer;
  });

  // FR-034: Create button enabled when both sides selected OR name provided alone (T045 intent-first)
  let canCreate = $derived(
    (name.trim().length > 0) || (producerSelection !== null && consumerSelection !== null)
  );

  // Whether this is a name-only (planning) creation
  let isNameOnly = $derived(name.trim().length > 0 && !producerSelection && !consumerSelection);

  // T026: Event ID selection rules (FR-002)
  function resolveEventId(
    producer: ElementSelection,
    consumer: ElementSelection,
  ): EventIdResolution {
    const usedInMap = bowtieCatalogStore.usedInMap;
    const prodConnected = usedInMap.has(producer.currentEventId) && !isPlaceholderEventId(producer.currentEventId);
    const consConnected = usedInMap.has(consumer.currentEventId) && !isPlaceholderEventId(consumer.currentEventId);

    // Rule 1: One side connected → use its event ID
    if (prodConnected && !consConnected) {
      return {
        eventIdHex: producer.currentEventId,
        writeTo: 'consumer',
      };
    }
    if (consConnected && !prodConnected) {
      return {
        eventIdHex: consumer.currentEventId,
        writeTo: 'producer',
      };
    }

    // Rule 2: Both connected to different bowties → prompt user
    if (prodConnected && consConnected && producer.currentEventId !== consumer.currentEventId) {
      const prodBowtie = usedInMap.get(producer.currentEventId);
      const consBowtie = usedInMap.get(consumer.currentEventId);
      return {
        eventIdHex: producer.currentEventId,
        writeTo: 'consumer',
        conflictPrompt: {
          producerBowtie: prodBowtie?.name ?? producer.currentEventId,
          consumerBowtie: consBowtie?.name ?? consumer.currentEventId,
        },
      };
    }

    // Rule 3: Both unconnected → use producer's current event ID, write to consumer
    const prodHasValue = !isPlaceholderEventId(producer.currentEventId);
    const consHasValue = !isPlaceholderEventId(consumer.currentEventId);

    if (prodHasValue) {
      return {
        eventIdHex: producer.currentEventId,
        writeTo: consHasValue && consumer.currentEventId === producer.currentEventId ? 'none' : 'consumer',
      };
    }
    if (consHasValue) {
      return {
        eventIdHex: consumer.currentEventId,
        writeTo: 'producer',
      };
    }

    // Both have no value — this shouldn't normally happen, but handle gracefully
    return {
      eventIdHex: '00.00.00.00.00.00.00.00',
      writeTo: 'none',
    };
  }

  function handleCreate(): void {
    if (isNameOnly) {
      // T045: name-only planning bowtie creation (no element picks)
      onConfirm(null, null, name.trim(), { eventIdHex: '', writeTo: 'none' });
      name = '';
      return;
    }

    if (!producerSelection || !consumerSelection) return;

    const resolution = resolveEventId(producerSelection, consumerSelection);
    onConfirm(producerSelection, consumerSelection, name.trim(), resolution);

    // Reset state
    name = '';
    producerSelection = null;
    consumerSelection = null;
  }

  function handleCancel(): void {
    name = '';
    producerSelection = null;
    consumerSelection = null;
    onCancel();
  }

  function handleProducerSelect(selection: ElementSelection): void {
    producerSelection = selection;
  }

  function handleConsumerSelect(selection: ElementSelection): void {
    consumerSelection = selection;
  }
</script>

{#if visible}
  <!-- Backdrop -->
  <div class="dialog-backdrop" onclick={handleCancel} role="presentation"></div>

  <dialog class="connection-dialog" open aria-label="New Connection">
    <header class="dialog-header">
      <h2>New Connection</h2>
      <button class="close-btn" onclick={handleCancel} aria-label="Close">✕</button>
    </header>

    <div class="dialog-body">
      <!-- Name input -->
      <div class="name-field">
        <label for="connection-name">Connection name (optional)</label>
        <input
          id="connection-name"
          type="text"
          placeholder="e.g., Yard Entry Signal"
          bind:value={name}
          class="name-input"
        />
      </div>

      <!-- Dual picker panels -->
      <div class="picker-panels">
        <div class="picker-panel">
          <h3 class="panel-label panel-label--producer">Producer</h3>
          <ElementPicker
            roleFilter="Producer"
            onSelect={handleProducerSelect}
            selectedElement={producerSelection}
          />
        </div>

        <div class="picker-divider" aria-hidden="true">
          <span class="arrow">→</span>
        </div>

        <div class="picker-panel">
          <h3 class="panel-label panel-label--consumer">Consumer</h3>
          <ElementPicker
            roleFilter="Consumer"
            onSelect={handleConsumerSelect}
            selectedElement={consumerSelection}
          />
        </div>
      </div>
    </div>

    <footer class="dialog-footer">
      <button class="btn btn-secondary" onclick={handleCancel}>Cancel</button>
      <button
        class="btn btn-primary"
        disabled={!canCreate}
        onclick={handleCreate}
        title={canCreate
          ? isNameOnly ? 'Create planning bowtie' : 'Create connection'
          : 'Enter a name or select both a producer and consumer'}
      >
        {isNameOnly ? 'Create Planning Bowtie' : 'Create Connection'}
      </button>
    </footer>
  </dialog>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 100;
  }

  .connection-dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 101;
    width: min(90vw, 960px);
    height: 85vh;
    display: flex;
    flex-direction: column;
    background: #fff;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.16);
    padding: 0;
    overflow: hidden;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-bottom: 1px solid #e5e7eb;
    background: #fafafa;
  }

  .dialog-header h2 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: #1f2937;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.1rem;
    cursor: pointer;
    color: #6b7280;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .close-btn:hover {
    background: #f3f4f6;
    color: #374151;
  }

  .dialog-body {
    flex: 1;
    overflow: hidden;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
  }

  .name-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name-field label {
    font-size: 0.82rem;
    font-weight: 500;
    color: #374151;
  }

  .name-input {
    border: 1px solid #d1d5db;
    border-radius: 4px;
    padding: 6px 10px;
    font-size: 0.88rem;
    font-family: inherit;
    outline: none;
  }

  .name-input:focus {
    border-color: #0078d4;
    box-shadow: 0 0 0 1px rgba(0, 120, 212, 0.3);
  }

  .picker-panels {
    display: flex;
    gap: 0;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .picker-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .panel-label {
    margin: 0;
    font-size: 0.78rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 4px 8px;
    border-radius: 4px;
    display: inline-block;
    width: fit-content;
  }

  .panel-label--producer {
    color: #0b6a0b;
    background: #dff6dd;
  }

  .panel-label--consumer {
    color: #0078d4;
    background: #deecf9;
  }

  .picker-divider {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 40px;
    color: #d1d5db;
    font-size: 1.2rem;
  }

  .arrow {
    color: #9ca3af;
  }

  .dialog-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: 1px solid #e5e7eb;
    background: #fafafa;
    flex-shrink: 0;
  }

  .btn {
    padding: 6px 16px;
    font-size: 0.88rem;
    font-family: inherit;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    font-weight: 500;
    transition: background 0.12s, border-color 0.12s;
  }

  .btn-secondary {
    background: #fff;
    border-color: #d1d5db;
    color: #374151;
  }

  .btn-secondary:hover {
    background: #f3f4f6;
  }

  .btn-primary {
    background: #0078d4;
    color: #fff;
    border-color: #0078d4;
  }

  .btn-primary:hover:not(:disabled) {
    background: #106ebe;
  }

  .btn-primary:disabled {
    background: #e5e7eb;
    border-color: #e5e7eb;
    color: #9ca3af;
    cursor: not-allowed;
  }
</style>
