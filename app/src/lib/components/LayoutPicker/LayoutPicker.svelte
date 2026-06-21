<script lang="ts">
  /**
   * LayoutPicker — startup gate (Spec 013 / S6).
   *
   * Renders when no layout is active. Shows the known-layout list (with a
   * Remove action per row), plus "New Layout" and "Browse…" buttons. The
   * picker owns no async work — it dispatches user intent up to the route,
   * which delegates to `startupOrchestrator`.
   *
   * Only one of these surfaces is visible at any time:
   *   - the picker (this component) when no layout is active,
   *   - or the main UI (toolbar + content area) when a layout is open.
   */
  import { open } from '@tauri-apps/plugin-dialog';
  import type { KnownLayoutEntry } from '$lib/api/startup';
  import LayoutEntry from './LayoutEntry.svelte';
  import NewLayoutDialog from './NewLayoutDialog.svelte';

  interface Props {
    entries: KnownLayoutEntry[];
    loaded: boolean;
    busy?: boolean;
    onOpen: (entry: KnownLayoutEntry) => void;
    onBrowse: (path: string) => void;
    onCreate: (args: { name: string; path: string }) => void;
    onRemove: (entry: KnownLayoutEntry) => void;
  }

  let { entries, loaded, busy = false, onOpen, onBrowse, onCreate, onRemove }: Props = $props();

  let newLayoutVisible = $state(false);
  let browseError = $state<string | null>(null);

  async function handleBrowse(): Promise<void> {
    browseError = null;
    try {
      const selected = await open({
        title: 'Open Layout Folder',
        directory: true,
        multiple: false,
      });
      if (typeof selected === 'string') {
        onBrowse(selected);
      }
    } catch (err) {
      browseError = err instanceof Error ? err.message : String(err);
    }
  }

  function handleCreate(args: { name: string; path: string }): void {
    newLayoutVisible = false;
    onCreate(args);
  }
</script>

<section class="lp-shell" aria-label="Layout picker">
  <div class="lp-card">
    <header class="lp-header">
      <h1 class="lp-title">Bowties</h1>
      <p class="lp-subtitle">Choose a layout to get started.</p>
    </header>

    <div class="lp-list-wrap">
      {#if !loaded}
        <p class="lp-empty" aria-live="polite">Loading known layouts…</p>
      {:else if entries.length === 0}
        <p class="lp-empty">No known layouts yet. Create one or browse to an existing layout file.</p>
      {:else}
        <ul class="lp-list" data-testid="layout-picker-list">
          {#each entries as entry (entry.path)}
            <li>
              <LayoutEntry {entry} disabled={busy} {onOpen} {onRemove} />
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    {#if browseError}
      <p class="lp-error" role="alert">{browseError}</p>
    {/if}

    <div class="lp-actions">
      <button
        type="button"
        class="lp-btn lp-btn-primary"
        onclick={() => (newLayoutVisible = true)}
        disabled={busy}
        data-testid="layout-picker-new"
      >New Layout</button>
      <button
        type="button"
        class="lp-btn"
        onclick={handleBrowse}
        disabled={busy}
        data-testid="layout-picker-browse"
      >Browse…</button>
    </div>
  </div>
</section>

<NewLayoutDialog
  visible={newLayoutVisible}
  {busy}
  onCancel={() => (newLayoutVisible = false)}
  onCreate={handleCreate}
/>

<style>
  .lp-shell {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #f3f4f6;
    z-index: 1200;
    overflow: auto;
    padding: 24px;
    box-sizing: border-box;
  }
  .lp-card {
    background: #ffffff;
    border-radius: 8px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.08);
    padding: 28px 32px;
    width: 100%;
    max-width: 560px;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }
  .lp-header {
    margin-bottom: 20px;
  }
  .lp-title {
    margin: 0 0 4px 0;
    font-size: 24px;
    font-weight: 700;
    color: #111827;
  }
  .lp-subtitle {
    margin: 0;
    font-size: 14px;
    color: #6b7280;
  }
  .lp-list-wrap {
    margin: 16px 0;
    max-height: 360px;
    overflow: auto;
  }
  .lp-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .lp-empty {
    color: #6b7280;
    font-size: 13px;
    text-align: center;
    margin: 24px 0;
  }
  .lp-error {
    color: #b91c1c;
    font-size: 13px;
    margin: 0 0 12px 0;
  }
  .lp-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    border-top: 1px solid #e5e7eb;
    padding-top: 16px;
  }
  .lp-btn {
    padding: 8px 16px;
    border-radius: 4px;
    border: 1px solid #d0d5dd;
    background: #ffffff;
    color: #374151;
    font-size: 13px;
    cursor: pointer;
  }
  .lp-btn:hover:not(:disabled) {
    background: #f3f4f6;
  }
  .lp-btn-primary {
    background: #2563eb;
    color: #ffffff;
    border-color: #2563eb;
  }
  .lp-btn-primary:hover:not(:disabled) {
    background: #1d4ed8;
  }
  .lp-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
