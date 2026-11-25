<script lang="ts">
  /**
   * AddBoardDialog — modal picker for adding a placeholder board to the
   * active offline layout (Spec 014 / S8 + S8.5).
   *
   * Lists bundled board-model profiles (FR-018a) sorted by manufacturer
   * and model. Implicit-naming pivot (2026-05-25): the dialog no longer
   * prompts for a name — the user just picks a profile and confirms. The
   * sidebar falls back to `{manufacturer} — {model}` until the CDI User
   * Name leaf is edited.
   *
   * dialog-shell-refactor (Slice 4): wraps the Fluent `Dialog` shell.
   * Body uses a native `<form>` so Enter on the focused radio submits via
   * the primary `Add board` button. Board models render in the shared
   * `SingleSelectList` (model with manufacturer beneath), matching the
   * other add dialogs. Esc / overlay / × → cancel (shell).
   */
  import { onMount, tick } from 'svelte';
  import { listBundledProfiles, type BundledProfileSummary } from '$lib/api/layout';
  import { addPlaceholderBoard } from '$lib/orchestration/placeholderBoardOrchestrator';
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';
  import SingleSelectList from './SingleSelectList/SingleSelectList.svelte';

  interface Props {
    /** Called when the user cancels (Escape, backdrop click, ×, or Cancel). */
    onCancel: () => void;
    /** Called after a successful add, with the new `placeholder:<uuidv4>` NodeKey. */
    onAdded: (nodeKey: string) => void;
  }

  let { onCancel, onAdded }: Props = $props();

  let profiles = $state<BundledProfileSummary[] | null>(null);
  let loadError = $state<string | null>(null);
  let selectedStem = $state<string | undefined>(undefined);
  let submitting = $state(false);
  let submitError = $state<string | null>(null);

  let listContainer: HTMLDivElement | undefined = $state();

  const canSubmit = $derived(selectedStem !== undefined && !submitting);
  const profileItems = $derived((profiles ?? []).map((p) => ({ key: p.stem, disabled: submitting })));
  const profileByStem = (stem: string) => profiles?.find((p) => p.stem === stem);

  async function loadProfiles() {
    try {
      const result = await listBundledProfiles();
      profiles = result;
      if (result.length > 0 && selectedStem === undefined) {
        selectedStem = result[0].stem;
        await tick();
        listContainer?.querySelector<HTMLInputElement>('input[type="radio"]:checked')?.focus();
      }
    } catch (err) {
      loadError = String(err);
    }
  }

  async function handleSubmit() {
    if (!canSubmit || selectedStem === undefined) return;
    submitting = true;
    submitError = null;
    try {
      const { nodeKey } = await addPlaceholderBoard({ profileStem: selectedStem });
      onAdded(nodeKey);
    } catch (err) {
      submitError = String(err);
    } finally {
      submitting = false;
    }
  }

  onMount(() => {
    void loadProfiles();
  });
</script>

<Dialog
  open
  width="md"
  ariaLabel="Add placeholder board"
  initialFocus="none"
  onCancel={submitting ? () => {} : onCancel}
>
  {#snippet title()}
    <DialogTitle>Add placeholder board</DialogTitle>
  {/snippet}

  <form
    class="abd-form"
    onsubmit={(e) => { e.preventDefault(); void handleSubmit(); }}
  >
    <div class="abd-field" bind:this={listContainer}>
      <span class="abd-label">Board model</span>
      {#if loadError}
        <div class="abd-error">Failed to load profiles: {loadError}</div>
      {:else if profiles === null}
        <div class="abd-loading">Loading…</div>
      {:else}
        <SingleSelectList
          items={profileItems}
          bind:selectedKey={selectedStem}
          ariaLabel="Board model"
          name="board-model"
          emptyMessage="No bundled board profiles are available."
        >
          {#snippet row(stem)}
            {@const profile = profileByStem(stem)}
            <span class="abd-choice">
              <span class="abd-model">{profile?.model ?? stem}</span>
              {#if profile}
                <span class="abd-manufacturer">{profile.manufacturer}</span>
              {/if}
            </span>
          {/snippet}
        </SingleSelectList>
      {/if}
    </div>

    {#if submitError}
      <div class="abd-error">{submitError}</div>
    {/if}

    <button type="submit" class="abd-hidden-submit" tabindex="-1" aria-hidden="true"></button>
  </form>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel} disabled={submitting}>
        Cancel
      </Button>
      <Button appearance="primary" onclick={handleSubmit} disabled={!canSubmit}>
        Add board
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .abd-form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin: 0;
  }
  .abd-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .abd-label {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground2);
    font-weight: 500;
  }
  .abd-choice {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .abd-model {
    color: var(--fluent-neutralForeground1);
    font-size: var(--fluent-fontSizeBase300);
  }
  .abd-manufacturer {
    color: var(--fluent-neutralForeground3);
    font-size: var(--fluent-fontSizeBase200);
  }
  .abd-loading {
    color: var(--fluent-neutralForeground3);
    font-style: italic;
    padding: 6px 0;
  }
  .abd-error {
    color: var(--fluent-dangerBackground);
    font-size: var(--fluent-fontSizeBase200);
  }
  .abd-hidden-submit {
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
