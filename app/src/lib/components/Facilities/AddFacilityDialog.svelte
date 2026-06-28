<script lang="ts">
  /**
   * AddFacilityDialog — modal for creating a new facility (Spec 018).
   *
   * dialog-shell-refactor (Slice 4): wraps the Fluent `Dialog` shell.
   * Body uses a native `<form>` so Enter on a focused input submits via
   * the primary `Add facility` button. Esc / overlay / × → cancel (shell).
   */
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';

  let {
    templates,
    onConfirm,
    onCancel,
  }: {
    templates: BehaviorTemplate[];
    onConfirm: (template: BehaviorTemplate, name: string) => void;
    onCancel: () => void;
  } = $props();

  let selectedTemplateId = $state<string | undefined>(undefined);
  let name = $state('');
  let error = $state<string | null>(null);

  // Default the selection to the first template once props are available.
  $effect(() => {
    if (selectedTemplateId === undefined && templates.length > 0) {
      selectedTemplateId = templates[0].templateId;
    }
  });

  function selectedTemplate(): BehaviorTemplate | undefined {
    return templates.find((t) => t.templateId === selectedTemplateId);
  }

  function confirm() {
    const trimmed = name.trim();
    if (trimmed.length === 0) {
      error = 'Please enter a name for the facility.';
      return;
    }
    const tmpl = selectedTemplate();
    if (!tmpl) {
      error = 'Please choose a behavior template.';
      return;
    }
    onConfirm(tmpl, trimmed);
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
  }
</script>

<Dialog
  open
  width="md"
  ariaLabel="Add facility"
  initialFocus="none"
  {onCancel}
>
  {#snippet title()}
    <DialogTitle>Add facility</DialogTitle>
  {/snippet}

  <form
    class="af-form"
    onsubmit={(e) => { e.preventDefault(); confirm(); }}
  >
    <label class="field">
      <span class="field-label">Behavior template</span>
      <select bind:value={selectedTemplateId} disabled={templates.length <= 1}>
        {#each templates as t (t.templateId)}
          <option value={t.templateId}>{t.displayName}</option>
        {/each}
      </select>
    </label>

    <label class="field">
      <span class="field-label">Name</span>
      <input
        type="text"
        bind:value={name}
        placeholder="e.g. Block 5"
        use:focusInput
        aria-invalid={!!error}
      />
    </label>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <!-- Hidden submit captures Enter; visible action lives in the footer. -->
    <button type="submit" class="af-hidden-submit" tabindex="-1" aria-hidden="true"></button>
  </form>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button
        appearance="primary"
        disabled={!selectedTemplate()}
        onclick={confirm}
      >Add facility</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .af-form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field-label {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground2);
    font-weight: 500;
  }
  .field select,
  .field input {
    padding: 6px 10px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase300);
  }
  .field select:focus,
  .field input:focus {
    outline: none;
    border-color: var(--fluent-strokeFocus2);
    box-shadow: 0 0 0 2px var(--fluent-strokeFocusHalo);
  }
  .error {
    color: var(--fluent-dangerBackground);
    font-size: var(--fluent-fontSizeBase200);
    margin: 0;
  }
  .af-hidden-submit {
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
