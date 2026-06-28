<script lang="ts">
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';

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

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') confirm();
    if (e.key === 'Escape') onCancel();
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
  }
</script>

<div class="dialog-backdrop" role="presentation" onclick={onCancel} onkeydown={handleKeydown}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="add-facility-title" tabindex={-1} onclick={(e) => e.stopPropagation()} onkeydown={handleKeydown}>
    <h2 id="add-facility-title" class="dialog-title">Add facility</h2>

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

    <div class="actions">
      <button type="button" class="btn-secondary" onclick={onCancel}>Cancel</button>
      <button type="button" class="btn-primary" onclick={confirm} disabled={!selectedTemplate()}>Add facility</button>
    </div>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .dialog {
    background: var(--surface-color, #fff);
    border-radius: 0.5rem;
    padding: 1.5rem;
    min-width: 22rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
  }
  .dialog-title {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .field-label {
    font-size: 0.85rem;
    color: var(--text-muted, #666);
  }
  .field select,
  .field input {
    padding: 0.4rem 0.6rem;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 0.25rem;
    background: var(--surface-color, #fff);
    color: var(--text-primary, #222);
    font-size: 0.95rem;
  }
  .error {
    color: var(--error-color, #b00020);
    font-size: 0.85rem;
    margin: 0;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }
  .btn-primary,
  .btn-secondary {
    padding: 0.4rem 0.9rem;
    border-radius: 0.25rem;
    border: 1px solid var(--border-color, #ccc);
    cursor: pointer;
    font-size: 0.9rem;
  }
  .btn-primary {
    background: var(--accent-color, #2563eb);
    color: white;
    border-color: transparent;
  }
  .btn-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .btn-secondary {
    background: var(--surface-color, #fff);
    color: var(--text-primary, #222);
  }
</style>
