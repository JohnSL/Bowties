<script lang="ts">
  /**
   * CdiXmlViewer — Modal viewer for a node's raw CDI XML, with Prism syntax
   * highlighting and a Copy-to-clipboard affordance.
   *
   * dialog-shell-refactor (Slice 7): wraps the Fluent `Dialog` shell. The
   * `Copy` action appears in the footer only when XML is loaded successfully.
   * The shell's × close (and Esc / overlay) all map to `onClose`; the
   * explicit Close action in the footer provides the redundant primary path.
   */
  import { formatXml } from '$lib/utils/xmlFormatter';
  import type { ViewerStatus } from '$lib/types/cdi';
  import { CDI_SIZE_WARNING_THRESHOLD } from '$lib/types/cdi';
  import Prism from 'prismjs';
  import 'prismjs/components/prism-markup'; // XML is under 'markup'
  import 'prismjs/themes/prism.css'; // Default light theme
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    visible?: boolean;
    nodeId?: string | null;
    xmlContent?: string | null;
    status?: ViewerStatus;
    errorMessage?: string | null;
    onClose?: () => void;
  }

  let {
    visible = false,
    nodeId = null,
    xmlContent = null,
    status = 'idle' as ViewerStatus,
    errorMessage = null,
    onClose = () => {},
  }: Props = $props();

  let copySuccess = $state(false);

  // Reformat + re-highlight whenever xmlContent changes successfully.
  let formattedXml = $derived.by(() =>
    xmlContent && status === 'success' ? formatXml(xmlContent) : null,
  );
  let highlightedXml = $derived.by(() =>
    formattedXml ? Prism.highlight(formattedXml, Prism.languages.markup, 'markup') : null,
  );
  let showWarning = $derived(
    Boolean(xmlContent && status === 'success' && xmlContent.length > CDI_SIZE_WARNING_THRESHOLD),
  );

  const hasContent = $derived(status === 'success' && xmlContent !== null);

  async function copyToClipboard() {
    if (!xmlContent) return;
    try {
      await navigator.clipboard.writeText(xmlContent);
      copySuccess = true;
      setTimeout(() => { copySuccess = false; }, 2000);
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  }
</script>

{#snippet viewerActions()}
  <DialogActions>
    {#if hasContent}
      <Button appearance="secondary" onclick={copyToClipboard} ariaLabel="Copy XML to clipboard">
        {copySuccess ? '✓ Copied' : 'Copy'}
      </Button>
    {/if}
    <Button appearance="primary" onclick={onClose}>Close</Button>
  </DialogActions>
{/snippet}

<Dialog
  open={visible}
  width="lg"
  ariaLabel={`CDI XML for node ${nodeId ?? 'Unknown'}`}
  initialFocus="none"
  actions={viewerActions}
  onCancel={onClose}
>
  {#snippet title()}
    <DialogTitle>CDI XML — Node {nodeId || 'Unknown'}</DialogTitle>
  {/snippet}

  <div class="xv-body">
    {#if status === 'loading'}
      <div class="xv-loading">
        <div class="xv-spinner" aria-hidden="true"></div>
        <p>Loading CDI XML…</p>
      </div>
    {:else if status === 'error'}
      <div class="xv-error">
        <p class="xv-error-icon" aria-hidden="true">⚠</p>
        <p class="xv-error-message">{errorMessage || 'An error occurred while loading CDI XML.'}</p>
      </div>
    {:else if status === 'success' && formattedXml}
      {#if showWarning}
        <div class="xv-warning">
          <p>⚠ Large document may impact performance</p>
        </div>
      {/if}
      <pre class="xv-xml"><code class="language-markup">{@html highlightedXml}</code></pre>
    {:else}
      <p class="xv-empty">No CDI data available.</p>
    {/if}
  </div>
</Dialog>

<style>
  .xv-body {
    height: min(70vh, 600px);
    min-height: 200px;
    display: flex;
    flex-direction: column;
    overflow: auto;
  }

  .xv-loading,
  .xv-error,
  .xv-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem;
    flex: 1;
  }

  .xv-spinner {
    width: 40px;
    height: 40px;
    border: 4px solid var(--fluent-neutralBackground3);
    border-top-color: var(--fluent-brandBackground);
    border-radius: 50%;
    animation: xv-spin 0.8s linear infinite;
  }
  @keyframes xv-spin {
    to { transform: rotate(360deg); }
  }
  .xv-loading p {
    margin-top: 1rem;
    color: var(--fluent-neutralForeground3);
  }

  .xv-error-icon {
    font-size: 2.5rem;
    margin: 0 0 1rem 0;
    color: var(--fluent-warningGlyph);
  }
  .xv-error-message {
    color: var(--fluent-dangerBackground);
    text-align: center;
    max-width: 500px;
  }

  .xv-warning {
    background-color: #fff4ce;
    border: 1px solid #f7d96a;
    border-radius: 4px;
    padding: 0.6rem 1rem;
    margin-bottom: 0.75rem;
  }
  .xv-warning p {
    margin: 0;
    color: var(--fluent-warningGlyph);
    font-size: var(--fluent-fontSizeBase200);
  }

  .xv-xml {
    background-color: var(--fluent-neutralBackground3);
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    padding: 1rem;
    margin: 0;
    font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
    font-size: 0.875rem;
    line-height: 1.5;
    overflow: auto;
    white-space: pre;
    color: var(--fluent-neutralForeground1);
    flex: 1;
    min-height: 0;
  }

  .xv-xml code {
    font-family: inherit;
    background: none; /* Override Prism default */
    padding: 0;
    border-radius: 0;
  }

  /* Prism.js syntax highlighting adjustments — kept verbatim from prior version. */
  .xv-xml :global(.token.tag) {
    color: #2563eb;
  }
  .xv-xml :global(.token.attr-name) {
    color: #d97706;
  }
  .xv-xml :global(.token.attr-value) {
    color: #16a34a;
  }
  .xv-xml :global(.token.punctuation) {
    color: #6b7280;
  }
  .xv-xml :global(.token.comment) {
    color: #9ca3af;
    font-style: italic;
  }

  .xv-empty {
    color: var(--fluent-neutralForeground3);
    font-style: italic;
  }
</style>
