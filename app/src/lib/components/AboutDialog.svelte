<script lang="ts">
  /**
   * AboutDialog — Modal showing application name, version, copyright, and links.
   *
   * dialog-shell-refactor (Slice 7): wraps the Fluent `Dialog` shell. The
   * action Close button receives initial focus via `initialFocus="last"` so
   * Enter (on the focused button) closes the dialog, preserving the prior
   * "Esc or Enter → close" keyboard contract. Esc / overlay / × all map to
   * `onClose` via the shell's `onCancel`.
   *
   * `zIndex={2000}` preserves the prior stacking (above other dialogs).
   */
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  let version = $state('');

  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      version = 'unknown';
    }
  });
</script>

<Dialog
  open
  width="sm"
  ariaLabel="About Bowties"
  initialFocus="last"
  zIndex={2000}
  onCancel={onClose}
>
  {#snippet title()}
    <DialogTitle>About Bowties</DialogTitle>
  {/snippet}

  <div class="ab-body">
    <h2 class="ab-name">Bowties</h2>
    <p class="ab-version">Version {version}</p>
    <p class="ab-description">An LCC/OpenLCB node configuration tool</p>
    <p class="ab-copyright">Copyright © 2026 John Socha-Leialoha</p>
    <p class="ab-license">Licensed under MIT or Apache-2.0</p>
    <p class="ab-link">
      <a href="https://github.com/JohnSL/Bowties" target="_blank" rel="noopener noreferrer">
        github.com/JohnSL/Bowties
      </a>
    </p>
  </div>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="primary" onclick={onClose}>Close</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .ab-body {
    text-align: center;
  }
  .ab-name {
    margin: 0 0 4px 0;
    font-size: 22px;
    font-weight: 700;
    color: var(--fluent-neutralForeground1);
  }
  .ab-version {
    margin: 0 0 16px 0;
    color: var(--fluent-neutralForeground3);
  }
  .ab-description {
    margin: 0 0 12px 0;
    color: var(--fluent-neutralForeground1);
  }
  .ab-copyright,
  .ab-license {
    margin: 0 0 4px 0;
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground3);
  }
  .ab-link {
    margin: 12px 0 0 0;
    font-size: var(--fluent-fontSizeBase200);
  }
  .ab-link a {
    color: var(--fluent-brandBackground);
    text-decoration: none;
  }
  .ab-link a:hover {
    text-decoration: underline;
  }
</style>
