<script lang="ts">
  /**
   * TreeLeafRow — renders a single leaf node from the unified tree.
   *
   * Horizontal layout: label (fixed width) + value + inline description.
   * Descriptions visible by default; truncated at ~100 chars with expand.
   * Maps enum values to labels via `constraints.mapEntries`.
   * Event IDs in monospace dotted hex, "(not set)" for all-zeros.
   *
   * Spec 007: string and int (no map) fields are editable inline.
   * Dirty state tracked in PendingEditsStore. Validation enforced on input.
   */
  import type { LeafConfigNode, TreeConfigValue, TreeMapEntry, PendingEdit } from '$lib/types/nodeTree';
  import type { BowtieCard } from '$lib/api/tauri';
  import { bowtieName } from '$lib/api/tauri';
  import { goto } from '$app/navigation';
  import { pendingEditsStore, makePendingEditKey, pendingEditsVersion } from '$lib/stores/pendingEdits.svelte';
  import { parseEventIdHex, formatEventIdHex } from '$lib/utils/serialize';

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

  let descExpanded = $state(false);

  /**
   * Local pending value set while the user is editing this field.
   * null = no in-progress edit (show leaf.value as read-only or pre-fill input).
   */
  let localPending = $state<TreeConfigValue | null>(null);

  // Sync localPending with store on mount / leaf change / any store mutation.
  // If the store already has an edit for this field (e.g. navigated away
  // and back, or another component discarded the edit), restore or clear it.
  $effect(() => {
    void $pendingEditsVersion;  // subscribe so this re-runs on any store mutation
    const key = makePendingEditKey(nodeId, leaf.space, leaf.address);
    const stored = pendingEditsStore.getEdit(key);
    if (stored) {
      localPending = stored.pendingValue;
    } else {
      localPending = null;
    }
  });

  // ── Derived values ─────────────────────────────────────────────────────────

  // Version counter ensures $derived re-evaluates when any edit changes
  let _version = $derived($pendingEditsVersion);

  let editKey = $derived(makePendingEditKey(nodeId, leaf.space, leaf.address));
  let currentEdit = $derived(_version >= 0 ? pendingEditsStore.getEdit(editKey) : undefined);
  let isDirty = $derived(localPending !== null || currentEdit !== undefined);
  let isInvalid = $derived(currentEdit?.validationState === 'invalid');
  let isWriting = $derived(currentEdit?.writeState === 'writing');
  let hasWriteError = $derived(currentEdit?.writeState === 'error');
  /** True when input should be disabled: either saving or node is offline (FR-007) */
  let isDisabled = $derived(isWriting || isNodeOffline);

  /** Whether this leaf type supports inline editing */
  let isEditable = $derived(
    nodeId.length > 0 &&
    (leaf.elementType === 'string' ||
      (leaf.elementType === 'int' && !(leaf.constraints?.mapEntries?.length)))
  );

  /** Whether this leaf is an int field with constrained map entries (dropdown) */
  let isSelectEditable = $derived(
    nodeId.length > 0 &&
    leaf.elementType === 'int' &&
    !!(leaf.constraints?.mapEntries?.length)
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

  /** Current display value — pending if dirty, else canonical */
  let displayValue = $derived(localPending ?? leaf.value);

  /** String value for controlled text input */
  let inputStr = $derived(
    displayValue?.type === 'string' ? displayValue.value : (leaf.value?.type === 'string' ? leaf.value.value : '')
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

  // ── Edit handlers ──────────────────────────────────────────────────────────

  /** Validate a string value and update the pending store */
  function handleStringInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;
    const newVal: TreeConfigValue = { type: 'string', value: raw };
    const original = leaf.value ?? { type: 'string', value: '' };

    // Validate: max length
    const maxLen = leaf.size - 1;
    const encoder = new TextEncoder();
    const byteLen = encoder.encode(raw).length;
    const isValid = byteLen <= maxLen;

    const edit: PendingEdit = {
      key: editKey,
      nodeId,
      segmentOrigin,
      segmentName,
      address: leaf.address,
      space: leaf.space,
      size: leaf.size,
      elementType: leaf.elementType,
      fieldPath: leaf.path,
      fieldLabel: leaf.name,
      originalValue: original,
      pendingValue: newVal,
      validationState: isValid ? 'valid' : 'invalid',
      validationMessage: isValid ? null : `Text too long (max ${maxLen} characters)`,
      writeState: 'dirty',
      writeError: null,
      constraints: leaf.constraints,
    };

    pendingEditsStore.setEdit(editKey, edit);
    // Sync localPending with store — auto-clears if value reverted to original
    localPending = pendingEditsStore.getEdit(editKey)?.pendingValue ?? null;
  }

  /** Validate an integer value and update the pending store */
  function handleIntInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;
    const parsed = parseInt(raw, 10);

    if (isNaN(parsed)) {
      // Keep localPending as-is; mark invalid
      const original = leaf.value ?? { type: 'int', value: 0 };
      const newVal: TreeConfigValue = { type: 'int', value: 0 };
      const edit: PendingEdit = {
        key: editKey,
        nodeId,
        segmentOrigin,
        segmentName,
        address: leaf.address,
        space: leaf.space,
        size: leaf.size,
        elementType: leaf.elementType,
        fieldPath: leaf.path,
        fieldLabel: leaf.name,
        originalValue: original,
        pendingValue: newVal,
        validationState: 'invalid',
        validationMessage: 'Must be a valid number',
        writeState: 'dirty',
        writeError: null,
        constraints: leaf.constraints,
      };
      pendingEditsStore.setEdit(editKey, edit);
      return;
    }

    const newVal: TreeConfigValue = { type: 'int', value: parsed };
    const original = leaf.value ?? { type: 'int', value: 0 };

    // Validate: min/max
    const min = leaf.constraints?.min;
    const max = leaf.constraints?.max;
    const tooLow = min !== null && min !== undefined && parsed < min;
    const tooHigh = max !== null && max !== undefined && parsed > max;
    const isValid = !tooLow && !tooHigh;
    const validMsg = tooLow
      ? `Value must be between ${min} and ${max}`
      : tooHigh
      ? `Value must be between ${min} and ${max}`
      : null;

    const edit: PendingEdit = {
      key: editKey,
      nodeId,
      segmentOrigin,
      segmentName,
      address: leaf.address,
      space: leaf.space,
      size: leaf.size,
      elementType: leaf.elementType,
      fieldPath: leaf.path,
      fieldLabel: leaf.name,
      originalValue: original,
      pendingValue: newVal,
      validationState: isValid ? 'valid' : 'invalid',
      validationMessage: validMsg,
      writeState: 'dirty',
      writeError: null,
      constraints: leaf.constraints,
    };

    pendingEditsStore.setEdit(editKey, edit);
    // Sync localPending with store — auto-clears if value reverted to original
    localPending = pendingEditsStore.getEdit(editKey)?.pendingValue ?? null;
  }

  /** Handle int-with-mapEntries select change */
  function handleSelectChange(e: Event) {
    const raw = (e.target as HTMLSelectElement).value;
    const parsed = parseInt(raw, 10);
    const original = leaf.value ?? { type: 'int', value: 0 };
    const newVal: TreeConfigValue = { type: 'int', value: parsed };

    const edit: PendingEdit = {
      key: editKey,
      nodeId,
      segmentOrigin,
      segmentName,
      address: leaf.address,
      space: leaf.space,
      size: leaf.size,
      elementType: leaf.elementType,
      fieldPath: leaf.path,
      fieldLabel: leaf.name,
      originalValue: original,
      pendingValue: newVal,
      validationState: 'valid',
      validationMessage: null,
      writeState: 'dirty',
      writeError: null,
      constraints: leaf.constraints,
    };

    localPending = newVal;
    pendingEditsStore.setEdit(editKey, edit);
    // Sync localPending with store — auto-clears if value reverted to original
    localPending = pendingEditsStore.getEdit(editKey)?.pendingValue ?? null;
  }

  /** Validate a float value and update the pending store */
  function handleFloatInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;
    const parsed = parseFloat(raw);
    const original = leaf.value ?? { type: 'float', value: 0 };

    if (isNaN(parsed)) {
      // Invalid — record invalid state
      const newVal: TreeConfigValue = { type: 'float', value: 0 };
      const edit: PendingEdit = {
        key: editKey,
        nodeId,
        segmentOrigin,
        segmentName,
        address: leaf.address,
        space: leaf.space,
        size: leaf.size,
        elementType: leaf.elementType,
        fieldPath: leaf.path,
        fieldLabel: leaf.name,
        originalValue: original,
        pendingValue: newVal,
        validationState: 'invalid',
        validationMessage: 'Must be a valid number',
        writeState: 'dirty',
        writeError: null,
        constraints: leaf.constraints,
      };
      pendingEditsStore.setEdit(editKey, edit);
      return;
    }

    const newVal: TreeConfigValue = { type: 'float', value: parsed };

    // Validate min/max constraints
    const min = leaf.constraints?.min;
    const max = leaf.constraints?.max;
    const tooLow = min !== null && min !== undefined && parsed < min;
    const tooHigh = max !== null && max !== undefined && parsed > max;
    const isValid = !tooLow && !tooHigh;
    const validMsg = (!isValid) ? `Value must be between ${min} and ${max}` : null;

    const edit: PendingEdit = {
      key: editKey,
      nodeId,
      segmentOrigin,
      segmentName,
      address: leaf.address,
      space: leaf.space,
      size: leaf.size,
      elementType: leaf.elementType,
      fieldPath: leaf.path,
      fieldLabel: leaf.name,
      originalValue: original,
      pendingValue: newVal,
      validationState: isValid ? 'valid' : 'invalid',
      validationMessage: validMsg,
      writeState: 'dirty',
      writeError: null,
      constraints: leaf.constraints,
    };

    localPending = newVal;
    pendingEditsStore.setEdit(editKey, edit);
    // Sync localPending with store — auto-clears if value reverted to original
    localPending = pendingEditsStore.getEdit(editKey)?.pendingValue ?? null;
  }

  /** Validate an event ID in dotted-hex and update the pending store */
  function handleEventIdInput(e: Event) {
    const raw = (e.target as HTMLInputElement).value;
    const original = leaf.value ?? { type: 'eventId', bytes: [0, 0, 0, 0, 0, 0, 0, 0] };

    const parsedBytes = parseEventIdHex(raw);

    if (!parsedBytes) {
      // Invalid format — record invalid state without a displayable value
      const invalidVal: TreeConfigValue = {
        type: 'eventId',
        bytes: original.type === 'eventId' ? original.bytes : [0, 0, 0, 0, 0, 0, 0, 0],
      };
      const edit: PendingEdit = {
        key: editKey,
        nodeId,
        segmentOrigin,
        segmentName,
        address: leaf.address,
        space: leaf.space,
        size: leaf.size,
        elementType: leaf.elementType,
        fieldPath: leaf.path,
        fieldLabel: leaf.name,
        originalValue: original,
        pendingValue: invalidVal,
        validationState: 'invalid',
        validationMessage: 'Invalid event ID format — use HH.HH.HH.HH.HH.HH.HH.HH',
        writeState: 'dirty',
        writeError: null,
        constraints: leaf.constraints,
      };
      pendingEditsStore.setEdit(editKey, edit);
      return;
    }

    const newVal: TreeConfigValue = { type: 'eventId', bytes: parsedBytes };
    const edit: PendingEdit = {
      key: editKey,
      nodeId,
      segmentOrigin,
      segmentName,
      address: leaf.address,
      space: leaf.space,
      size: leaf.size,
      elementType: leaf.elementType,
      fieldPath: leaf.path,
      fieldLabel: leaf.name,
      originalValue: original,
      pendingValue: newVal,
      validationState: 'valid',
      validationMessage: null,
      writeState: 'dirty',
      writeError: null,
      constraints: leaf.constraints,
    };

    localPending = newVal;
    pendingEditsStore.setEdit(editKey, edit);
    // Sync localPending with store — auto-clears if value reverted to original
    localPending = pendingEditsStore.getEdit(editKey)?.pendingValue ?? null;
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

  function handleNavigateToBowties() {
    if (usedIn) {
      goto('/bowties?highlight=' + usedIn.event_id_hex);
    }
  }
</script>

<div
  class="field-row"
  class:compact={depth >= 3}
  class:dirty={isDirty && !isInvalid}
  class:invalid={isInvalid}
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
        {#each leaf.constraints.mapEntries as entry}
          <option value={entry.value}>{entry.label}</option>
        {/each}
      </select>
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

    {#if isInvalid && currentEdit?.validationMessage}
      <span class="validation-msg" role="alert">{currentEdit.validationMessage}</span>
    {/if}

    {#if hasWriteError && currentEdit?.writeError}
      <span class="write-error-msg" role="alert">⚠ {currentEdit.writeError}</span>
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
          aria-label="View bowtie connection for {bowtieName(usedIn)}"
        >{bowtieName(usedIn)}</button>
      </span>
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
</style>
