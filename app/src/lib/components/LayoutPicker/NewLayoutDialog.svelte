<script lang="ts">
  /**
   * NewLayoutDialog — modal for creating a new layout (Spec 013 / S6).
   *
   * Collects a display name + parent directory, builds the layout folder
   * path (`<parent>/<name>/`), and emits a single `onCreate` event with
   * both. The picker's orchestrator owns the actual folder/registry work.
   *
   * dialog-shell-refactor (Slice 4): wraps the Fluent `Dialog` shell.
   * While `busy`, the dialog locks (no Esc, overlay, or × — `closable={false}`).
   */
  import { open } from '@tauri-apps/plugin-dialog';
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';

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
</script>

<Dialog
  open={visible}
  width="md"
  ariaLabel="New layout"
  closable={!busy}
  initialFocus="none"
  zIndex={1600}
  onCancel={cancel}
>
  {#snippet title()}
    <DialogTitle>New Layout</DialogTitle>
  {/snippet}

  <form
    class="nl-form"
    onsubmit={(e) => { e.preventDefault(); submit(); }}
  >
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
        <Button appearance="secondary" onclick={pickDirectory} disabled={busy}>
          Browse…
        </Button>
      </div>
    </div>

    {#if derivedPath}
      <p class="nl-preview" title={derivedPath}>
        Will create folder: <code>{derivedPath}</code>
      </p>
    {/if}

    {#if errorMessage}
      <p class="nl-error" role="alert">{errorMessage}</p>
    {/if}

    <button type="submit" class="nl-hidden-submit" tabindex="-1" aria-hidden="true"></button>
  </form>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={cancel} disabled={busy}>Cancel</Button>
      <Button
        appearance="primary"
        onclick={submit}
        disabled={busy || !name.trim() || !directory}
        dataTestid="new-layout-create"
      >Create Layout</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .nl-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  .nl-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .nl-field label {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground2);
    font-weight: 500;
  }
  .nl-field input {
    padding: 6px 10px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase300);
  }
  .nl-field input:focus {
    outline: none;
    border-color: var(--fluent-strokeFocus2);
    box-shadow: 0 0 0 2px var(--fluent-strokeFocusHalo);
  }
  .nl-dir-row {
    display: flex;
    gap: 8px;
    align-items: stretch;
  }
  .nl-dir-row input {
    flex: 1;
  }
  .nl-preview {
    margin: 0;
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground3);
    word-break: break-all;
  }
  .nl-preview code {
    background: var(--fluent-neutralBackground3);
    padding: 1px 4px;
    border-radius: 3px;
  }
  .nl-error {
    margin: 0;
    color: var(--fluent-dangerBackground);
    font-size: var(--fluent-fontSizeBase200);
  }
  .nl-hidden-submit {
    position: absolute;
    width: 0;
    height: 0;
    padding: 0;
    border: 0;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
  }
</style>
