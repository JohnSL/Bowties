<script lang="ts">
  /**
   * NewLayoutDialog — modal for creating a new layout (Spec 013 / S6).
   *
   * Collects a display name + parent directory, builds the layout folder
   * path (`<parent>/<name>/`), and emits a single `onCreate` event with
   * both. The picker's orchestrator owns the actual folder/registry work.
   */
  import { open } from '@tauri-apps/plugin-dialog';

  interface Props {
    visible: boolean;
    busy?: boolean;
    onCancel: () => void;
    onCreate: (args: { name: string; path: string }) => void;
  }

  let { visible, busy = false, onCancel, onCreate }: Props = $props();

  let name = $state('');
  let directory = $state('');
  let errorMessage = $state<string | null>(null);

  // Sanitise the folder name — strip path separators and trailing dots/spaces
  // that would otherwise break on Windows.
  let filenameSafeName = $derived(name.trim().replace(/[\\/:*?"<>|]/g, '_').replace(/[. ]+$/, ''));
  let derivedPath = $derived(buildDerivedPath(directory, filenameSafeName));

  function buildDerivedPath(dir: string, folderName: string): string {
    if (!dir || !folderName) return '';
    const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
    const trimmed = dir.replace(/[\\/]+$/, '');
    return `${trimmed}${sep}${folderName}`;
  }

  async function pickDirectory(): Promise<void> {
    try {
      const selected = await open({
        title: 'Choose Layout Location',
        directory: true,
        multiple: false,
      });
      if (typeof selected === 'string') {
        directory = selected;
      }
    } catch (err) {
      errorMessage = err instanceof Error ? err.message : String(err);
    }
  }

  function reset(): void {
    name = '';
    directory = '';
    errorMessage = null;
  }

  function cancel(): void {
    reset();
    onCancel();
  }

  function submit(): void {
    errorMessage = null;
    const trimmed = name.trim();
    if (!trimmed) {
      errorMessage = 'Please enter a name for the layout.';
      return;
    }
    if (!directory) {
      errorMessage = 'Please choose a folder for the layout.';
      return;
    }
    if (!derivedPath) {
      errorMessage = 'Could not build a valid file path.';
      return;
    }
    onCreate({ name: trimmed, path: derivedPath });
  }

  // Reset internal state whenever the dialog is closed.
  $effect(() => {
    if (!visible) reset();
  });

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape' && !busy) {
      e.preventDefault();
      cancel();
    }
  }
</script>

{#if visible}
  <div class="nl-overlay" role="presentation">
    <div
      class="nl-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="nl-title"
      onkeydown={handleKeydown}
      tabindex="-1"
    >
      <h2 id="nl-title" class="nl-title">New Layout</h2>

      <div class="nl-field">
        <label for="nl-name">Name</label>
        <input
          id="nl-name"
          type="text"
          bind:value={name}
          disabled={busy}
          placeholder="e.g. Yard"
          autocomplete="off"
          data-testid="new-layout-name"
        />
      </div>

      <div class="nl-field">
        <label for="nl-dir">Location</label>
        <div class="nl-dir-row">
          <input
            id="nl-dir"
            type="text"
            bind:value={directory}
            disabled={busy}
            placeholder="Folder where the layout will be created"
            data-testid="new-layout-directory"
          />
          <button
            type="button"
            class="nl-browse"
            onclick={pickDirectory}
            disabled={busy}
          >Browse…</button>
        </div>
      </div>

      {#if derivedPath}
        <p class="nl-preview" title={derivedPath}>Will create folder: <code>{derivedPath}</code></p>
      {/if}

      {#if errorMessage}
        <p class="nl-error" role="alert">{errorMessage}</p>
      {/if}

      <div class="nl-actions">
        <button type="button" class="nl-cancel" onclick={cancel} disabled={busy}>Cancel</button>
        <button
          type="button"
          class="nl-create"
          onclick={submit}
          disabled={busy || !name.trim() || !directory}
          data-testid="new-layout-create"
        >Create Layout</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .nl-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1600;
  }
  .nl-dialog {
    background: #ffffff;
    border-radius: 6px;
    padding: 20px 24px;
    min-width: 360px;
    max-width: 520px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
  }
  .nl-title {
    margin: 0 0 16px 0;
    font-size: 18px;
    font-weight: 600;
    color: #111827;
  }
  .nl-field {
    display: block;
    margin-bottom: 12px;
  }
  .nl-field label {
    display: block;
    font-size: 12px;
    color: #4b5563;
    margin-bottom: 4px;
  }
  .nl-field input[type='text'] {
    width: 100%;
    padding: 6px 8px;
    font-size: 14px;
    border: 1px solid #d0d5dd;
    border-radius: 4px;
    box-sizing: border-box;
  }
  .nl-dir-row {
    display: flex;
    gap: 8px;
  }
  .nl-dir-row input {
    flex: 1 1 auto;
  }
  .nl-browse {
    flex: 0 0 auto;
    padding: 6px 12px;
    border: 1px solid #d0d5dd;
    border-radius: 4px;
    background: #f9fafb;
    cursor: pointer;
    font-size: 13px;
  }
  .nl-browse:hover:not(:disabled) {
    background: #f3f4f6;
  }
  .nl-preview {
    font-size: 12px;
    color: #6b7280;
    margin: 4px 0 0;
    word-break: break-all;
  }
  .nl-preview code {
    background: #f3f4f6;
    padding: 1px 4px;
    border-radius: 3px;
  }
  .nl-error {
    color: #b91c1c;
    font-size: 13px;
    margin: 12px 0 0;
  }
  .nl-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
  .nl-cancel,
  .nl-create {
    padding: 6px 14px;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
    border: 1px solid #d0d5dd;
  }
  .nl-cancel {
    background: #ffffff;
    color: #374151;
  }
  .nl-cancel:hover:not(:disabled) {
    background: #f3f4f6;
  }
  .nl-create {
    background: #2563eb;
    color: #ffffff;
    border-color: #2563eb;
  }
  .nl-create:hover:not(:disabled) {
    background: #1d4ed8;
  }
  .nl-create:disabled,
  .nl-cancel:disabled,
  .nl-browse:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
