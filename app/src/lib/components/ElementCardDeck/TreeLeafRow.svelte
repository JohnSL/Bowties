<script lang="ts">
  /**
   * TreeLeafRow — renders a single leaf node from the unified tree.
   *
   * Horizontal layout: label (fixed width) + value + inline description.
   * Descriptions visible by default; truncated at ~100 chars with expand.
   * Maps enum values to labels via `constraints.mapEntries`.
   * Event IDs in monospace dotted hex, "(not set)" for all-zeros.
   *
   * Editable fields send their values to the Rust tree via `setModifiedValue`.
   * The tree's `modifiedValue` and `writeState` drive dirty/error display.
   */
  import type { LeafConfigNode, TreeConfigValue, TreeMapEntry } from '$lib/types/nodeTree';
  import { effectiveValue } from '$lib/types/nodeTree';
  import type { BowtieCard } from '$lib/api/tauri';
  import { setModifiedValue, triggerAction } from '$lib/api/config';
  import { bowtieFocusStore } from '$lib/stores/bowtieFocus.svelte';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { configFocusStore } from '$lib/stores/configFocus.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { layoutOpenInProgress } from '$lib/stores/layoutOpenLifecycle';
  import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { parseEventIdHex, formatEventIdHex } from '$lib/utils/serialize';
  import { isPlaceholderEventId } from '$lib/utils/eventIds';
  import { connectionRequestStore } from '$lib/stores/connectionRequest.svelte';
  import { untrack } from 'svelte';

  let {
    leaf,
    depth = 0,
    usedIn = undefined,
    nodeId = '',
    segmentOrigin = 0,
    segmentName = '',
    isNodeOffline = false,
  }: {
    leaf: LeafConfigNode;
    depth?: number;
    usedIn?: BowtieCard;
    nodeId?: string;
    /** Segment origin address — for per-segment save queries in SaveControls */
    segmentOrigin?: number;
    /** Segment display name — for progress labels in SaveControls */
    segmentName?: string;
    /** When true, all editable inputs are disabled (node is offline per FR-007, T050) */
    isNodeOffline?: boolean;
  } = $props();

  const DESC_TRUNCATE_THRESHOLD = 120;
  const DESC_TRUNCATE_AT = 100;

  // ── Local state ────────────────────────────────────────────────────────────

  let rowEl: HTMLDivElement;
  let descExpanded = $state(false);
  let actionTriggering = $state(false);

  /**
   * Local validation state tracked on the frontend only.
   * Invalid values are not sent to Rust — they stay local until corrected.
   */
  let localInvalidValue = $state<string | null>(null);
  let localValidationMessage = $state<string | null>(null);

  /**
   * Local buffer for the string input value while the user is actively typing.
   * Prevents the async IPC round-trip from resetting the input mid-keystroke.
   * Cleared on blur so the committed tree value takes over after focus leaves.
   */
  let localStrInput = $state<string | null>(null);

  function leafOffsetKey(): string {
    return `0x${leaf.address.toString(16).toUpperCase().padStart(8, '0')}`;
  }

  function valueToOfflineString(v: TreeConfigValue): string {
    switch (v.type) {
      case 'string':
        return v.value;
      case 'int':
        return String(v.value);
      case 'float':
        return String(v.value);
      case 'eventId':
        return v.hex ?? formatEventIdHex(v.bytes);
    }
  }

  function parseOfflinePlannedValue(raw: string): TreeConfigValue | null {
    if (leaf.elementType === 'string') {
      return { type: 'string', value: raw };
    }
    if (leaf.elementType === 'int') {
      const n = parseInt(raw, 10);
      return Number.isNaN(n) ? null : { type: 'int', value: n };
    }
    if (leaf.elementType === 'float') {
      const n = parseFloat(raw);
      return Number.isNaN(n) ? null : { type: 'float', value: n };
    }
    if (leaf.elementType === 'eventId') {
      const bytes = parseEventIdHex(raw);
      return bytes ? { type: 'eventId', bytes, hex: formatEventIdHex(bytes) } : null;
    }
    return null;
  }

  // ── Derived values ─────────────────────────────────────────────────────────

  let isDirty = $derived(leaf.modifiedValue != null);
  let draftOfflineRow = $derived.by(() => {
    if (!layoutStore.isOfflineMode || !nodeId) return null;
    return offlineChangesStore.findDraftConfigChange(nodeId, leaf.space, leafOffsetKey());
  });

  let persistedOfflineRow = $derived.by(() => {
    if (!layoutStore.isOfflineMode || !nodeId) return null;
    return offlineChangesStore.findPersistedConfigChange(nodeId, leaf.space, leafOffsetKey());
  });

  let pendingOfflineRow = $derived(draftOfflineRow ?? persistedOfflineRow);

  let offlinePlannedValue = $derived.by(() => {
    if (!pendingOfflineRow) return null;
    return parseOfflinePlannedValue(pendingOfflineRow.plannedValue);
  });

  let hasPendingApply = $derived(!!persistedOfflineRow);
  let suppressTransientIndicators = $derived($layoutOpenInProgress);
  let isDirtyVisible = $derived(isDirty && !suppressTransientIndicators);

  /** True when an editable event ID field's effective value is a leading-zero placeholder
   * (per LCC S-9.7.0.3 §5.2 — reserved range, never a valid routable event ID) */
  let isEventIdPlaceholder = $derived.by(() => {
    if (!(nodeId.length > 0 && leaf.elementType === 'eventId')) return false;
    const ev = effectiveValue(leaf);
    return ev?.type === 'eventId' && ev.bytes[0] === 0;
  });

  let isInvalid = $derived(localInvalidValue !== null);
  let hasPendingApplyVisible = $derived(hasPendingApply && !isDirty && !isInvalid && !suppressTransientIndicators);

  /** Active validation message: local input errors only (committed placeholder is shown separately) */
  let activeValidationMessage = $derived(localValidationMessage ?? null);
  let isWriting = $derived(leaf.writeState === 'writing');
  let hasWriteError = $derived(leaf.writeState === 'error');
  /** True when input should be disabled: either saving, node is offline (FR-007), or
   * the device rejected a write for this field with 0x1083 (runtime read-only) */
  let isDisabled = $derived(isWriting || isNodeOffline || !!leaf.readOnly);

  /** Whether this leaf type supports inline editing */
  let isEditable = $derived(
    nodeId.length > 0 &&
    (leaf.elementType === 'string' ||
      (leaf.elementType === 'int' && !(leaf.constraints?.mapEntries?.length) && !leaf.hintSlider))
  );

  /** Whether this leaf is an int field with constrained map entries (dropdown) */
  let isSelectEditable = $derived(
    nodeId.length > 0 &&
    leaf.elementType === 'int' &&
    !!(leaf.constraints?.mapEntries?.length) &&
    !leaf.hintRadio
  );

  /** Whether this leaf is an int field with slider hint */
  let isSliderEditable = $derived(
    nodeId.length > 0 &&
    leaf.elementType === 'int' &&
    !!leaf.hintSlider &&
    !leaf.constraints?.mapEntries?.length
  );

  /** Whether this leaf is an int field with radio button hint */
  let isRadioEditable = $derived(
    nodeId.length > 0 &&
    leaf.elementType === 'int' &&
    !!leaf.hintRadio &&
    !!(leaf.constraints?.mapEntries?.length)
  );

  /** Whether this leaf is an action element */
  let isActionLeaf = $derived(
    nodeId.length > 0 &&
    leaf.elementType === 'action'
  );

  /** Whether this leaf is a float field that supports inline editing */
  let isFloatEditable = $derived(
    nodeId.length > 0 &&
    leaf.elementType === 'float'
  );

  /** Whether this leaf is an event ID field that supports inline editing */
  let isEventIdEditable = $derived(
    nodeId.length > 0 &&
    leaf.elementType === 'eventId'
  );

  /** Current display value: offline planned, else modifiedValue, else committed value */
  let displayValue = $derived(offlinePlannedValue ?? effectiveValue(leaf));

  /** String value for controlled text input — local buffer takes priority while typing */
  let inputStr = $derived(
    localStrInput !== null
      ? localStrInput
      : (displayValue?.type === 'string' ? displayValue.value : (leaf.value?.type === 'string' ? leaf.value.value : ''))
  );

  /** Number value for controlled number input */
  let inputNum = $derived(
    displayValue?.type === 'int' ? displayValue.value : (leaf.value?.type === 'int' ? leaf.value.value : 0)
  );

  /** Float value for controlled float input */
  let inputFloat = $derived(
    displayValue?.type === 'float' ? displayValue.value : (leaf.value?.type === 'float' ? leaf.value.value : 0)
  );

  /** Current selected map-entry value for controlled select */
  let inputSelect = $derived(
    displayValue?.type === 'int' ? displayValue.value : (leaf.value?.type === 'int' ? leaf.value.value : 0)
  );

  /** Dotted-hex string for event ID text input */
  let inputEventId = $derived(
    displayValue?.type === 'eventId'
      ? formatEventIdHex(displayValue.bytes)
      : (leaf.value?.type === 'eventId' ? formatEventIdHex(leaf.value.bytes) : '00.00.00.00.00.00.00.00')
  );

  /** Whether the value uses a monospace font */
  let isMonoValue = $derived(
    leaf.elementType === 'eventId' || leaf.elementType === 'int' || leaf.elementType === 'float'
  );

  /** Truncated description text */
  let descText = $derived(leaf.description ?? '');
  let needsTruncation = $derived(descText.length > DESC_TRUNCATE_THRESHOLD);
  let displayDesc = $derived(
    needsTruncation && !descExpanded ? descText.slice(0, DESC_TRUNCATE_AT) + '…' : descText
  );

  // ── Config focus: scroll + focus this leaf if it is the navigation target ──

  $effect(() => {
    const focus = configFocusStore.leafFocusRequest;
    if (!focus || !nodeId) return;
    if (focus.nodeId !== nodeId) return;
    if (focus.elementPath.join('/') !== leaf.path.join('/')) return;

    // Consume the focus before scheduling side-effects to avoid re-triggering
    untrack(() => configFocusStore.clearLeafFocus());

    requestAnimationFrame(() => {
      rowEl?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      rowEl?.querySelector<HTMLElement>('input, select')?.focus();
    });
  });

  // ── Edit handlers ──────────────────────────────────────────────────────────

  function applyOfflineChange(newVal: TreeConfigValue): void {
    if (!nodeId) return;

    const baselineFromRow = pendingOfflineRow?.baselineValue;
    const baselineFromLeaf = leaf.value ? valueToOfflineString(leaf.value) : '';
    const baselineValue = baselineFromRow ?? baselineFromLeaf;
    const plannedValue = valueToOfflineString(newVal);

    offlineChangesStore.upsertConfigChange({
      nodeId,
      space: leaf.space,
      offset: leafOffsetKey(),
      baselineValue,
      plannedValue,
    });

    const draftRow = offlineChangesStore.findDraftConfigChange(nodeId, leaf.space, leafOffsetKey());
    const modifiedValue = draftRow ? parseOfflinePlannedValue(draftRow.plannedValue) : null;
    nodeTreeStore.setLeafModifiedValue(nodeId, leaf.path, modifiedValue);
  }

  function applyLeafValueChange(newVal: TreeConfigValue): void {
    if (layoutStore.isOfflineMode || isNodeOffline) {
      applyOfflineChange(newVal);
      return;
    }

    void setModifiedValue(nodeId, leaf.address, leaf.space, newVal);
  }

  /** Validate a string value and send to Rust tree */
  function handleStringInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;

    // Synchronously buffer the typed value so the derived inputStr stays
    // at what the user typed, preventing the async IPC response from
    // overwriting the input while they continue typing.
    localStrInput = raw;

    const newVal: TreeConfigValue = { type: 'string', value: raw };

    // Validate: max length
    const maxLen = leaf.size - 1;
    const encoder = new TextEncoder();
    const byteLen = encoder.encode(raw).length;
    const isValid = byteLen <= maxLen;

    if (!isValid) {
      localInvalidValue = raw;
      localValidationMessage = `Text too long (max ${maxLen} characters)`;
      return;
    }

    localInvalidValue = null;
    localValidationMessage = null;

    // In offline mode, commit on blur/tab to avoid creating rows per keystroke.
    if (!layoutStore.isOfflineMode && !isNodeOffline) {
      applyLeafValueChange(newVal);
    }
  }

  function handleStringBlur() {
    if (!(layoutStore.isOfflineMode || isNodeOffline)) {
      localStrInput = null;
      return;
    }

    const raw = localStrInput ?? inputStr;
    const maxLen = leaf.size - 1;
    const byteLen = new TextEncoder().encode(raw).length;
    if (byteLen <= maxLen) {
      const newVal: TreeConfigValue = { type: 'string', value: raw };
      applyLeafValueChange(newVal);
    }
    localStrInput = null;
  }

  /** Validate an integer value and send to Rust tree */
  function handleIntInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;
    const parsed = parseInt(raw, 10);

    if (isNaN(parsed)) {
      localInvalidValue = raw;
      localValidationMessage = 'Must be a valid number';
      return;
    }

    const newVal: TreeConfigValue = { type: 'int', value: parsed };

    // Validate: min/max
    const min = leaf.constraints?.min;
    const max = leaf.constraints?.max;
    const tooLow = min !== null && min !== undefined && parsed < min;
    const tooHigh = max !== null && max !== undefined && parsed > max;

    if (tooLow || tooHigh) {
      localInvalidValue = raw;
      localValidationMessage = `Value must be between ${min} and ${max}`;
      return;
    }

    localInvalidValue = null;
    localValidationMessage = null;
    applyLeafValueChange(newVal);
  }

  /** Handle int-with-mapEntries select change */
  function handleSelectChange(e: Event) {
    const raw = (e.target as HTMLSelectElement).value;
    const parsed = parseInt(raw, 10);
    const newVal: TreeConfigValue = { type: 'int', value: parsed };

    localInvalidValue = null;
    localValidationMessage = null;
    applyLeafValueChange(newVal);
  }

  /** Validate a float value and send to Rust tree */
  function handleFloatInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;
    const parsed = parseFloat(raw);

    if (isNaN(parsed)) {
      localInvalidValue = raw;
      localValidationMessage = 'Must be a valid number';
      return;
    }

    const newVal: TreeConfigValue = { type: 'float', value: parsed };

    // Validate min/max constraints
    const min = leaf.constraints?.min;
    const max = leaf.constraints?.max;
    const tooLow = min !== null && min !== undefined && parsed < min;
    const tooHigh = max !== null && max !== undefined && parsed > max;

    if (tooLow || tooHigh) {
      localInvalidValue = raw;
      localValidationMessage = `Value must be between ${min} and ${max}`;
      return;
    }

    localInvalidValue = null;
    localValidationMessage = null;
    applyLeafValueChange(newVal);
  }

  /** T034: event ID field — text input with dotted-hex validation */
  function handleEventIdInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;

    const parsedBytes = parseEventIdHex(raw);

    if (!parsedBytes) {
      localInvalidValue = raw;
      localValidationMessage = 'Invalid event ID format — use HH.HH.HH.HH.HH.HH.HH.HH';
      return;
    }

    if (parsedBytes[0] === 0) {
      // If the typed value exactly matches the committed (saved) value, the user has
      // reverted to the original placeholder — clear local error state so the
      // placeholder note takes over rather than showing a red error.
      const committed = leaf.value;
      if (committed?.type === 'eventId' && committed.bytes.every((b, i) => b === parsedBytes[i])) {
        localInvalidValue = null;
        localValidationMessage = null;
        return;
      }
      localInvalidValue = raw;
      localValidationMessage = 'Event IDs starting with 00 are reserved placeholders and cannot be configured';
      return;
    }

    const newVal: TreeConfigValue = { type: 'eventId', bytes: parsedBytes, hex: formatEventIdHex(parsedBytes) };

    localInvalidValue = null;
    localValidationMessage = null;
    applyLeafValueChange(newVal);
  }

  // ── Value display helpers ──────────────────────────────────────────────────

  /** Format a TreeConfigValue for display */
  function formatValue(v: TreeConfigValue | null): string {
    if (v === null) return '—';
    switch (v.type) {
      case 'int':     return formatIntValue(v.value);
      case 'string':  return v.value || '(empty)';
      case 'float':   return v.value.toFixed(4);
      case 'eventId': return formatEventId(v.bytes);
    }
  }

  /** Map int values to enum labels when mapEntries exist */
  function formatIntValue(value: number): string {
    if (leaf.constraints?.mapEntries) {
      const entry = leaf.constraints.mapEntries.find((e: TreeMapEntry) => e.value === value);
      if (entry) return entry.label;
    }
    return String(value);
  }

  /** Format event ID bytes; all-zero = "(not set)" */
  function formatEventId(bytes: number[]): string {
    if (bytes.every(b => b === 0)) return '(not set)';
    return bytes.map(b => b.toString(16).padStart(2, '0')).join('.');
  }

  /** Trigger an action element, with optional confirmation dialog. */
  async function handleActionTrigger() {
    if (leaf.dialogText) {
      if (!confirm(leaf.dialogText)) return;
    }
    actionTriggering = true;
    try {
      await triggerAction(nodeId, leaf.space, leaf.address, leaf.size, leaf.actionValue ?? 0);
    } finally {
      actionTriggering = false;
    }
  }

  function handleNavigateToBowties() {
    if (usedIn) {
      bowtieFocusStore.focusBowtie(usedIn.event_id_hex);
    }
  }

  /** T039: Open the bowties tab with this slot pre-filled as a connection side. */
  function handleCreateConnection() {
    const selection = {
      nodeId,
      nodeName: nodeId,
      elementLabel: leaf.name,
      elementPath: leaf.path,
      address: leaf.address,
      space: leaf.space,
      currentEventId: leaf.value?.type === 'eventId'
        ? (leaf.value as { type: 'eventId'; bytes: number[]; hex: string }).hex
        : '00.00.00.00.00.00.00.00',
    };
    connectionRequestStore.requestConnection(selection, leaf.eventRole ?? 'Ambiguous');
  }
</script>

<div
  bind:this={rowEl}
  class="field-row"
  class:compact={depth >= 3}
  class:dirty={isDirtyVisible && !isInvalid}
  class:offline-pending={hasPendingApplyVisible}
  class:invalid={isInvalid}
  class:eventid-placeholder={isEventIdPlaceholder && !isInvalid}
  class:writing={isWriting}
  class:write-error={hasWriteError}
  role="listitem"
>
  <span class="field-label" title={leaf.name}>{leaf.name}</span>

  <div class="field-content">

    {#if isEditable && leaf.elementType === 'string'}
      <input
        type="text"
        class="field-input"
        value={inputStr}
        maxlength={leaf.size - 1}
        disabled={isDisabled}
        aria-label={leaf.name}
        aria-invalid={isInvalid}
        oninput={handleStringInput}
        onblur={handleStringBlur}
      />
    {:else if isEditable && leaf.elementType === 'int'}
      <input
        type="number"
        class="field-input field-input--number"
        value={inputNum}
        min={leaf.constraints?.min ?? undefined}
        max={leaf.constraints?.max ?? undefined}
        disabled={isDisabled}
        aria-label={leaf.name}
        aria-invalid={isInvalid}
        oninput={handleIntInput}
      />
    {:else if isSelectEditable && leaf.constraints?.mapEntries}
      <!-- T030: int field with constrained map entries — dropdown select -->
      <select
        class="field-input field-input--select"
        value={inputSelect}
        disabled={isDisabled}
        aria-label={leaf.name}
        onchange={handleSelectChange}
      >
        {#if !leaf.constraints.mapEntries.some((e: TreeMapEntry) => e.value === inputSelect)}
          <option value={inputSelect} disabled>(Reserved: {inputSelect})</option>
        {/if}
        {#each leaf.constraints.mapEntries as entry}
          <option value={entry.value}>{entry.label}</option>
        {/each}
      </select>
    {:else if isSliderEditable && leaf.hintSlider}
      <!-- Slider hint: range input for int without map entries -->
      <div class="field-slider-wrap">
        <input
          type="range"
          class="field-input field-input--slider"
          value={inputNum}
          min={leaf.constraints?.min ?? undefined}
          max={leaf.constraints?.max ?? undefined}
          step={leaf.hintSlider.tickSpacing > 0 ? leaf.hintSlider.tickSpacing : 1}
          disabled={isDisabled}
          aria-label={leaf.name}
          oninput={leaf.hintSlider.immediate ? handleIntInput : undefined}
          onchange={!leaf.hintSlider.immediate ? handleIntInput : undefined}
        />
        {#if leaf.hintSlider.showValue}
          <span class="slider-value mono">{inputNum}</span>
        {/if}
      </div>
    {:else if isRadioEditable && leaf.constraints?.mapEntries}
      <!-- Radio hint: radio buttons for int with map entries -->
      <div class="field-radio-group" role="radiogroup" aria-label={leaf.name}>
        {#each leaf.constraints.mapEntries as entry}
          <label class="field-radio-label">
            <input
              type="radio"
              name={`leaf-radio-${leaf.space}-${leaf.address}`}
              value={entry.value}
              checked={inputSelect === entry.value}
              disabled={isDisabled}
              onchange={handleSelectChange}
            />
            {entry.label}
          </label>
        {/each}
      </div>
    {:else if isActionLeaf}
      <!-- Action element: trigger button -->
      <button
        class="action-trigger-btn"
        disabled={isDisabled || actionTriggering}
        onclick={handleActionTrigger}
        aria-label={leaf.buttonText ?? leaf.name}
      >
        {#if actionTriggering}
          Triggering…
        {:else}
          {leaf.buttonText ?? 'Trigger'}
        {/if}
      </button>
    {:else if isFloatEditable}
      <!-- T031: float field — number input with step="any" -->
      <input
        type="number"
        step="any"
        class="field-input field-input--number"
        value={inputFloat}
        min={leaf.constraints?.min ?? undefined}
        max={leaf.constraints?.max ?? undefined}
        disabled={isDisabled}
        aria-label={leaf.name}
        aria-invalid={isInvalid}
        oninput={handleFloatInput}
      />
    {:else if isEventIdEditable}
      <!-- T034: event ID field — text input with dotted-hex validation -->
      <input
        type="text"
        class="field-input field-input--eventid"
        value={inputEventId}
        placeholder="HH.HH.HH.HH.HH.HH.HH.HH"
        disabled={isDisabled}
        aria-label={leaf.name}
        aria-invalid={isInvalid}
        oninput={handleEventIdInput}
      />
    {:else}
      <span class="field-value" class:mono={isMonoValue}>
        {formatValue(leaf.value)}
      </span>
    {/if}

    {#if hasWriteError && leaf.writeError}
      <span class="write-error-msg" role="alert">⚠ {leaf.writeError}</span>
    {/if}

    {#if descText}
      <span class="field-desc">
        {displayDesc}
        {#if needsTruncation}
          <button
            class="desc-expand-btn"
            onclick={() => (descExpanded = !descExpanded)}
            aria-label={descExpanded ? 'Collapse description' : 'Expand description'}
          >{descExpanded ? '[−]' : '[+]'}</button>
        {/if}
      </span>
    {/if}

    {#if isInvalid && activeValidationMessage}
      <span class="validation-msg" role="alert">{activeValidationMessage}</span>
    {/if}

    {#if !suppressTransientIndicators && draftOfflineRow}
      <span class="offline-change-msg" role="status">
        Unsaved offline edit: {draftOfflineRow.baselineValue} -> {draftOfflineRow.plannedValue}
      </span>
      <button
        class="revert-baseline-btn"
        onclick={() => offlineChangesStore.revertToBaseline(draftOfflineRow!.changeId)}
        title="Revert to captured baseline value"
        aria-label="Revert to baseline"
        disabled={offlineChangesStore.isBusy}
      >↩ Revert</button>
    {/if}

    {#if !suppressTransientIndicators && persistedOfflineRow}
      <span class="offline-pending-msg" role="status">
        Pending apply: {persistedOfflineRow.baselineValue} -> {persistedOfflineRow.plannedValue}
      </span>
      <button
        class="revert-baseline-btn"
        onclick={() => offlineChangesStore.revertToBaseline(persistedOfflineRow!.changeId)}
        title="Revert to captured baseline value"
        aria-label="Revert to baseline"
        disabled={offlineChangesStore.isBusy}
      >↩ Revert</button>
    {/if}

    {#if leaf.eventRole}
      <span class="event-role">
        <span class="role-tag role-{leaf.eventRole.toLowerCase()}">{leaf.eventRole}</span>
      </span>
    {/if}

    {#if usedIn}
      <span class="used-in">
        → <button
          class="used-in-link"
          onclick={handleNavigateToBowties}
          title="View bowtie for event {usedIn.event_id_hex}"
          aria-label="View bowtie connection for {bowtieCatalogStore.getDisplayName(usedIn.event_id_hex)}"
        >{bowtieCatalogStore.getDisplayName(usedIn.event_id_hex)}</button>
      </span>
    {/if}

    {#if isEventIdEditable && !usedIn && !isEventIdPlaceholder}
      <button
        class="new-connection-btn"
        onclick={handleCreateConnection}
        title="Create a bowtie connection using this event slot"
        aria-label="Create connection from {leaf.name}"
      >→ New Connection</button>
    {/if}

    {#if isEventIdPlaceholder && !isInvalid}
      <span class="placeholder-msg">Unconfigured placeholder — this event ID will never be emitted</span>
    {/if}
  </div>
</div>

<style>
  /* ══════════════════════════════════════════
     Fluent UI Design — TreeLeafRow
     ══════════════════════════════════════════ */

  .field-row {
    display: flex;
    align-items: baseline;
    gap: var(--field-gap, 8px);
    min-height: 26px;
    padding: 2px 4px 2px 6px;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    transition: background-color 0.1s ease;
    border-radius: 3px;
    border-left: 3px solid transparent;
  }

  .field-row:hover {
    background-color: rgba(0,0,0,0.02);            /* very subtle hover for scannability */
  }

  .field-row.compact {
    --field-gap: 4px;
    min-height: 22px;
    padding: 1px 4px 1px 6px;
  }

  /* ── Dirty / invalid state indicators ── */

  .field-row.dirty {
    border-left-color: #ca8500;                    /* amber — unsaved change (distinct from selection blue) */
    background-color: rgba(202, 133, 0, 0.05);
  }

  .field-row.offline-pending {
    border-left-color: #0f766e;                    /* teal — saved in layout, pending apply */
    background-color: rgba(15, 118, 110, 0.05);
  }

  .field-row.invalid {
    border-left-color: #a4262c;                    /* colorPaletteRedForeground1 */
    background-color: rgba(164, 38, 44, 0.04);
  }

  .field-row.writing {
    border-left-color: #8a8886;                    /* colorNeutralForeground3 — neutral while saving */
    opacity: 0.8;
  }

  .field-row.write-error {
    border-left-color: #ca5010;                    /* colorPaletteOrangeForeground1 */
    background-color: rgba(202, 80, 16, 0.05);
  }

  .field-row.eventid-placeholder {
    border-left-color: #0f6cbd;                    /* Fluent info blue — unconfigured placeholder */
  }

  .field-label {
    flex: 0 0 var(--field-label-width, 120px);
    text-align: right;
    color: #605e5c;                                /* colorNeutralForeground2 — subdued so values stand out */
    font-size: 12px;
    font-weight: 400;
    line-height: 1.45;
    white-space: normal;
    overflow-wrap: break-word;
    word-break: break-word;
  }

  .field-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
    font-size: 13px;
    line-height: 1.45;
  }

  .field-value {
    color: #242424;                                /* colorNeutralForeground1 */
    word-break: break-word;
  }

  .field-value.mono {
    font-family: 'Cascadia Code', 'Cascadia Mono', 'SF Mono', 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    letter-spacing: -0.01em;
  }

  .offline-change-msg {
    color: #7c2d12;
    background: #ffedd5;
    border: 1px solid #fdba74;
    border-radius: 10px;
    padding: 1px 8px;
    font-size: 11px;
    line-height: 1.5;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  .offline-pending-msg {
    color: #115e59;
    background: #ccfbf1;
    border: 1px solid #99f6e4;
    border-radius: 10px;
    padding: 1px 8px;
    font-size: 11px;
    line-height: 1.5;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  .revert-baseline-btn {
    color: #616161;
    background: transparent;
    border: 1px solid #d1d1d1;
    border-radius: 10px;
    padding: 1px 8px;
    font-size: 11px;
    line-height: 1.5;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    cursor: pointer;
  }
  .revert-baseline-btn:hover:not(:disabled) {
    background: #f5f5f5;
    border-color: #a0a0a0;
    color: #242424;
  }
  .revert-baseline-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ── Editable input fields ── */

  .field-input {
    flex: 1;
    min-width: 60px;
    max-width: 320px;
    padding: 2px 6px;
    font-size: 13px;
    font-family: inherit;
    color: #242424;
    background: #ffffff;
    border: 1px solid #c8c6c4;                    /* colorNeutralStroke1 */
    border-radius: 3px;
    outline: none;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }

  .field-input:focus {
    border-color: #0078d4;                         /* colorBrandForeground1 */
    box-shadow: 0 0 0 1px #0078d4;
  }

  .field-input:disabled {
    background: #f3f2f1;
    color: #a19f9d;
    cursor: not-allowed;
  }

  /* Invalid state — red border on the input itself */
  :global(.invalid) .field-input,
  .field-row.invalid .field-input {
    border-color: #a4262c;
  }

  .field-input--number {
    max-width: 120px;
    font-family: 'Cascadia Code', 'Cascadia Mono', 'SF Mono', 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
  }

  .field-input--select {
    flex: 0 0 auto;                                /* don't stretch — size to content */
    width: auto;                                   /* browser measures widest <option> */
    max-width: 280px;                              /* guard against pathologically long labels */
  }

  .field-input--slider {
    flex: 1 1 auto;
    max-width: 200px;
    accent-color: #0078d4;
    cursor: pointer;
  }

  .field-slider-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 auto;
  }

  .slider-value {
    font-family: 'Cascadia Code', 'Cascadia Mono', 'SF Mono', 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    min-width: 4ch;
    text-align: right;
  }

  .field-radio-group {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    align-items: center;
  }

  .field-radio-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 13px;
    cursor: pointer;
  }

  .action-trigger-btn {
    flex: 0 0 auto;
    padding: 4px 12px;
    font-size: 13px;
    font-family: inherit;
    background: #0078d4;
    color: #fff;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .action-trigger-btn:hover:not(:disabled) {
    background: #106ebe;
  }

  .action-trigger-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .field-input--eventid {
    flex: 0 1 auto;
    width: calc(23ch + 14px);                      /* exact fit for HH.HH.HH.HH.HH.HH.HH.HH */
    max-width: none;
    font-family: 'Cascadia Code', 'Cascadia Mono', 'SF Mono', 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    letter-spacing: -0.01em;
  }

  /* ── Validation / error messages ── */

  .validation-msg {
    display: block;
    width: 100%;
    font-size: 11px;
    color: #a4262c;                                /* colorPaletteRedForeground1 */
    margin-top: 2px;
  }

  .placeholder-msg {
    display: block;
    width: 100%;
    font-size: 11px;
    color: #0f6cbd;                                /* Fluent info blue — informational, no urgency */
    margin-top: 2px;
  }

  .write-error-msg {
    display: block;
    width: 100%;
    font-size: 11px;
    color: #ca5010;                                /* colorPaletteOrangeForeground1 */
    margin-top: 2px;
  }

  .field-desc {
    color: #8a8886;                                /* warmer hint gray */
    font-size: 12px;
    font-style: italic;                            /* hint-like feel */
    line-height: 1.35;
    white-space: pre-wrap;                         /* preserve newlines from CDI descriptions */
  }

  .desc-expand-btn {
    background: none;
    border: none;
    padding: 0 2px;
    font-size: 12px;
    font-style: normal;
    color: #0078d4;                                /* colorBrandForeground1 */
    cursor: pointer;
    font-weight: 600;
  }

  .desc-expand-btn:hover {
    text-decoration: underline;
    color: #106ebe;                                /* colorBrandForeground1Hover */
  }

  .desc-expand-btn:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 1px;
    border-radius: 2px;
  }

  .event-role {
    font-size: 11px;
  }

  .role-tag {
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 4px;                            /* borderRadiusMedium */
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .role-tag.role-producer {
    color: #0b6a0b;                                /* colorPaletteGreenForeground1 */
    background: #dff6dd;                           /* colorPaletteGreenBackground1 */
  }

  .role-tag.role-consumer {
    color: #0078d4;                                /* colorBrandForeground1 */
    background: #deecf9;                           /* colorPaletteBlueBg1 */
  }

  .role-tag.role-ambiguous {
    color: #605e5c;                                /* colorNeutralForeground2 */
    background: #f3f2f1;                           /* colorNeutralBackground3 */
  }

  .used-in {
    font-size: 11px;
    color: #8a8886;                                /* warmer gray */
  }

  .used-in-link {
    background: none;
    border: none;
    padding: 0;
    font-size: 11px;
    color: #0078d4;                                /* colorBrandForeground1 */
    cursor: pointer;
    text-decoration: underline;
  }

  .used-in-link:hover {
    color: #106ebe;                                /* colorBrandForeground1Hover */
  }

  .used-in-link:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 1px;
    border-radius: 2px;
  }

  .new-connection-btn {
    background: none;
    border: 1px solid #b4d6fa;
    padding: 1px 8px;
    font-size: 11px;
    color: #0078d4;
    cursor: pointer;
    border-radius: 3px;
    transition: background 0.15s, border-color 0.15s;
    white-space: nowrap;
  }

  .new-connection-btn:hover {
    background: #deecf9;
    border-color: #0078d4;
  }

  .new-connection-btn:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 1px;
  }
</style>
