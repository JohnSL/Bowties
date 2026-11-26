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
  import SingleSelectList from '$lib/components/SingleSelectList/SingleSelectList.svelte';
  import FacilityTypeIcon from './FacilityTypeIcon.svelte';

  /** A facility type offered in the list, with an optional one-line description; an `unavailableNote` shows it disabled. */
  type FacilityChoice = BehaviorTemplate & { description?: string; examples?: string; unavailableNote?: string };

  let {
    templates,
    onConfirm,
    onCancel,
  }: {
    templates: FacilityChoice[];
    onConfirm: (template: BehaviorTemplate, name: string) => void;
    onCancel: () => void;
  } = $props();

  let selectedTemplateId = $state<string | undefined>(undefined);
  let name = $state('');
  let error = $state<string | null>(null);

  // Default the selection to the first available template once props are available.
  $effect(() => {
    const first = templates.find((t) => !t.unavailableNote);
    if (selectedTemplateId === undefined && first) {
      selectedTemplateId = first.templateId;
    }
  });

  const templateItems = $derived(
    templates.map((t) => ({ key: t.templateId, disabled: !!t.unavailableNote })),
  );

  function templateByKey(key: string): FacilityChoice | undefined {
    return templates.find((t) => t.templateId === key);
  }

  function selectedTemplate(): BehaviorTemplate | undefined {
    const choice = selectedTemplateId ? templateByKey(selectedTemplateId) : undefined;
    return choice && !choice.unavailableNote ? choice : undefined;
  }

  function confirm() {
    const trimmed = name.trim();
    const tmpl = selectedTemplate();
    if (!tmpl) {
      error = 'Please choose a behavior template.';
      return;
    }
    if (trimmed.length === 0) {
      error = 'Please enter a name for the facility.';
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
    <div class="field">
      <span class="field-label">Behavior template</span>
      <SingleSelectList
        items={templateItems}
        bind:selectedKey={selectedTemplateId}
        ariaLabel="Behavior templates"
        name="behavior-template"
      >
        {#snippet row(key)}
          {@const choice = templateByKey(key)}
          <span class="af-choice" class:unavailable={!!choice?.unavailableNote}>
            <span class="af-template-icon"><FacilityTypeIcon templateId={key} /></span>
            <span class="af-template-text">
              <span class="af-template-name">{choice?.displayName ?? key}</span>
              {#if choice?.description}
                <span class="af-template-desc">{choice.description}</span>
              {/if}
              {#if choice?.examples}
                <span class="af-template-examples">{choice.examples}</span>
              {/if}
              {#if choice?.unavailableNote}
                <span class="af-template-note">{choice.unavailableNote}</span>
              {/if}
            </span>
          </span>
        {/snippet}
      </SingleSelectList>
    </div>

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
        disabled={!selectedTemplate() || name.trim().length === 0}
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
  .field input {
    padding: 6px 10px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase300);
  }
  .field input:focus {
    outline: none;
    border-color: var(--fluent-strokeFocus2);
    box-shadow: 0 0 0 2px var(--fluent-strokeFocusHalo);
  }
  .af-choice {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  /* fixed width so names line up even for a type without a picture */
  .af-template-icon {
    display: flex;
    flex: 0 0 44px;
    min-height: 34px;
    align-items: center;
  }
  .af-choice.unavailable .af-template-icon {
    opacity: 0.45;
    filter: grayscale(1);
  }
  .af-template-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding-block: 3px;
  }
  .af-template-name {
    font-weight: 500;
    color: var(--fluent-neutralForeground1);
  }
  .af-template-desc {
    font-size: var(--fluent-fontSizeBase200);
    line-height: 1.35;
    color: var(--fluent-neutralForeground2);
  }
  .af-template-examples {
    font-size: var(--fluent-fontSizeBase200);
    font-style: italic;
    color: var(--fluent-neutralForeground3);
  }
  .af-template-note {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground3);
  }
  .af-choice.unavailable .af-template-name,
  .af-choice.unavailable .af-template-desc {
    color: var(--fluent-neutralForegroundDisabled, #a0a0a0);
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
