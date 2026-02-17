<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { formatXml } from '$lib/utils/xmlFormatter';
  import type { ViewerStatus } from '$lib/types/cdi';
  import { CDI_SIZE_WARNING_THRESHOLD } from '$lib/types/cdi';
  import Prism from 'prismjs';
  import 'prismjs/components/prism-markup'; // XML is under 'markup'
  import 'prismjs/themes/prism.css'; // Default light theme

  // Props
  export let visible: boolean = false;
  export let nodeId: string | null = null;
  export let xmlContent: string | null = null;
  export let status: ViewerStatus = 'idle';
  export let errorMessage: string | null = null;
  export let onClose: () => void = () => {};

  // Local state
  let formattedXml: string | null = null;
  let highlightedXml: string | null = null;
  let copySuccess: boolean = false;
  let showWarning: boolean = false;

  // Format and highlight XML when content changes
  $: if (xmlContent && status === 'success') {
    formattedXml = formatXml(xmlContent);
    // Apply Prism syntax highlighting
    highlightedXml = Prism.highlight(formattedXml, Prism.languages.markup, 'markup');
    showWarning = xmlContent.length > CDI_SIZE_WARNING_THRESHOLD;
  }

  // Handle Escape key
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && visible) {
      onClose();
    }
    
    // Focus trap: Handle Tab key to cycle focus within modal
    if (event.key === 'Tab' && visible) {
      trapFocus(event);
    }
  }

  // Trap focus within modal
  function trapFocus(event: KeyboardEvent) {
    const modal = document.querySelector('.modal-content');
    if (!modal) return;

    const focusableElements = modal.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    
    const firstElement = focusableElements[0] as HTMLElement;
    const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

    if (event.shiftKey && document.activeElement === firstElement) {
      // Shift+Tab on first element: focus last element
      event.preventDefault();
      lastElement?.focus();
    } else if (!event.shiftKey && document.activeElement === lastElement) {
      // Tab on last element: focus first element
      event.preventDefault();
      firstElement?.focus();
    }
  }

  // Copy to clipboard
  async function copyToClipboard() {
    if (!xmlContent) return;
    
    try {
      await navigator.clipboard.writeText(xmlContent);
      copySuccess = true;
      setTimeout(() => {
        copySuccess = false;
      }, 2000);
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  }

  // Handle click outside modal
  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      onClose();
    }
  }

  // Lifecycle
  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if visible}
  <div class="modal-overlay" onclick={handleOverlayClick} onkeydown={handleKeydown} role="presentation" tabindex="-1">
    <div class="modal-content" onclick={(e) => e.stopPropagation()} role="dialog" aria-labelledby="modal-title" aria-modal="true" tabindex="0">
      <!-- Header -->
      <header class="modal-header">
        <h2 id="modal-title">CDI XML - Node {nodeId || 'Unknown'}</h2>
        <div class="modal-actions">
          {#if status === 'success' && xmlContent}
            <button 
              class="btn-copy" 
              onclick={copyToClipboard}
              aria-label="Copy XML to clipboard"
            >
              {copySuccess ? '✓ Copied' : 'Copy'}
            </button>
          {/if}
          <button 
            class="btn-close" 
            onclick={onClose}
            aria-label="Close modal"
          >
            Close
          </button>
        </div>
      </header>

      <!-- Content -->
      <div class="modal-body">
        {#if status === 'loading'}
          <div class="loading">
            <div class="spinner"></div>
            <p>Loading CDI XML...</p>
          </div>
        {:else if status === 'error'}
          <div class="error">
            <p class="error-icon">⚠️</p>
            <p class="error-message">{errorMessage || 'An error occurred while loading CDI XML.'}</p>
          </div>
        {:else if status === 'success' && formattedXml}
          {#if showWarning}
            <div class="warning">
              <p>⚠️ Large document may impact performance</p>
            </div>
          {/if}
          <pre class="xml-content"><code class="language-markup">{@html highlightedXml}</code></pre>
        {:else}
          <p class="no-content">No CDI data available.</p>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    animation: fadeIn 0.2s ease-in;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .modal-content {
    background: white;
    border-radius: 8px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1), 0 10px 20px rgba(0, 0, 0, 0.2);
    width: 90%;
    max-width: 900px;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    animation: slideIn 0.2s ease-out;
  }

  @keyframes slideIn {
    from {
      transform: translateY(-20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem;
    border-bottom: 1px solid #e5e7eb;
  }

  .modal-header h2 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
    color: #111827;
  }

  .modal-actions {
    display: flex;
    gap: 0.5rem;
  }

  button {
    padding: 0.5rem 1rem;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    border: 1px solid transparent;
  }

  .btn-copy {
    background-color: #3b82f6;
    color: white;
    border-color: #3b82f6;
  }

  .btn-copy:hover {
    background-color: #2563eb;
    border-color: #2563eb;
  }

  .btn-close {
    background-color: #f3f4f6;
    color: #374151;
    border-color: #d1d5db;
  }

  .btn-close:hover {
    background-color: #e5e7eb;
    border-color: #9ca3af;
  }

  .modal-body {
    padding: 1.5rem;
    overflow-y: auto;
    flex: 1;
    min-height: 200px;
  }

  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem;
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 4px solid #e5e7eb;
    border-top-color: #3b82f6;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .loading p {
    margin-top: 1rem;
    color: #6b7280;
  }

  .error {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 3rem;
  }

  .error-icon {
    font-size: 3rem;
    margin-bottom: 1rem;
  }

  .error-message {
    color: #dc2626;
    font-size: 1rem;
    text-align: center;
    max-width: 500px;
  }

  .warning {
    background-color: #fef3c7;
    border: 1px solid #fbbf24;
    border-radius: 6px;
    padding: 0.75rem 1rem;
    margin-bottom: 1rem;
  }

  .warning p {
    margin: 0;
    color: #92400e;
    font-size: 0.875rem;
  }

  .xml-content {
    background-color: #f9fafb;
    border: 1px solid #e5e7eb;
    border-radius: 6px;
    padding: 1rem;
    margin: 0;
    font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
    font-size: 0.875rem;
    line-height: 1.5;
    overflow-x: auto;
    white-space: pre;
    color: #111827;
  }

  .xml-content code {
    font-family: inherit;
    background: none; /* Override Prism default */
    padding: 0; /* Override Prism default */
    border-radius: 0; /* Override Prism default */
  }

  /* Prism.js syntax highlighting adjustments */
  .xml-content :global(.token.tag) {
    color: #2563eb; /* Blue for tags */
  }

  .xml-content :global(.token.attr-name) {
    color: #059669; /* Green for attributes */
  }

  .xml-content :global(.token.attr-value),
  .xml-content :global(.token.string) {
    color: #dc2626; /* Red for attribute values */
  }

  .xml-content :global(.token.punctuation) {
    color: #6b7280; /* Gray for brackets/quotes */
  }

  .xml-content :global(.token.comment) {
    color: #9ca3af; /* Light gray for comments */
    font-style: italic;
  }

  .no-content {
    text-align: center;
    color: #6b7280;
    padding: 3rem;
  }

  /* Focus trap styles */
  .modal-content:focus {
    outline: none;
  }

  /* Scrollbar styling */
  .modal-body::-webkit-scrollbar {
    width: 8px;
  }

  .modal-body::-webkit-scrollbar-track {
    background: #f3f4f6;
  }

  .modal-body::-webkit-scrollbar-thumb {
    background: #d1d5db;
    border-radius: 4px;
  }

  .modal-body::-webkit-scrollbar-thumb:hover {
    background: #9ca3af;
  }
</style>
