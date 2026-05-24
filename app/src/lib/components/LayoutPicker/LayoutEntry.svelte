<script lang="ts">
  /**
   * LayoutEntry — one row in the layout picker (Spec 013 / S6).
   *
   * Displays the layout's name, file path, and last-opened timestamp.
   * Click on the row body opens the layout; the trailing Remove button
   * unregisters the entry from the known-layouts registry (without
   * deleting the file on disk).
   */
  import type { KnownLayoutEntry } from '$lib/api/startup';

  interface Props {
    entry: KnownLayoutEntry;
    disabled?: boolean;
    onOpen: (entry: KnownLayoutEntry) => void;
    onRemove: (entry: KnownLayoutEntry) => void;
  }

  let { entry, disabled = false, onOpen, onRemove }: Props = $props();

  let formattedLastOpened = $derived(formatLastOpened(entry.lastOpened));

  function formatLastOpened(iso: string): string {
    try {
      const d = new Date(iso);
      if (Number.isNaN(d.getTime())) return iso;
      return d.toLocaleString(undefined, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      });
    } catch {
      return iso;
    }
  }

  function handleOpen(): void {
    if (disabled) return;
    onOpen(entry);
  }

  function handleRemove(e: MouseEvent | KeyboardEvent): void {
    e.stopPropagation();
    if (disabled) return;
    onRemove(entry);
  }
</script>

<div class="le-row" class:le-disabled={disabled}>
  <button
    type="button"
    class="le-body"
    onclick={handleOpen}
    disabled={disabled}
    data-testid="layout-entry-open"
  >
    <span class="le-name">{entry.name}</span>
    <span class="le-path" title={entry.path}>{entry.path}</span>
    <span class="le-date">Last opened {formattedLastOpened}</span>
  </button>
  <button
    type="button"
    class="le-remove"
    onclick={handleRemove}
    disabled={disabled}
    aria-label="Remove {entry.name} from list"
    title="Remove from list (does not delete files)"
    data-testid="layout-entry-remove"
  >
    ✕
  </button>
</div>

<style>
  .le-row {
    display: flex;
    align-items: stretch;
    border: 1px solid #d0d5dd;
    border-radius: 6px;
    background: #ffffff;
    margin-bottom: 8px;
    overflow: hidden;
    transition: border-color 120ms;
  }
  .le-row:hover {
    border-color: #2563eb;
  }
  .le-disabled {
    opacity: 0.55;
  }

  .le-body {
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: 1fr;
    grid-row-gap: 2px;
    align-items: start;
    text-align: left;
    padding: 10px 14px;
    background: transparent;
    border: 0;
    cursor: pointer;
    font: inherit;
  }
  .le-body:disabled {
    cursor: not-allowed;
  }
  .le-body:focus-visible {
    outline: 2px solid #2563eb;
    outline-offset: -2px;
  }

  .le-name {
    font-weight: 600;
    color: #111827;
    font-size: 14px;
  }
  .le-path {
    color: #6b7280;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .le-date {
    color: #9ca3af;
    font-size: 11px;
  }

  .le-remove {
    flex: 0 0 auto;
    width: 36px;
    background: transparent;
    border: 0;
    border-left: 1px solid #e5e7eb;
    color: #9ca3af;
    font-size: 16px;
    cursor: pointer;
  }
  .le-remove:hover:not(:disabled) {
    background: #fef2f2;
    color: #b91c1c;
  }
  .le-remove:focus-visible {
    outline: 2px solid #2563eb;
    outline-offset: -2px;
  }
</style>
