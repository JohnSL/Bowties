<!--
  T034: RoleClassifyPrompt.svelte
  Inline prompt asking the user to classify an ambiguous event slot as
  Producer or Consumer.

  Props:
    elementName: string — display name of the element being classified
    onClassify: (role: 'Producer' | 'Consumer') => void
    onCancel: () => void
-->

<script lang="ts">
  interface Props {
    elementName: string;
    onClassify: (role: 'Producer' | 'Consumer') => void;
    onCancel?: () => void;
  }

  let { elementName, onClassify, onCancel }: Props = $props();
</script>

<div class="classify-prompt" role="dialog" aria-label="Classify role for {elementName}">
  <div class="prompt-header">
    <span class="prompt-icon">?</span>
    <span class="prompt-text">
      What role does <strong class="element-name">{elementName}</strong> play?
    </span>
  </div>
  <div class="prompt-actions">
    <button
      class="classify-btn classify-btn--producer"
      onclick={() => onClassify('Producer')}
      title="This slot produces (sends) events"
    >
      ▲ Producer
    </button>
    <button
      class="classify-btn classify-btn--consumer"
      onclick={() => onClassify('Consumer')}
      title="This slot consumes (receives) events"
    >
      ▼ Consumer
    </button>
    {#if onCancel}
      <button class="classify-btn classify-btn--cancel" onclick={onCancel}>
        Cancel
      </button>
    {/if}
  </div>
</div>

<style>
  .classify-prompt {
    background: #fffbf0;
    border: 1px solid #f59e0b;
    border-radius: 6px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .prompt-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.82rem;
    color: #374151;
  }

  .prompt-icon {
    font-size: 0.85rem;
    font-weight: 700;
    color: #d97706;
    background: #fef3c7;
    border: 1px solid #fde68a;
    border-radius: 50%;
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .element-name {
    font-family: 'ui-monospace', monospace;
    font-size: 0.78rem;
    color: #1f2937;
  }

  .prompt-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .classify-btn {
    padding: 4px 12px;
    font-size: 0.78rem;
    font-weight: 600;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.15s, border-color 0.15s;
  }

  .classify-btn--producer {
    color: #0b6a0b;
    background: #dff6dd;
    border-color: #a3cfb4;
  }

  .classify-btn--producer:hover {
    background: #c6efce;
    border-color: #0b6a0b;
  }

  .classify-btn--consumer {
    color: #0078d4;
    background: #deecf9;
    border-color: #b4d6fa;
  }

  .classify-btn--consumer:hover {
    background: #c7e0f4;
    border-color: #0078d4;
  }

  .classify-btn--cancel {
    color: #6b7280;
    background: #f9fafb;
    border-color: #d1d5db;
    margin-left: auto;
  }

  .classify-btn--cancel:hover {
    background: #f3f4f6;
    border-color: #9ca3af;
  }
</style>
