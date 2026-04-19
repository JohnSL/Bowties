<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { syncPanelStore } from '$lib/stores/syncPanel.svelte';
  import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { buildOfflineNodeTree } from '$lib/api/layout';
  import { markNodeConfigRead } from '$lib/stores/configReadStatus';
  import ConflictRow from './ConflictRow.svelte';
  import CleanSummarySection from './CleanSummarySection.svelte';
  import type { SyncMode } from '$lib/api/sync';

  /** Whether the modal should be visible (controlled by parent). */
  let { visible = $bindable(false) }: { visible?: boolean } = $props();

  // ── Sync mode prompt state ────────────────────────────────────────────────
  let showModePrompt = $derived(
    syncPanelStore.matchStatus !== null &&
    syncPanelStore.matchStatus.classification !== 'likely_same' &&
    syncPanelStore.syncMode === null
  );

  // ── Focus trapping ────────────────────────────────────────────────────────
  function handleKeydown(event: KeyboardEvent) {
    if (!visible) return;
    if (event.key === 'Tab') trapFocus(event);
    if (event.key === 'Escape' && !syncPanelStore.isApplying) {
      handleDismiss();
    }
  }

  function trapFocus(event: KeyboardEvent) {
    const modal = document.querySelector('.sp-content');
    if (!modal) return;
    const focusable = modal.querySelectorAll(
      'button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])'
    );
    if (focusable.length === 0) return;
    const first = focusable[0] as HTMLElement;
    const last = focusable[focusable.length - 1] as HTMLElement;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  // ── Handlers ──────────────────────────────────────────────────────────────
  async function handleModeChoice(mode: SyncMode) {
    await syncPanelStore.setMode(mode);
    if (mode === 'bench_other_bus') {
      // Bench mode — dismiss without syncing
      syncPanelStore.dismiss();
      visible = false;
      return;
    }
    // Target bus — proceed to build sync session
    await syncPanelStore.loadSession();
    if (!syncPanelStore.session ||
      (syncPanelStore.session.conflictRows.length === 0 &&
       syncPanelStore.session.cleanRows.length === 0 &&
       syncPanelStore.session.nodeMissingRows.length === 0)) {
      // Nothing to sync
      syncPanelStore.dismiss();
      visible = false;
    }
  }

  async function handleApply() {
    const result = await syncPanelStore.applySelected();
    if (result) {
      // Refresh offline changes list to reflect applied/cleared rows
      await offlineChangesStore.reloadFromBackend();

      // Rebuild node trees for affected nodes so baselines reflect applied values.
      // Collect unique nodeIds from rows whose changeId was applied or read-only-cleared.
      const clearedIds = new Set([...result.applied, ...result.readOnlyCleared]);
      const allRows = [
        ...(syncPanelStore.session?.conflictRows ?? []),
        ...(syncPanelStore.session?.cleanRows ?? []),
        ...(syncPanelStore.session?.nodeMissingRows ?? []),
      ];
      const affectedNodeIds = new Set(
        allRows
          .filter(r => r.nodeId && clearedIds.has(r.changeId))
          .map(r => r.nodeId!.replace(/\./g, '').toUpperCase())
      );
      for (const nodeId of affectedNodeIds) {
        try {
          const tree = await buildOfflineNodeTree(nodeId);
          nodeTreeStore.setTree(tree.nodeId, tree);
          markNodeConfigRead(tree.nodeId);
        } catch (e) {
          console.warn(`[sync] Failed to rebuild tree for ${nodeId}:`, e);
        }
      }

      if (result.failed.length === 0) {
        syncPanelStore.dismiss();
        visible = false;
      }
    }
  }

  function handleDismiss() {
    syncPanelStore.dismiss();
    visible = false;
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });
  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if visible}
  <div class="sp-overlay" role="presentation">
    <div
      class="sp-content"
      role="dialog"
      aria-modal="true"
      aria-label="Sync offline changes"
    >
      <!-- Header -->
      <div class="sp-header">
        <h2 class="sp-title">Sync Offline Changes</h2>
        {#if syncPanelStore.matchStatus}
          <span
            class="sp-match-badge"
            class:sp-match-good={syncPanelStore.matchStatus.classification === 'likely_same'}
            class:sp-match-uncertain={syncPanelStore.matchStatus.classification === 'uncertain'}
            class:sp-match-different={syncPanelStore.matchStatus.classification === 'likely_different'}
          >
            {#if syncPanelStore.matchStatus.classification === 'likely_same'}
              Layout match: {syncPanelStore.matchStatus.overlapPercent}%
            {:else if syncPanelStore.matchStatus.classification === 'uncertain'}
              Uncertain match: {syncPanelStore.matchStatus.overlapPercent}%
            {:else}
              Different bus: {syncPanelStore.matchStatus.overlapPercent}%
            {/if}
          </span>
        {/if}
      </div>

      <!-- Sync mode prompt (T044) -->
      {#if showModePrompt}
        <div class="sp-mode-prompt">
          <p class="sp-mode-text">
            The discovered nodes don't closely match this layout.
            How should Bowties treat this bus?
          </p>
          <div class="sp-mode-buttons">
            <button
              class="sp-mode-btn sp-mode-target"
              onclick={() => handleModeChoice('target_layout_bus')}
            >
              Target layout bus
              <span class="sp-mode-desc">Sync offline changes to this bus</span>
            </button>
            <button
              class="sp-mode-btn sp-mode-bench"
              onclick={() => handleModeChoice('bench_other_bus')}
            >
              Bench / other bus
              <span class="sp-mode-desc">Keep offline changes pending</span>
            </button>
          </div>
        </div>
      {:else}
        <!-- Loading state -->
        {#if syncPanelStore.isLoading}
          <div class="sp-loading">
            <div class="sp-spinner"></div>
            <p>Building sync session…</p>
          </div>

        <!-- Error state -->
        {:else if syncPanelStore.error}
          <div class="sp-error">
            <p class="sp-error-text">{syncPanelStore.error}</p>
            <button class="sp-btn sp-btn-secondary" onclick={handleDismiss}>
              Close
            </button>
          </div>

        <!-- Session content -->
        {:else if syncPanelStore.session}
          <div class="sp-body">
            <!-- Conflicts section -->
            {#if syncPanelStore.conflictRows.length > 0}
              <section class="sp-section sp-section-conflicts">
                <h3 class="sp-section-title sp-section-title-conflict">
                  Conflicts ({syncPanelStore.conflictRows.length})
                </h3>
                <p class="sp-section-hint">
                  The bus value has changed since capture. Resolve each before applying.
                </p>
                <div class="sp-conflict-list">
                  {#each syncPanelStore.conflictRows as row (row.changeId)}
                    <ConflictRow {row} />
                  {/each}
                </div>
              </section>
            {/if}

            <!-- Clean changes section -->
            {#if syncPanelStore.cleanRows.length > 0}
              <CleanSummarySection />
            {/if}

            <!-- Already-applied count -->
            {#if syncPanelStore.alreadyAppliedCount > 0}
              <section class="sp-section sp-section-applied">
                <p class="sp-applied-text">
                  {syncPanelStore.alreadyAppliedCount}
                  {syncPanelStore.alreadyAppliedCount === 1 ? 'change' : 'changes'}
                  already applied — cleared automatically.
                </p>
              </section>
            {/if}

            <!-- Node-missing rows -->
            {#if syncPanelStore.nodeMissingRows.length > 0}
              <section class="sp-section sp-section-missing">
                <h3 class="sp-section-title sp-section-title-missing">
                  Node not found ({syncPanelStore.nodeMissingRows.length})
                </h3>
                <div class="sp-missing-list">
                  {#each syncPanelStore.nodeMissingRows as row (row.changeId)}
                    <div class="sp-missing-row">
                      <span class="sp-missing-node">{row.nodeId ?? 'Unknown'}</span>
                      <span class="sp-missing-label">Not on bus</span>
                    </div>
                  {/each}
                </div>
              </section>
            {/if}

            <!-- Apply result feedback -->
            {#if syncPanelStore.applyResult}
              <div class="sp-result">
                {#if syncPanelStore.applyResult.applied.length > 0}
                  <p class="sp-result-ok">
                    ✓ {syncPanelStore.applyResult.applied.length} applied
                  </p>
                {/if}
                {#if syncPanelStore.applyResult.readOnlyCleared.length > 0}
                  <p class="sp-result-info">
                    {syncPanelStore.applyResult.readOnlyCleared.length} read-only — cleared
                  </p>
                {/if}
                {#if syncPanelStore.applyResult.failed.length > 0}
                  <p class="sp-result-fail">
                    ✗ {syncPanelStore.applyResult.failed.length} failed
                  </p>
                  <ul class="sp-fail-list">
                    {#each syncPanelStore.applyResult.failed as f}
                      <li>{f.changeId}: {f.reason}</li>
                    {/each}
                  </ul>
                {/if}
              </div>
            {/if}
          </div>

          <!-- Footer -->
          <div class="sp-footer">
            <button class="sp-btn sp-btn-secondary" onclick={handleDismiss} disabled={syncPanelStore.isApplying}>
              {syncPanelStore.applyResult ? 'Close' : 'Skip Sync'}
            </button>
            {#if !syncPanelStore.applyResult}
              <button
                class="sp-btn sp-btn-primary"
                onclick={handleApply}
                disabled={!syncPanelStore.canApply || syncPanelStore.isApplying}
              >
                {#if syncPanelStore.isApplying}
                  Applying…
                {:else}
                  Apply ({syncPanelStore.applyCount})
                {/if}
              </button>
            {/if}
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  /* ─── Overlay ─────────────────────────────────────────── */
  .sp-overlay {
    position: fixed;
    inset: 0;
    z-index: 950;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    animation: sp-fade-in 0.2s ease-out;
  }

  @keyframes sp-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .sp-content {
    background: #fff;
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
    width: 560px;
    max-width: 95vw;
    max-height: 85vh;
    padding: 24px 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: sp-slide-in 0.25s ease-out;
    overflow-y: auto;
  }

  @keyframes sp-slide-in {
    from { opacity: 0; transform: translateY(-12px) scale(0.97); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

  /* ─── Header ──────────────────────────────────────────── */
  .sp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .sp-title {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #1e293b;
  }

  .sp-match-badge {
    font-size: 12px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 9999px;
  }

  .sp-match-good {
    background: #dcfce7;
    color: #166534;
  }

  .sp-match-uncertain {
    background: #fef3c7;
    color: #92400e;
  }

  .sp-match-different {
    background: #fee2e2;
    color: #991b1b;
  }

  /* ─── Sync mode prompt ────────────────────────────────── */
  .sp-mode-prompt {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .sp-mode-text {
    margin: 0;
    font-size: 13px;
    color: #475569;
    line-height: 1.5;
  }

  .sp-mode-buttons {
    display: flex;
    gap: 12px;
  }

  .sp-mode-btn {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 12px 16px;
    border: 2px solid #e2e8f0;
    border-radius: 8px;
    background: #fff;
    cursor: pointer;
    font-size: 14px;
    font-weight: 600;
    color: #1e293b;
    transition: border-color 0.15s, background-color 0.15s;
  }

  .sp-mode-btn:hover {
    border-color: #94a3b8;
    background: #f8fafc;
  }

  .sp-mode-target:hover {
    border-color: #2563eb;
    background: #eff6ff;
  }

  .sp-mode-bench:hover {
    border-color: #d97706;
    background: #fffbeb;
  }

  .sp-mode-desc {
    font-size: 12px;
    font-weight: 400;
    color: #64748b;
  }

  /* ─── Loading ─────────────────────────────────────────── */
  .sp-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 24px 0;
  }

  .sp-loading p {
    margin: 0;
    font-size: 13px;
    color: #475569;
  }

  .sp-spinner {
    width: 28px;
    height: 28px;
    border: 3px solid #e2e8f0;
    border-top-color: #2563eb;
    border-radius: 50%;
    animation: sp-spin 0.8s linear infinite;
  }

  @keyframes sp-spin {
    to { transform: rotate(360deg); }
  }

  /* ─── Error ───────────────────────────────────────────── */
  .sp-error {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 0;
  }

  .sp-error-text {
    margin: 0;
    font-size: 13px;
    color: #dc2626;
  }

  /* ─── Body sections ───────────────────────────────────── */
  .sp-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .sp-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sp-section-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .sp-section-title-conflict {
    color: #dc2626;
  }

  .sp-section-title-missing {
    color: #6b7280;
  }

  .sp-section-hint {
    margin: 0;
    font-size: 12px;
    color: #64748b;
  }

  .sp-conflict-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  /* ─── Already applied ─────────────────────────────────── */
  .sp-section-applied {
    padding: 8px 12px;
    background: #f0fdf4;
    border-radius: 6px;
  }

  .sp-applied-text {
    margin: 0;
    font-size: 13px;
    color: #166534;
  }

  /* ─── Node missing ────────────────────────────────────── */
  .sp-missing-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sp-missing-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    background: #f9fafb;
    border-radius: 4px;
    font-size: 12px;
  }

  .sp-missing-node {
    color: #374151;
    font-family: monospace;
  }

  .sp-missing-label {
    color: #9ca3af;
    font-style: italic;
  }

  /* ─── Apply result ────────────────────────────────────── */
  .sp-result {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    background: #f8fafc;
    border-radius: 6px;
  }

  .sp-result-ok {
    margin: 0;
    font-size: 13px;
    color: #166534;
  }

  .sp-result-info {
    margin: 0;
    font-size: 13px;
    color: #64748b;
  }

  .sp-result-fail {
    margin: 0;
    font-size: 13px;
    color: #dc2626;
  }

  .sp-fail-list {
    margin: 4px 0 0;
    padding-left: 16px;
    font-size: 12px;
    color: #991b1b;
  }

  /* ─── Footer ──────────────────────────────────────────── */
  .sp-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
  }

  .sp-btn {
    padding: 6px 20px;
    font-size: 13px;
    border-radius: 6px;
    cursor: pointer;
    transition: background-color 0.15s, border-color 0.15s;
  }

  .sp-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .sp-btn-primary {
    background: #2563eb;
    color: #fff;
    border: 1px solid #2563eb;
  }

  .sp-btn-primary:hover:not(:disabled) {
    background: #1d4ed8;
  }

  .sp-btn-secondary {
    background: #fff;
    color: #334155;
    border: 1px solid #cbd5e1;
  }

  .sp-btn-secondary:hover:not(:disabled) {
    background: #f1f5f9;
    border-color: #94a3b8;
  }
</style>
