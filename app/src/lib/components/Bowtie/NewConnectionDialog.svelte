<!--
  T025 + T026: NewConnectionDialog.svelte — Dual element picker dialog for
  creating a new bowtie connection.

  dialog-shell-refactor (Slice 4): wraps the Fluent `Dialog` shell, replacing
  the prior native `<dialog>` + custom backdrop. Wide surface (960 px) with
  the picker panels filling a fixed-height body so internal scroll containers
  in `<ElementPicker>` still work.
-->

<script lang="ts">
  import ElementPicker from './ElementPicker.svelte';
  import type { ElementSelection, EventIdResolution } from '$lib/types/bowtie';
  // ADR-0004 / S2c: read bowtie preview through the layout facade.
  import { effectiveLayoutStore } from '$lib/layout';
  import { isPlaceholderEventId } from '$lib/utils/eventIds';
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';

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
    const usedInMap = effectiveLayoutStore.usedInMap;
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

<Dialog
  open={visible}
  width={960}
  ariaLabel="New Connection"
  initialFocus="none"
  onCancel={handleCancel}
>
  {#snippet title()}
    <DialogTitle>New Connection</DialogTitle>
  {/snippet}

  <div class="ncd-body">
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

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={handleCancel}>Cancel</Button>
      <Button
        appearance="primary"
        disabled={!canCreate}
        onclick={handleCreate}
        title={canCreate
          ? isNameOnly ? 'Create planning bowtie' : 'Create connection'
          : 'Enter a name or select both a producer and consumer'}
      >
        {isNameOnly ? 'Create Planning Bowtie' : 'Create Connection'}
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  /* Body fills a fixed height so the inner ElementPicker scroll containers
     can size correctly. The shell's body overflow stays auto (default) but
     never triggers because this fills it exactly. */
  .ncd-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: min(72vh, 640px);
    min-height: 0;
  }

  .name-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .name-field label {
    font-size: var(--fluent-fontSizeBase200);
    font-weight: 500;
    color: var(--fluent-neutralForeground2);
  }
  .name-input {
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    padding: 6px 10px;
    font-size: var(--fluent-fontSizeBase300);
    font-family: var(--fluent-fontFamily);
    outline: none;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
  }
  .name-input:focus {
    border-color: var(--fluent-strokeFocus2);
    box-shadow: 0 0 0 2px var(--fluent-strokeFocusHalo);
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
    color: var(--fluent-neutralForeground3);
    font-size: 1.2rem;
  }
</style>
