<script lang="ts">
  /**
   * PillSelector — Fluent UI–styled searchable dropdown pill for selecting from a list.
   *
   * Used to navigate replicated CDI group instances (e.g. "Line 1" through "Line 16").
   * Compact button shows "Description (Index)" → click opens dropdown with search input.
   * Keyboard: arrows navigate, Enter selects, Escape closes.
   *
   * Only one PillSelector can be open at a time — opening one closes all others
   * via a CustomEvent('pill-selector-open') on window.
   *
   * The dropdown uses fixed positioning to escape any parent overflow constraints.
   *
   * Styled per Microsoft Fluent UI guidelines.
   * Spec: plan-cdiConfigNavigator, Step 1.
   */
  import { onMount, onDestroy } from 'svelte';

  export interface PillItem {
    value: number;
    label: string;
    description?: string;
  }

  /** Available items to select from */
  export let items: PillItem[] = [];
  /** Currently selected value */
  export let selected: number = 0;
  /** Callback when selection changes */
  export let onSelect: (value: number) => void = () => {};

  // ── Unique instance ID for mutual-exclusion ──
  let instanceId = `pill-${Math.random().toString(36).slice(2, 9)}`;

  let open = false;
  let searchText = '';
  let highlightIndex = 0;
  let dropdownEl: HTMLDivElement;
  let searchInputEl: HTMLInputElement;
  let buttonEl: HTMLButtonElement;

  /** Fixed position for the dropdown, computed from button rect */
  let dropdownStyle = '';

  $: selectedItem = items.find((i) => i.value === selected) ?? items[0];
  $: filteredItems = searchText
    ? items.filter(
        (i) =>
          i.label.toLowerCase().includes(searchText.toLowerCase()) ||
          (i.description ?? '').toLowerCase().includes(searchText.toLowerCase()),
      )
    : items;

  // Reset highlight when filtered list changes
  $: if (filteredItems) highlightIndex = 0;

  // ── Mutual-exclusion: close when another pill opens ──
  function handleOtherPillOpen(e: Event) {
    const detail = (e as CustomEvent<{ id: string }>).detail;
    if (detail.id !== instanceId && open) {
      close(false);  // don't refocus — another pill is taking over
    }
  }

  onMount(() => {
    window.addEventListener('pill-selector-open', handleOtherPillOpen);
  });

  onDestroy(() => {
    window.removeEventListener('pill-selector-open', handleOtherPillOpen);
  });

  function toggle() {
    if (open) {
      close();
    } else {
      openDropdown();
    }
  }

  function openDropdown() {
    // Notify all other pills to close
    window.dispatchEvent(new CustomEvent('pill-selector-open', { detail: { id: instanceId } }));

    open = true;
    searchText = '';
    highlightIndex = Math.max(
      0,
      filteredItems.findIndex((i) => i.value === selected),
    );
    // Compute fixed position from button rect
    if (buttonEl) {
      const rect = buttonEl.getBoundingClientRect();
      dropdownStyle = `position: fixed; top: ${rect.bottom + 2}px; left: ${rect.left}px; min-width: ${Math.max(180, rect.width)}px;`;
    }
    // Focus search input after Svelte renders the dropdown
    requestAnimationFrame(() => searchInputEl?.focus());
  }

  function close(refocus = true) {
    open = false;
    searchText = '';
    if (refocus) buttonEl?.focus();
  }

  function selectItem(item: PillItem) {
    onSelect(item.value);
    close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        highlightIndex = Math.min(highlightIndex + 1, filteredItems.length - 1);
        scrollHighlightIntoView();
        break;
      case 'ArrowUp':
        e.preventDefault();
        highlightIndex = Math.max(highlightIndex - 1, 0);
        scrollHighlightIntoView();
        break;
      case 'Enter':
        e.preventDefault();
        if (filteredItems[highlightIndex]) {
          selectItem(filteredItems[highlightIndex]);
        }
        break;
      case 'Escape':
        e.preventDefault();
        close();
        break;
    }
  }

  function scrollHighlightIntoView() {
    requestAnimationFrame(() => {
      const el = dropdownEl?.querySelector('.pill-option.highlighted');
      el?.scrollIntoView({ block: 'nearest' });
    });
  }

  /** Close dropdown if clicking outside */
  function handleWindowClick(e: MouseEvent) {
    if (open && dropdownEl && !dropdownEl.contains(e.target as Node) && !buttonEl.contains(e.target as Node)) {
      close();
    }
  }

  /** Reposition on scroll/resize */
  function handleWindowScroll() {
    if (open && buttonEl) {
      const rect = buttonEl.getBoundingClientRect();
      dropdownStyle = `position: fixed; top: ${rect.bottom + 2}px; left: ${rect.left}px; min-width: ${Math.max(180, rect.width)}px;`;
    }
  }
</script>

<svelte:window on:click={handleWindowClick} on:scroll|capture={handleWindowScroll} on:resize={handleWindowScroll} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="pill-selector" on:keydown={handleKeydown}>
  <button
    class="pill-button"
    class:active={open}
    bind:this={buttonEl}
    on:click|stopPropagation={toggle}
    aria-haspopup="listbox"
    aria-expanded={open}
    title={selectedItem?.description ?? selectedItem?.label ?? ''}
  >
    <span class="pill-label">{selectedItem?.label ?? '—'}</span>
    <svg class="pill-chevron" aria-hidden="true" viewBox="0 0 12 12" width="12" height="12">
      <path d={open ? 'M2.5 7.5 6 4l3.5 3.5' : 'M2.5 4.5 6 8l3.5-3.5'} fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </button>

  {#if open}
    <!-- Fixed-position dropdown to escape overflow:hidden parents -->
    <div class="pill-dropdown" bind:this={dropdownEl} role="listbox" style={dropdownStyle}>
      {#if items.length > 6}
        <div class="pill-search-wrap">
          <input
            bind:this={searchInputEl}
            bind:value={searchText}
            class="pill-search"
            type="text"
            placeholder="Search…"
            aria-label="Search items"
            on:click|stopPropagation
          />
        </div>
      {/if}

      <div class="pill-options" class:virtualized={filteredItems.length > 20}>
        {#each filteredItems as item, idx (item.value)}
          <button
            class="pill-option"
            class:highlighted={idx === highlightIndex}
            class:selected={item.value === selected}
            role="option"
            aria-selected={item.value === selected}
            on:click|stopPropagation={() => selectItem(item)}
            on:mouseenter={() => (highlightIndex = idx)}
          >
            <span class="option-label">{item.label}</span>
            {#if item.description}
              <span class="option-desc">{item.description}</span>
            {/if}
            {#if item.value === selected}
              <svg class="option-check" viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
                <path d="M2.5 6.5 5 9l4.5-6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            {/if}
          </button>
        {/each}

        {#if filteredItems.length === 0}
          <div class="pill-empty">No matches</div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* ══════════════════════════════════════════
     Fluent UI Design Tokens (scoped)
     ══════════════════════════════════════════ */

  .pill-selector {
    display: inline-flex;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  /* ── Pill Button (Fluent ComboBox trigger) ── */
  .pill-button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px 2px 10px;
    font-size: 12px;
    font-weight: 400;
    color: #242424;                               /* colorNeutralForeground1 */
    background: #f5f5f5;                           /* colorNeutralBackground1 */
    border: 1px solid #d1d1d1;                     /* colorNeutralStroke1 */
    border-radius: 4px;                            /* borderRadiusMedium */
    cursor: pointer;
    white-space: nowrap;
    max-width: 220px;
    line-height: 20px;
    transition: background-color 0.1s ease, border-color 0.1s ease;
  }

  .pill-button:hover {
    background: #ebebeb;                           /* colorNeutralBackground1Hover */
    border-color: #c7c7c7;
  }

  .pill-button:active,
  .pill-button.active {
    background: #e0e0e0;                           /* colorNeutralBackground1Pressed */
    border-color: #b3b3b3;
  }

  .pill-button:focus-visible {
    outline: 2px solid #0078d4;                    /* colorBrandStroke1 */
    outline-offset: 1px;
    border-color: transparent;
  }

  .pill-label {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pill-chevron {
    flex-shrink: 0;
    color: #616161;                                /* colorNeutralForeground3 */
  }

  /* ── Dropdown (Fluent Listbox) ── */
  .pill-dropdown {
    z-index: 9999;
    min-width: 180px;
    max-width: 320px;
    background: #fff;                              /* colorNeutralBackground1 */
    border: 1px solid #e0e0e0;                     /* colorNeutralStroke1 */
    border-radius: 4px;                            /* borderRadiusMedium */
    box-shadow: 0 2px 4px rgba(0,0,0,0.04),
                0 8px 16px rgba(0,0,0,0.12);       /* shadow16 */
    overflow: hidden;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }

  /* ── Search input ── */
  .pill-search-wrap {
    padding: 6px 6px 4px;
    border-bottom: 1px solid #f0f0f0;             /* colorNeutralStroke2 */
  }

  .pill-search {
    width: 100%;
    padding: 5px 8px;
    font-size: 12px;
    font-family: inherit;
    border: 1px solid #d1d1d1;
    border-radius: 4px;
    outline: none;
    background: #fff;
    color: #242424;
    transition: border-color 0.1s ease;
  }

  .pill-search:focus {
    border-color: #0078d4;                         /* colorBrandStroke1 */
    box-shadow: 0 0 0 1px #0078d4 inset;           /* Fluent "thick bottom border" feel */
  }

  /* ── Options list ── */
  .pill-options {
    max-height: 240px;
    overflow-y: auto;
    padding: 4px 0;
  }

  .pill-options.virtualized {
    max-height: 320px;
  }

  .pill-option {
    display: flex;
    flex-direction: column;
    position: relative;
    width: 100%;
    padding: 6px 28px 6px 10px;                   /* right padding for checkmark */
    border: none;
    background: none;
    cursor: pointer;
    text-align: left;
    font-size: 12px;
    font-family: inherit;
    color: #242424;
    line-height: 1.35;
    border-radius: 4px;
    margin: 0 4px;
    width: calc(100% - 8px);
    transition: background-color 0.05s ease;
  }

  .pill-option:hover,
  .pill-option.highlighted {
    background: #f5f5f5;                           /* colorNeutralBackground1Hover */
  }

  .pill-option.selected {
    font-weight: 600;
    color: #0078d4;                                /* colorBrandForeground1 */
  }

  .option-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .option-desc {
    font-size: 11px;
    color: #707070;                                /* colorNeutralForeground3 */
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .option-check {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    color: #0078d4;                                /* colorBrandForeground1 */
  }

  .pill-empty {
    padding: 12px 10px;
    font-size: 12px;
    color: #707070;
    text-align: center;
  }
</style>
