<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount, untrack } from 'svelte';
  import { get } from 'svelte/store';
  import { WebviewWindow, getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import ConfigSidebar from '$lib/components/ConfigSidebar/ConfigSidebar.svelte';
  import SidebarResizeHandle from '$lib/components/ConfigSidebar/SidebarResizeHandle.svelte';
  import SegmentView from '$lib/components/ElementCardDeck/SegmentView.svelte';
  import CdiXmlViewer from '$lib/components/CdiXmlViewer.svelte';
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { sidebarWidthStore } from '$lib/stores/sidebarWidth';
  import { probeNodes as probeNodesApi, querySnip, queryPip, registerNode, refreshAllNodes } from '$lib/api/tauri';
  import { buildBowtieCatalog, clearRecentLayout, getRecentLayout } from '$lib/api/bowties';
  import { closeLayout, saveLayoutDirectory, saveLayoutWithBusWrites, openLayoutDirectory, buildOfflineNodeTree, createNewLayoutCapture, getNodeTree } from '$lib/api/layout';
  import type { OfflineNodeSnapshot } from '$lib/api/layout';
  import { getKnownLayouts, addKnownLayout, removeKnownLayout } from '$lib/api/startup';
  import { knownLayoutsStore } from '$lib/stores/knownLayouts.svelte';
  import {
    loadKnownLayouts,
    openLayoutFromRegistry,
    createNewLayout,
    removeKnownLayout as removeKnownLayoutOrchestrated,
  } from '$lib/orchestration/startupOrchestrator';
  import LayoutPicker from '$lib/components/LayoutPicker/LayoutPicker.svelte';
  import { toast } from '@zerodevx/svelte-toast';
  import { readAllConfigValues, cancelConfigReading, getCdiXml, downloadCdi } from '$lib/api/cdi';
  import type { ReadProgressState } from '$lib/api/types';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { resolvePillSelectionsForPath } from '$lib/types/nodeTree';
  import type { NodeConfigTree } from '$lib/types/nodeTree';
  import { configReadNodesStore, markNodeConfigRead, clearConfigReadStatus, removeNodesConfigRead } from '$lib/stores/configReadStatus';
  import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
  import { cdiCacheStore } from '$lib/stores/cdiCache.svelte';
  import BowtieCatalogPanel from '$lib/components/Bowtie/BowtieCatalogPanel.svelte';
  import DiscoveryProgressModal from '$lib/components/DiscoveryProgressModal.svelte';
  import SaveControls from '$lib/components/ElementCardDeck/SaveControls.svelte';
  import CdiDownloadDialog from '$lib/components/CdiDownloadDialog.svelte';
  import CdiRedownloadDialog from '$lib/components/CdiRedownloadDialog.svelte';
  import ErrorDialog from '$lib/components/ErrorDialog.svelte';
  import SaveProgressDialog from '$lib/components/SaveProgressDialog.svelte';
  import AddBoardDialog from '$lib/components/AddBoardDialog.svelte';
  import { saveProgressStore } from '$lib/stores/saveProgress.svelte';
  import MissingCaptureBadge from '$lib/components/Layout/MissingCaptureBadge.svelte';
  import ConnectionManager from '$lib/ConnectionManager.svelte';
  import { connectionRequestStore } from '$lib/stores/connectionRequest.svelte';
   import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
  import { bowtieFocusStore } from '$lib/stores/bowtieFocus.svelte';
  import { configFocusStore } from '$lib/stores/configFocus.svelte';
  import { setPillSelection } from '$lib/stores/pillSelection';
  import { layoutOpenInProgress, layoutOpenStatusText, failLayoutOpen, resetLayoutOpenPhase } from '$lib/stores/layoutOpenLifecycle';
  import {
    openOfflineLayoutWithReplay,
    rehydrateOfflineStateFromSnapshots,
    restoreRecentOfflineLayout,
  } from '$lib/orchestration/offlineLayoutOrchestrator';
  import { layoutLifecycleOrchestrator } from '$lib/orchestration/layoutLifecycleOrchestrator';
  import {
    getUnreadConfigEligibleNodes,
    pipConfirmsNoCdi,
    toConfigReadCandidate,
  } from '$lib/orchestration/configReadOrchestrator';
  import {
    applyConnectorSelectionChange,
    recomputeConnectorCompatibility,
  } from '$lib/orchestration/connectorSelectionOrchestrator';
  import { ConfigAcquisitionOrchestrator } from '$lib/orchestration/configAcquisitionOrchestrator.svelte';
  import { CdiInspectionOrchestrator } from '$lib/orchestration/cdiInspectionOrchestrator.svelte';
  import { handleDiscoveredNode, reconcileRefreshState, refreshReinitializedNode } from '$lib/orchestration/discoveryOrchestrator';
  import { hasUnsavedPromptChanges } from '$lib/orchestration/unsavedChangesGuard';
  import { configChangesStore } from '$lib/stores/configChanges.svelte';
  import {
    bootstrapStartupLifecycle,
    SyncSessionOrchestrator,
  } from '$lib/orchestration/syncSessionOrchestrator.svelte';
  import { installMenuShortcuts } from '../lib/keyboard/menuShortcuts';
  import { registerMenuListeners } from '$lib/orchestration/menuListeners';
  import { computeMenuEnableState } from '$lib/utils/menuEnableState';
  import SyncPanel from '$lib/components/Sync/SyncPanel.svelte';
  import { syncPanelStore } from '$lib/stores/syncPanel.svelte';
  import OfflineBanner from '$lib/components/Layout/OfflineBanner.svelte';
  import { saveLayoutOrchestrated, type SaveLayoutOrchestratedArgs } from '$lib/orchestration/saveLayoutOrchestrator';
  import { stageDraftsForOfflineSave } from '$lib/orchestration/configDraftOrchestrator';
  import { normalizeLayoutTitle } from '$lib/utils/layoutPath';
  import { formatNodeId, nodeIdStringToBytes } from '$lib/utils/nodeId';
  import { canonicalizeNodeId } from '$lib/utils/nodeRoster';
  import { effectiveNodeStore } from '$lib/layout';
  import { partialCaptureNodesStore } from '$lib/stores/partialCaptureNodes.svelte';
  import { deletePlaceholderBoard } from '$lib/orchestration/placeholderBoardOrchestrator';
  import { isPlaceholderInput } from '$lib/utils/nodeKey';

  // Active tab state — 'config' (default) or 'bowties'
  let activeTab = $state<'config' | 'bowties'>('config');

  // Ref to SaveControls for imperative save calls from menu shortcuts
  let saveControlsRef = $state<SaveControls | null>(null);

  // Keyboard handler for the segmented mode control (ArrowLeft / ArrowRight)
  function handleModeKeydown(e: KeyboardEvent): void {
    if (e.key === 'ArrowLeft')  { activeTab = 'config';  e.preventDefault(); }
    else if (e.key === 'ArrowRight') { activeTab = 'bowties'; e.preventDefault(); }
  }

  // T050: prompt-to-save guard state
  let unsavedDialog = $state<{ message: string; proceed: () => void; confirmLabel: string } | null>(null);
  let isForceClosing = false;
  let errorDialog = $state<{ title: string; message: string } | null>(null);

  // Spec 014 / S8: placeholder picker modal visibility. Opened by the
  // "Add Placeholder Board…" menu item; closed on cancel or after a
  // successful add.
  let showAddBoardDialog = $state(false);

  // Spec 014 / S8.5 / T11: pending "Delete Placeholder Board" confirmation.
  // Holds the NodeKey to delete while the user confirms; cleared on
  // confirm/cancel.
  let pendingDeletePlaceholderKey = $state<string | null>(null);

  function promptUnsaved(message: string, proceed: () => void, confirmLabel = 'Discard & Continue'): void {
    const hasUnsaved = hasUnsavedPromptChanges(
      nodeTreeStore.trees.keys(),
      bowtieMetadataStore.isDirty,
      offlineChangesStore.draftCount,
      effectiveNodeStore.isDirty,
      offlineChangesStore.revertedPersistedCount,
    );
    if (hasUnsaved) {
      unsavedDialog = { message, proceed, confirmLabel };
    } else {
      proceed();
    }
  }

  function isMenuBusy(): boolean {
    // S3: while a save is in progress the dialog is modal — no second save
    // can be initiated and other menu actions are gated as well.
    return probing || configAcquisition.readingRemaining || saveProgressStore.isActive;
  }

  function canOpenLayoutAction(): boolean {
    return !isMenuBusy();
  }

  function canCloseLayoutAction(): boolean {
    return !!layoutStore.activeContext;
  }

  function canSaveLayoutAction(): boolean {
    const busy = isMenuBusy();
    const layoutLoaded = layoutStore.isLoaded;
    const layoutDirty = layoutStore.isDirty;
    const metaDirty = bowtieMetadataStore.isDirty;
    const offlineActive = !!layoutStore.activeContext && layoutStore.hasLayoutFile;
    // ADR-0011: `effectiveNodeStore.isDirty` is the aggregate "any in-memory
    // change" signal (LayoutFile struct + drafts + metadata + offline +
    // unsaved-new). `layoutStore.isDirty` now means struct edits only.
    const hasInMemoryEdits = effectiveNodeStore.isDirty;
    return !busy && ((offlineActive && hasInMemoryEdits) || (layoutLoaded && (layoutDirty || metaDirty)));
  }

  function runOpenLayoutAction(): void {
    // Clear the active layout to surface the layout picker, where the user
    // chooses to open a known layout, browse for one, or create a new one.
    // (Disabled in the menu when no layout is active — the picker is already
    // visible in that case.)
    promptUnsaved('Opening a new layout will discard unsaved changes. Continue?', () => {
      void clearActiveLayout();
    }, 'Discard & Open');
  }

  function runCloseLayoutAction(): void {
    promptUnsaved('Closing the layout will discard unsaved changes. Continue?', () => {
      void clearActiveLayout();
    }, 'Discard & Close');
  }

  function runSaveLayoutAction(): void {
    saveControlsRef?.triggerSave();
  }

  function runSaveLayoutAsAction(): void {
    void saveControlsRef?.triggerSaveAs();
  }

  function canSaveLayoutAsAction(): boolean {
    return !isMenuBusy();
  }

  // T041: Switch to bowties tab when a config-first connection request is pending
  $effect(() => {
    if (connectionRequestStore.pendingRequest) {
      activeTab = 'bowties';
    }
  });

  // Switch to bowties tab when a "Used in" link is clicked on the config page
  $effect(() => {
    if (bowtieFocusStore.highlightedEventIdHex) {
      activeTab = 'bowties';
    }
  });

  // Switch to config tab and navigate to the target field when a bowtie entry link is clicked
  $effect(() => {
    const focus = configFocusStore.navigationRequest;
    if (!focus) return;

    // Consume immediately — TreeLeafRow handles its own leafFocusRequest.
    untrack(() => configFocusStore.clearNavigation());

    activeTab = 'config';

    const tree = untrack(() => nodeTreeStore.getTree(focus.nodeId));
    if (!tree) return;

    const segMatch = focus.elementPath[0]?.match(/^seg:(\d+)$/);
    if (!segMatch) return;
    const segIdx = parseInt(segMatch[1], 10);
    const seg = tree.segments[segIdx];
    if (!seg) return;

    // Compute and apply pill selections (pure utility — no tree-structure
    // knowledge required here).
    const pillEntries = resolvePillSelectionsForPath(focus.nodeId, seg, focus.elementPath);
    for (const [key, idx] of pillEntries) {
      setPillSelection(key, idx);
    }

    // Expand node in sidebar if needed
    const sidebarState = get(configSidebarStore);
    if (!sidebarState.expandedNodeIds.includes(focus.nodeId)) {
      configSidebarStore.toggleNodeExpanded(focus.nodeId);
    }

    // Select the segment → triggers card deck render → TreeLeafRow mounts →
    // leafFocusRequest scrolls + focuses the input.
    configSidebarStore.selectSegment(focus.nodeId, `seg:${segIdx}`, seg.name);
  });

  // Connection state
  // `connected` is owned by `layoutStore` (read via `layoutStore.isConnected`)
  // and the connection label is owned by `syncSessionOrchestrator` (S3).
  // `errorMessage` is the page-wide error banner, written by several workflows.
  let errorMessage = $state("");

  // Discovery state
  // Spec 014 / S8.7: the page-local `nodes` array was the bug-2 misfire — it
  // mirrored only live discoveries, missing every placeholder, so the
  // "No nodes found." gate fired for placeholder-only layouts. Now derived
  // from `nodeRoster.allEntries` (live + placeholder) so any consumer
  // reading `nodes` sees the unified roster. `liveNodes` is exposed for
  // call sites that still need the strict live-only subset.
  const nodes = $derived(nodeRoster.allEntries.map((e) => e.info));
  const liveNodes = $derived(nodeRoster.liveNodes);
  let probing = $state(false);
  let showConnectionDialog = $state(false);
  let startupBootstrapPending = $state(true);
  let syncPanelVisible = $state(false);

  // Snapshots from the currently active layout file — used to re-hydrate the
  // offline tree after disconnect so nodes are not lost from the UI (Bug 4).
  let currentLayoutSnapshots = $state<OfflineNodeSnapshot[]>([]);

  // ADR-0011: `effectiveNodeStore` is the single source of truth for the
  // per-node persistability projection and the aggregate "any in-memory
  // change" signal. Callers read `effectiveNodeStore.isDirty` /
  // `.unsavedInMemoryNodeIds` directly; nothing mirrors back into
  // `layoutStore`. `layoutStore.isDirty` now means LayoutFile-struct edits
  // only — its proper domain.

  // Sync-session lifecycle state is coordinated in a dedicated orchestrator.
  // S3: it also owns the connect/disconnect workflow and the connection label;
  // `connected` lives in `layoutStore` (authoritative) and `errorMessage` stays
  // page-owned (written by several workflows), reported via `setErrorMessage`.
  const syncSessionOrchestrator = new SyncSessionOrchestrator({
    disconnectLcc: () => invoke('disconnect_lcc'),
    probeForNodes,
    hasLayoutFile: () => layoutStore.hasLayoutFile,
    hasSnapshots: () => currentLayoutSnapshots.length > 0,
    setLayoutConnected: (value) => { layoutStore.setConnected(value); },
    resetFreshLiveSessionState: () => { resetFreshLiveSessionState(); },
    rehydrateOffline: async () => {
      nodeTreeStore.reset();
      await hydrateOfflineSnapshots(currentLayoutSnapshots);
      const availableKeys = new Set(nodeRoster.allEntries.map((e) => e.nodeKey));
      configSidebarStore.pruneToAvailableNodes(availableKeys);
    },
    clearLiveState: () => {
      configSidebarStore.reset();
      clearConfigReadStatus();
      nodeRoster.replaceLiveRoster([]);
      nodeTreeStore.reset();
    },
    resetSyncPanel: () => {
      syncPanelStore.reset();
      syncPanelVisible = false;
    },
    setShowConnectionDialog: (visible) => { showConnectionDialog = visible; },
    setErrorMessage: (message) => { errorMessage = message; },
    warn: (message, error) => { console.warn(message, error); },
  });


  let activeLayoutLabel = $derived.by(() => {
    const ctx = layoutStore.activeContext;
    if (!ctx) return null;
    return normalizeLayoutTitle(ctx.layoutId) ?? normalizeLayoutTitle(ctx.rootPath);
  });

  async function hydrateOfflineSnapshots(snapshots: OfflineNodeSnapshot[]) {
    await rehydrateOfflineStateFromSnapshots({
      snapshots,
      nodeIdStringToBytes,
      buildOfflineNodeTree,
      publishNodes: (offlineNodes) => {
        nodeRoster.replaceLiveRoster(offlineNodes);
      },
      clearConfigReadStatus,
      resetNodeTrees: () => {
        nodeTreeStore.reset();
      },
      setTree: (nodeId, tree) => {
        nodeTreeStore.setTree(nodeId, tree);
      },
      markNodeConfigRead,
      onTreeBuildWarning: (message) => {
        console.warn(message);
      },
    });

    // S9: Restore placeholder nodes that were saved in the layout.
    // The backend reconstituted them into the registry during
    // open_layout_directory; we just need to publish them to the sidebar.
    const placeholderSnapshots = snapshots.filter((s) => isPlaceholderInput(s.nodeKey));
    const restoredKeys: string[] = [];
    for (const snap of placeholderSnapshots) {
      try {
        const tree = await getNodeTree(snap.nodeKey);
        const nowIso = new Date().toISOString();
        nodeRoster.addPlaceholder({
          nodeKey: snap.nodeKey,
          profileStem: snap.profileStem!,
          info: {
            node_id: [],
            alias: 0,
            snip_data: {
              manufacturer: snap.snip.manufacturerName,
              model: snap.snip.modelName,
              hardware_version: '',
              software_version: '',
              user_name: snap.snip.userName,
              user_description: snap.snip.userDescription,
            },
            snip_status: 'Complete',
            connection_status: 'Unknown',
            last_verified: nowIso,
            last_seen: nowIso,
            cdi: null,
            pip_flags: null,
            pip_status: 'NotSupported',
          },
          tree,
        });
        restoredKeys.push(snap.nodeKey);
      } catch (e) {
        console.warn(`[offline] Failed to restore placeholder ${snap.nodeKey}:`, e);
      }
    }
    if (restoredKeys.length > 0) {
      nodeRoster.markPlaceholdersPersisted(restoredKeys);
    }
  }

  /**
   * Open a layout from a known path through the same replay flow used by
   * "Open Recent". After a successful open the layout is upserted into the
   * known-layouts registry so its `lastOpened` timestamp is refreshed.
   *
   * Used by both the menu-driven "Open…" command and the startup picker.
   *
   * If a live bus connection is active, disconnect first — the connections
   * defined on the outgoing layout are not necessarily valid for the incoming
   * one (Spec 013 / S7: connections are per-layout).
   */
  async function runOpenLayoutByPath(path: string, name?: string): Promise<void> {
    errorDialog = null;
    try {
      if (layoutStore.isConnected) {
        await disconnectBeforeLayoutSwitch();
      }
      await openLayoutFromRegistry({
        path,
        name,
        openLayout: (p) => openOfflineLayoutWithReplay({
          path: p,
          openLayout: openLayoutDirectory,
          hydrateOfflineSnapshots,
          resetSidebar: () => {
            configSidebarStore.reset();
          },
          hydrateConnectorSelections: (layout) => {
            connectorSelectionsStore.hydrateFromLayout(layout);
          },
          onOpened: () => {
            showConnectionDialog = false;
          },
        }),
        api: { addKnownLayout },
        store: knownLayoutsStore,
        onOpened: (result) => {
          partialCaptureNodesStore.replace(result.partialNodes);
          currentLayoutSnapshots = result.nodeSnapshots;
          if (result.recoveryOccurred) {
            toast.push('Previous save was interrupted and has been restored.', {
              theme: { '--toastBackground': '#fff4ce', '--toastColor': '#4f3a04', '--toastBarBackground': '#835b00' },
            });
          }
        },
      });
    } catch (error) {
      failLayoutOpen();
      errorDialog = {
        title: 'Failed to Load Layout',
        message: String(error ?? 'Unknown error'),
      };
    }
  }

  // ── Layout picker handlers (Spec 013 / S6) ──────────────────────────────

  function handlePickerOpen(entry: { name: string; path: string }): void {
    void runOpenLayoutByPath(entry.path, entry.name);
  }

  function handlePickerBrowse(path: string): void {
    void runOpenLayoutByPath(path);
  }

  async function handlePickerCreate(args: { name: string; path: string }): Promise<void> {
    errorDialog = null;
    try {
      await createNewLayout({
        name: args.name,
        path: args.path,
        api: { addKnownLayout },
        lifecycle: {
          closeLayout: () => clearActiveLayout(),
          createNewLayoutCapture,
          saveLayoutDirectory: (p, overwrite, _deltas) =>
            saveLayoutDirectory(p, overwrite, []),
          openLayout: (p) => openOfflineLayoutWithReplay({
            path: p,
            openLayout: openLayoutDirectory,
            hydrateOfflineSnapshots,
            resetSidebar: () => {
              configSidebarStore.reset();
            },
            hydrateConnectorSelections: (layout) => {
              connectorSelectionsStore.hydrateFromLayout(layout);
            },
            onOpened: () => {
              showConnectionDialog = false;
            },
          }),
        },
        store: knownLayoutsStore,
        onOpened: (result) => {
          partialCaptureNodesStore.replace(result.partialNodes);
          currentLayoutSnapshots = result.nodeSnapshots;
        },
      });
    } catch (error) {
      failLayoutOpen();
      errorDialog = {
        title: 'Failed to Create Layout',
        message: String(error ?? 'Unknown error'),
      };
    }
  }

  async function handlePickerRemove(entry: { path: string }): Promise<void> {
    await removeKnownLayoutOrchestrated({
      path: entry.path,
      api: { removeKnownLayout },
      store: knownLayoutsStore,
      onError: (err) => {
        errorDialog = {
          title: 'Failed to Remove Layout',
          message: String(err ?? 'Unknown error'),
        };
      },
    });
  }

  async function saveCurrentCaptureToFile(forceSaveAs = false): Promise<boolean> {
    try {
      let targetPath = layoutStore.activeContext?.mode === 'offline_file'
        ? layoutStore.activeContext.rootPath
        : '';
      if (forceSaveAs || !targetPath) {
        const selected = await open({
          title: 'Choose Layout Folder',
          directory: true,
          multiple: false,
        });
        if (!selected || typeof selected !== 'string') return false;
        targetPath = selected;
      }

      const deltas = bowtieMetadataStore.collectDeltas();

      // Offline mode owns draft staging: promote config drafts into the
      // offline-change channel before the orchestrator runs. The component
      // (SaveControls) no longer reaches into draft state — it just calls
      // `onSave()` and trusts the page/orchestrator to do the right thing.
      if (layoutStore.isOfflineMode) {
        stageDraftsForOfflineSave();
      }

      const sharedOrchestratorArgs = {
        clearMetadata: () => bowtieMetadataStore.clearAll(),
        markClean: () => layoutStore.markClean(),
        hydrateLayout: (layout: import('$lib/types/bowtie').LayoutFile) =>
          layoutStore.hydrateFromBackend(layout),
        flushPending: layoutStore.isOfflineMode
          ? async () => { await offlineChangesStore.flushPendingToBackend(); }
          : undefined,
        setActiveContext: (ctx: import('$lib/stores/layout.svelte').ActiveLayoutContext) =>
          layoutStore.setActiveContext(ctx),
        updatePartialCaptureNodes: (warnings: string[]) => {
          partialCaptureNodesStore.replace(warnings);
        },
        getPendingChangeCount: () => offlineChangesStore.pendingCount,
        // ADR-0004 (S2c): drop config drafts that are now persisted on disk so
        // the effective read model never observes a stale draft after the
        // catalog has been rebuilt from the saved layout.
        clearPersistedDrafts: () => configChangesStore.clearAllDrafts(),
        path: targetPath,
        deltas,
        // S8.11: unified set of in-memory node keys (real + placeholder)
        // that are not yet in the saved layout roster.
        inMemorySnapshotKeys: effectiveNodeStore.unsavedInMemoryNodeIds,
        // Bug 3: placeholders the user removed from a persisted layout —
        // emitted as `removeNode` deltas so the backend drops them and
        // prunes their snapshot files.
        inMemoryRemovedKeys: effectiveNodeStore.unsavedRemovedNodeIds,
        clearPersistedPlaceholders: (nodeKeys: string[]) => {
          nodeRoster.markPlaceholdersPersisted(nodeKeys);
        },
        clearPersistedRemovals: () => nodeRoster.clearPersistedRemovals(),
        // Offline mode only: the backend rewrites pending offline changes
        // during the save, so the frontend mirror must be re-pulled before
        // the UI settles. Online mode omits this callback.
        reloadOfflineChanges: layoutStore.isOfflineMode
          ? async () => { await offlineChangesStore.reloadFromBackend(); }
          : undefined,
      };
      const orchestratorArgs: SaveLayoutOrchestratedArgs = layoutStore.isConnected
        ? {
            ...sharedOrchestratorArgs,
            saveWithBusWrites: (p, d) => saveLayoutWithBusWrites(p, d, true),
          }
        : {
            ...sharedOrchestratorArgs,
            saveFile: (p, d) => saveLayoutDirectory(p, true, d),
            rebuildCatalog: buildBowtieCatalog,
            setCatalog: (catalog) => bowtieCatalogStore.setCatalog(catalog),
          };
      // S3: drive the save-progress modal. For the online path the backend
      // emits `save-progress` events that overwrite the phase as they arrive;
      // for the offline path this is the sole phase driver.
      saveProgressStore.begin();
      try {
        const saveResult = await saveLayoutOrchestrated(orchestratorArgs);
        // Bug 2b fix: cache the snapshots from this save so the disconnect
        // transition matrix sees `hasSnapshots: true` and takes the
        // `rehydrated_offline` path instead of clearing everything.
        currentLayoutSnapshots = saveResult.nodeSnapshots;
        saveProgressStore.apply({ phase: 'complete', failedCount: 0 });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        saveProgressStore.fail(`Save failed: ${msg}`);
        throw err;
      }

      return true;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // The SaveProgressDialog (driven by `saveProgressStore.fail(msg)` above)
      // owns the user-visible failure surface; no separate banner needed.
      console.error('[Page] saveCurrentCaptureToFile failed:', msg);
      throw e;
    }
  }

  async function resetLayoutStateForNoLayout(reprobeLiveNodes = true) {
    await layoutLifecycleOrchestrator.resetForNewLayout({
      connected: layoutStore.isConnected,
      reprobeLiveNodes,
      probeForNodes,
      afterReset: () => {
        currentLayoutSnapshots = [];
        syncSessionOrchestrator.resetAutoTrigger();
      },
    });
  }

  function resetFreshLiveSessionState(): void {
    layoutLifecycleOrchestrator.resetForFreshLiveSession();
    cdiCacheStore.reset();
  }

  async function clearActiveLayout() {
    await layoutLifecycleOrchestrator.closeLayout({
      activeMode: layoutStore.activeContext?.mode,
      closeLayoutIpc: (decision) => closeLayout(decision),
      clearRecentLayout,
      connected: layoutStore.isConnected,
      disconnectBeforeClose: () => syncSessionOrchestrator.disconnectBeforeLayoutSwitch(),
      probeForNodes,
      afterReset: () => {
        currentLayoutSnapshots = [];
        syncSessionOrchestrator.resetAutoTrigger();
      },
      onRecentLayoutClearError: (error) => {
        console.warn('[layout] Failed to clear persisted startup layout:', error);
      },
    });
  }

  // Reactive count of nodes with SNIP data not yet config-read — drives "Read Remaining" visibility
  let unreadCount = $derived(
    getUnreadConfigEligibleNodes(nodes, $configReadNodesStore).length
  );

  // Show CTA panel only on a fresh session where NO nodes have been read yet.
  // Once the user has read at least one node we stop showing it — new arrivals
  // mid-session are signalled by the toolbar badge instead, so we don't
  // disrupt whatever the user is already doing.
  // Use configReadNodesStore.size === 0 rather than unreadCount === nodes.length
  // because unreadCount excludes CDI-less nodes (e.g. JMRI), so the counts
  // would never match on a mixed network.
  let showConfigCta = $derived(
    layoutStore.isConnected &&
    !layoutStore.hasLayoutFile &&
    !$layoutOpenInProgress &&
    nodes.length > 0 &&
    unreadCount > 0 &&
    $configReadNodesStore.size === 0 &&
    !$configSidebarStore.selectedSegment &&
    !$configSidebarStore.selectedNodeId
  );

  // S8-T16: empty-state "Read all" affordance when a layout IS loaded.
  // Mirrors `showConfigCta` but for the post-layout-load case: surfaces the
  // message + button whenever (a) the user has no node/segment selected and
  // (b) at least one discovered node is not yet fully captured (i.e. not in
  // `fullyCapturedNodeIds`). Clicking it invokes the same `readRemainingNodes`
  // flow used by the toolbar "Read Remaining" button, promoting every
  // not-yet-captured node across the capture threshold. The pane returns to
  // its normal empty-state once everything is captured.
  let showCaptureRemainingCta = $derived(
    layoutStore.isConnected &&
    layoutStore.hasLayoutFile &&
    !$layoutOpenInProgress &&
    nodes.length > 0 &&
    unreadCount > 0 &&
    !$configSidebarStore.selectedSegment &&
    !$configSidebarStore.selectedNodeId
  );

  // The node whose segment is currently selected in the sidebar, if it has not
  // been read yet AND supports CDI. Used to show a per-node "read this node" CTA
  // instead of an empty SegmentView. CDI-less nodes (e.g. JMRI LccPro) are excluded
  // so we don't show a "Read Configuration" button for nodes that can't provide one.
  let selectedUnreadNodeId = $derived((() => {
    const sel = $configSidebarStore.selectedSegment?.nodeId ?? $configSidebarStore.selectedNodeId;
    if (!sel) return null;
    if ($configReadNodesStore.has(canonicalizeNodeId(sel))) return null;
    const selCanonical = canonicalizeNodeId(sel);
    const selectedNode = nodes.find(n => canonicalizeNodeId(formatNodeId(n.node_id)) === selCanonical);
    if (selectedNode && pipConfirmsNoCdi(selectedNode)) return null;
    return sel;
  })());

  let selectedUnreadNodeName = $derived(
    selectedUnreadNodeId
      ? (() => {
          const selCanonical = canonicalizeNodeId(selectedUnreadNodeId);
          const selectedNode = nodes.find((n) => canonicalizeNodeId(formatNodeId(n.node_id)) === selCanonical);
          return selectedNode ? toConfigReadCandidate(selectedNode).nodeName : selectedUnreadNodeId;
        })()
      : null
  );

  let selectedNodeIdAny = $derived(
    $configSidebarStore.selectedSegment?.nodeId ?? $configSidebarStore.selectedNodeId ?? null
  );

  // Config-acquisition workflow owner (preflight → missing-CDI download →
  // reads → progress → cancel). Route delegates intent and subscribes to its
  // reactive getters; the shared cached-CDI set lives in `cdiCacheStore` (S6).
  const configAcquisition = new ConfigAcquisitionOrchestrator({
    getNodes: () => nodes,
    getReadNodeIds: () => get(configReadNodesStore),
    getCdiXml,
    downloadCdi,
    readAllConfigValues: (nodeId, nodeIndex, totalNodes) => (
      readAllConfigValues(nodeId, undefined, nodeIndex, totalNodes)
    ),
    cancelConfigReading,
    markNodeConfigRead,
    refreshTree: (nodeId) => nodeTreeStore.refreshTree(nodeId),
    loadTree: (nodeId) => nodeTreeStore.loadTree(nodeId),
    recomputeConnectorCompatibility,
    setErrorMessage: (message) => { errorMessage = message; },
  });

  // CDI-inspection workflow owner (read-only XML viewer + menu re-download).
  // Separate from acquisition: inspecting CDI does not read config values.
  const cdiInspection = new CdiInspectionOrchestrator({
    getCdiXml,
    downloadCdi,
    getRedownloadCandidates: () => nodes.map((node) => toConfigReadCandidate(node)),
  });

  // Check connection status on mount
  onMount(() => {
    // Spec 013 / S3: keep the save-progress modal in sync with backend phases.
    void saveProgressStore.startListening();
    const unlistens: Array<() => void> = [];
    unlistens.push(
      installMenuShortcuts({
        guards: {
          canOpenLayout: canOpenLayoutAction,
          canCloseLayout: canCloseLayoutAction,
          canSaveLayout: canSaveLayoutAction,
          canSaveLayoutAs: canSaveLayoutAsAction,
        },
        actions: {
          openLayout: runOpenLayoutAction,
          closeLayout: runCloseLayoutAction,
          saveLayout: runSaveLayoutAction,
          saveLayoutAs: runSaveLayoutAsAction,
        },
      })
    );

    (async () => {
      await bootstrapStartupLifecycle({
        getConnectionStatus: async () => await invoke('get_connection_status') as {
          connected: boolean;
          config?: { name?: string | null; host?: string | null; port?: string | number | null; serialPort?: string | null } | null;
        },
        setLayoutConnected: (value) => {
          layoutStore.setConnected(value);
        },
        setConnectionLabel: (label) => {
          syncSessionOrchestrator.setConnectionLabel(label);
        },
        onConnectionStatusError: (error) => {
          console.error('Failed to get connection status:', error);
        },
        // Feature 006: Start bowties store listener so cdi-read-complete is captured
        // regardless of whether the user has visited the Bowties page.
        // Must be awaited so the listener is registered before layout restore work.
        startBowtieListening: () => bowtieCatalogStore.startListening(),
        // Spec 009 T015: Auto-reopen the most recent layout file on startup.
        restoreRecentOfflineLayout: () => restoreRecentOfflineLayout({
          getRecentLayout,
          restoreLayout: (path) => openOfflineLayoutWithReplay({
            path,
            openLayout: openLayoutDirectory,
            hydrateOfflineSnapshots,
            resetSidebar: () => {
              configSidebarStore.reset();
            },
            hydrateConnectorSelections: (layout) => {
              connectorSelectionsStore.hydrateFromLayout(layout);
            },
            onOpened: () => {
              showConnectionDialog = false;
            },
          }),
          clearRecentLayout,
          resetLayoutStateForNoLayout,
          resetLayoutOpenPhase,
          onRestored: (opened) => {
            partialCaptureNodesStore.replace(opened.partialNodes);
            currentLayoutSnapshots = opened.nodeSnapshots;
            if (opened.recoveryOccurred) {
              toast.push('Previous save was interrupted and has been restored.', {
                theme: { '--toastBackground': '#fff4ce', '--toastColor': '#4f3a04', '--toastBarBackground': '#835b00' },
              });
            }
          },
          onWarning: (message, error) => {
            console.warn(message, error);
          },
        }),
        // Spec 007: Refresh trees as config values and event roles merge server-side.
        startNodeTreeListening: () => {
          nodeTreeStore.startListening((_nodeId) => {
            // After edit layer refactor: draft reconciliation replaces restamp.
            configChangesStore.pruneResolvedDraftsForNode(_nodeId);
            void recomputeConnectorCompatibility(_nodeId);
          });
        },
        hasLayoutFile: () => layoutStore.hasLayoutFile,
        resetFreshLiveSessionState,
        probeForNodes,
      });

      // T050: Prompt-to-save guard on app close (FR-024)
      // Always prevent the native close so we can disconnect gracefully
      // (sends FIN instead of RST to the LCC hub) and, when dirty, show
      // the unsaved-changes dialog before exiting.
      const appWindow = getCurrentWebviewWindow();
      unlistens.push(await appWindow.onCloseRequested(async (event) => {
        if (isForceClosing) return;
        event.preventDefault();

        const hasUnsaved =
          hasUnsavedPromptChanges(
            nodeTreeStore.trees.keys(),
            bowtieMetadataStore.isDirty,
            offlineChangesStore.draftCount,
            effectiveNodeStore.isDirty,
            offlineChangesStore.revertedPersistedCount,
          );
        if (hasUnsaved) {
          unsavedDialog = {
            message: 'You have unsaved changes. Exit without saving?',
            confirmLabel: 'Exit Without Saving',
            proceed: async () => {
              isForceClosing = true;
              bowtieMetadataStore.clearAll();
              await invoke('disconnect_lcc');
              appWindow.close();
            },
          };
        } else {
          isForceClosing = true;
          await invoke('disconnect_lcc');
          appWindow.close();
        }
      }));

      // T063: Setup config-read-progress event listener
      unlistens.push(await listen<ReadProgressState>('config-read-progress', (event) => {
        configAcquisition.applyProgressEvent(event.payload);
      }));

      // Reactive node discovery: nodes appear one-by-one as VerifiedNode replies arrive.
      // Register in backend cache, add skeleton to local list, then fetch SNIP+PIP per node.
      // When a layout is open, nodes may already exist as offline skeletons (synthetic alias,
      // no backend proxy). In that case we upgrade them with the real bus alias and proceed
      // to registerNode + SNIP/PIP so backend proxies are created for sync.
      unlistens.push(await listen<{ nodeId: string; alias: number; timestamp: string }>('lcc-node-discovered', async (event) => {
        if (!layoutStore.isConnected) return; // ignore stray events after disconnect
        const { nodeId, alias } = event.payload;

        // Bug 1 fix: pass liveNodes (excludes placeholders) instead of nodes
        // (allEntries). The discovery pipeline uses `keyOf(node)` which calls
        // `nodeKey(formatNodeId(node.node_id))` — placeholder entries have
        // node_id: [] and crash that path. replaceLiveRoster preserves
        // placeholders from the store independently.
        const result = await handleDiscoveredNode({
          currentNodes: liveNodes,
          getCurrentNodes: () => liveNodes,
          nodeId,
          alias,
          registerNode,
          querySnip,
          queryPip,
          publishNodes: (nextNodes) => {
            nodeRoster.replaceLiveRoster(nextNodes);
          },
        });
        nodeRoster.replaceLiveRoster(result.nodes);
        if (result.skipped) return;

        // Reset the discovery settling timer — wait for 1s of silence after the
        // last node-discovered event before triggering sync.
        syncSessionOrchestrator.scheduleAutoSync({
          hasLayoutFile: layoutStore.hasLayoutFile,
          pendingCount: offlineChangesStore.pendingCount,
          triggerSync: () => maybeTriggerSync(),
        });
      }));

      // D15: When a known node sends InitializationComplete (reboot/factory-reset),
      // re-query its SNIP+PIP so the UI reflects any changed configuration.
      unlistens.push(await listen<{ nodeId: string; alias: number; timestamp: string }>('lcc-node-reinitialized', async (event) => {
        if (!layoutStore.isConnected) return;
        const { nodeId, alias } = event.payload;
        console.log(`[D15] Node ${nodeId} reinitialized — refreshing SNIP+PIP`);
        // Bug 1 fix: use liveNodes — same rationale as lcc-node-discovered.
        const result = await refreshReinitializedNode({
          currentNodes: liveNodes,
          getCurrentNodes: () => liveNodes,
          nodeId,
          alias,
          querySnip,
          queryPip,
          publishNodes: (nextNodes) => {
            nodeRoster.replaceLiveRoster(nextNodes);
          },
        });
        nodeRoster.replaceLiveRoster(result.nodes);
      }));

      // Native menu event listeners — relay OS menu clicks to handler
      // functions. The registrar owns the listen/teardown bookkeeping; the
      // route owns each action body (store access + unsaved-changes guards).
      unlistens.push(await registerMenuListeners({
        // S8-T17: disconnect must honor the unsaved-changes guard so the user
        // is warned if there are in-memory edits or fully-captured discovered
        // nodes not yet promoted to the layout file.
        disconnect: () => promptUnsaved(
          'Disconnecting will discard unsaved changes. Continue?',
          () => disconnect(),
        ),
        refresh: () => { if (layoutStore.isConnected) handleRefresh(); },
        traffic: () => { if (layoutStore.isConnected) openTrafficMonitor(); },
        viewCdi: () => {
          const state = get(configSidebarStore);
          const nodeId = state.selectedSegment?.nodeId ?? state.selectedNodeId;
          if (nodeId) cdiInspection.openViewer(nodeId);
        },
        redownloadCdi: () => {
          const state = get(configSidebarStore);
          const nodeId = state.selectedSegment?.nodeId ?? state.selectedNodeId;
          if (nodeId) cdiInspection.openRedownload(nodeId);
        },
        exit: () => {
          const win = getCurrentWebviewWindow();
          promptUnsaved('You have unsaved changes. Exit without saving?', async () => {
            isForceClosing = true;
            bowtieMetadataStore.clearAll();
            await invoke('disconnect_lcc');
            win.close();
          }, 'Exit Without Saving');
        },
        openLayout: () => runOpenLayoutAction(),
        closeLayout: () => runCloseLayoutAction(),
        saveLayout: () => runSaveLayoutAction(),
        saveLayoutAs: () => runSaveLayoutAsAction(),
        syncToBus: () => {
          if (layoutStore.isConnected && layoutStore.hasLayoutFile) forceSyncPanel();
        },
        addPlaceholderBoard: () => {
          // FR-017a: only meaningful with an active offline layout — the menu
          // item is already gated, but guard here too for completeness.
          if (layoutStore.hasLayoutFile) showAddBoardDialog = true;
        },
        deletePlaceholderBoard: () => {
          // S8.5 / T11 — gated server-side by the menu enable bit; reassert
          // here so a stale event from before a selection change cannot
          // bypass the placeholder check.
          const store = get(configSidebarStore);
          const selectedNodeId = store.selectedSegment?.nodeId ?? store.selectedNodeId;
          if (!selectedNodeId || !isPlaceholderInput(selectedNodeId)) return;
          pendingDeletePlaceholderKey = selectedNodeId;
        },
        diagnostics: async () => {
          try {
            const report = await invoke('get_diagnostic_report') as Record<string, unknown>;
            // Enrich with frontend-only device info before clipboard copy.
            if (connectedDeviceLabel && report.stats && typeof report.stats === 'object') {
              (report.stats as Record<string, unknown>).device = connectedDeviceLabel;
            }
            await navigator.clipboard.writeText(JSON.stringify(report, null, 2));
            // Brief visual feedback via console — a toast could be added later.
            console.info('Diagnostic report copied to clipboard');
          } catch (e) {
            console.error('Failed to copy diagnostic report:', e);
          }
        },
      }));
    })().finally(() => {
      startupBootstrapPending = false;
      // S6: load the known-layout registry so the picker (if shown) has
      // entries to display. Runs in parallel with the rest of startup; the
      // picker's render branch handles the "still loading" state on its own.
      void loadKnownLayouts({
        api: { getKnownLayouts },
        store: knownLayoutsStore,
        onError: (err) => console.warn('[startup] Failed to load known layouts:', err),
      });
    });

    // Cleanup all listeners on component unmount
    return () => {
      syncSessionOrchestrator.cancelPendingTrigger();
      unlistens.forEach(u => u());
    };
  });

  /** Device preset label for the active connection (frontend-only, for diagnostics). */
  let connectedDeviceLabel: string | null = null;

  function handleConnected(e: CustomEvent<{ config: any; device: string }>) {
    connectedDeviceLabel = e.detail.device;
    syncSessionOrchestrator.connect(e.detail.config);
  }

  /**
   * After connecting, check if an offline layout with pending changes is active.
   * If so, compute match status and build a sync session, then show the panel.
   */
  async function maybeTriggerSync() {
    await syncSessionOrchestrator.maybeTriggerSync({
      hasLayoutFile: layoutStore.hasLayoutFile,
      pendingCount: offlineChangesStore.pendingCount,
      discoveredNodeIds: nodes.map((node) => canonicalizeNodeId(formatNodeId(node.node_id))),
      syncPanelStore,
      showSyncPanel: () => {
        syncPanelVisible = true;
      },
    });
  }

  /** Re-open the sync panel on demand (e.g. from menu or OfflineBanner button). */
  async function forceSyncPanel() {
    await syncSessionOrchestrator.forceSyncPanel({
      hasLayoutFile: layoutStore.hasLayoutFile,
      pendingCount: offlineChangesStore.pendingCount,
      discoveredNodeIds: nodes.map((node) => canonicalizeNodeId(formatNodeId(node.node_id))),
      syncPanelStore,
      showSyncPanel: () => {
        syncPanelVisible = true;
      },
    });
  }

  async function disconnect() {
    connectedDeviceLabel = null;
    await syncSessionOrchestrator.disconnect();
  }

  /**
   * Tear down the live bus session before switching to a different layout.
   * Skips the offline-rehydration branch of the regular disconnect path
   * because the layout (and its snapshots) are about to be replaced.
   */
  async function disconnectBeforeLayoutSwitch() {
    await syncSessionOrchestrator.disconnectBeforeLayoutSwitch();
  }

  /** Fire-and-forget probe — nodes appear via lcc-node-discovered events */
  async function probeForNodes() {
    try {
      await probeNodesApi();
    } catch (e) {
      console.error("Probe failed:", e);
    }
  }

  /**
   * Re-probe the network. Culls stale nodes (those that don't reply) from the
   * UI; new or returning nodes appear automatically via lcc-node-discovered events.
   */
  async function handleRefresh() {
    if (probing) return;
    errorMessage = "";
    probing = true;
    try {
      const staleIds = await refreshAllNodes();
      if (staleIds.length > 0) {
        const sidebarState = get(configSidebarStore);
        const selectedId = sidebarState.selectedSegment?.nodeId ?? sidebarState.selectedNodeId;
        // Bug 1 fix: use liveNodes — same rationale as lcc-node-discovered.
        const refreshed = reconcileRefreshState({
          currentNodes: liveNodes,
          staleNodeIds: staleIds,
          selectedNodeId: selectedId,
          nodesWithCdi: cdiCacheStore.nodes,
        });

        nodeRoster.replaceLiveRoster(refreshed.nodes);
        // Only clean up state for nodes that actually left — preserve config read
        // status and CDI data for nodes that are still present.
        removeNodesConfigRead(refreshed.removedNodeIds);
        cdiCacheStore.replace(refreshed.nodesWithCdi);
        if (refreshed.shouldResetSidebar) {
          configSidebarStore.reset();
        }
      }
    } catch (e) {
      console.error("Refresh failed:", e);
      errorMessage = `Refresh failed: ${e}`;
    } finally {
      probing = false;
    }
  }

  async function openTrafficMonitor() {
    const win = new WebviewWindow('traffic', {
      url: '/traffic',
      title: 'LCC Traffic Monitor',
      width: 960,
      height: 640,
      maximizable: true,
      minimizable: true,
      visible: false,
    });
    // If a window with this label already exists Tauri emits tauri://error
    // instead of creating a duplicate — just focus the existing one.
    win.once('tauri://error', async () => {
      const existing = await WebviewWindow.getByLabel('traffic');
      if (existing) await existing.setFocus();
    });
  }

  // Sync native menu item enable/disable state with current app state.
  // Tauri v2 has no "menu will open" event, so we push state eagerly whenever
  // any of the tracked reactive values change.
  async function syncMenuState(
    conn: boolean,
    busy: boolean,
    canViewCdi: boolean,
    canRedownloadCdi: boolean,
    canOpenLayout: boolean,
    canCloseLayout: boolean,
    canSaveLayout: boolean,
    canSaveLayoutAs: boolean,
    canSyncToBus: boolean,
    canAddPlaceholderBoard: boolean,
    canDeletePlaceholderBoard: boolean,
  ) {
    try {
      await invoke("update_menu_state", {
        connected: conn,
        isBusy: busy,
        canViewCdi,
        canRedownloadCdi,
        canOpenLayout,
        canCloseLayout,
        canSaveLayout,
        canSaveLayoutAs,
        canSyncToBus,
        canAddPlaceholderBoard,
        canDeletePlaceholderBoard,
      });
    } catch (e) {
      console.warn("Failed to update menu state:", e);
    }
  }

  $effect(() => {
    const conn = layoutStore.isConnected;
    const busy = probing || configAcquisition.readingRemaining;
    const store = $configSidebarStore;

    // Determine which node is selected
    const selectedNodeId = store.selectedSegment?.nodeId ?? store.selectedNodeId;

    // Build a reactive snapshot for the pure menu-enable policy. Reading each
    // store value here keeps the effect tracking them; `computeMenuEnableState`
    // owns the rules (ADR-0011: `effectiveNodeStore.isDirty` is the aggregate
    // edit facade; `layoutStore.isDirty` is struct-only).
    const menu = computeMenuEnableState({
      connected: conn,
      busy,
      hasSelection: !!selectedNodeId,
      hasSelectedSegment: !!store.selectedSegment,
      selectedNodeHasCdi: !!selectedNodeId && cdiCacheStore.has(selectedNodeId),
      selectedIsPlaceholder: !!selectedNodeId && isPlaceholderInput(selectedNodeId),
      selectedInRoster: !!selectedNodeId && nodeRoster.has(selectedNodeId),
      layoutLoaded: layoutStore.isLoaded,
      layoutDirty: layoutStore.isDirty,
      metaDirty: bowtieMetadataStore.isDirty,
      hasActiveLayout: !!layoutStore.activeContext,
      hasLayoutFile: layoutStore.hasLayoutFile,
      hasInMemoryEdits: effectiveNodeStore.isDirty,
      pendingSyncCount: offlineChangesStore.pendingCount,
    });

    syncMenuState(
      conn,
      busy,
      menu.canViewCdi,
      menu.canRedownloadCdi,
      menu.canOpenLayout,
      menu.canCloseLayout,
      menu.canSaveLayout,
      menu.canSaveLayoutAs,
      menu.canSyncToBus,
      menu.canAddPlaceholderBoard,
      menu.canDeletePlaceholderBoard,
    );
  });

  $effect(() => {
    const selectedNodeId = selectedNodeIdAny;
    const trees = nodeTreeStore.trees;

    if (!selectedNodeId) {
      return;
    }

    const selectedTree = trees.get(selectedNodeId);
    if (!selectedTree) {
      return;
    }

    const selectedConnectorProfile = selectedTree.connectorProfile ?? null;

    const cachedProfile = connectorSelectionsStore.getProfile(selectedNodeId);
    const cachedDocument = connectorSelectionsStore.getDocument(selectedNodeId);
    if (!selectedConnectorProfile) {
      if (
        cachedProfile
        || cachedDocument
        || connectorSelectionsStore.getWarnings(selectedNodeId).length > 0
      ) {
        void connectorSelectionsStore.loadNode(selectedNodeId, null);
      }
      return;
    }

    if (
      cachedProfile?.carrierKey === selectedConnectorProfile.carrierKey
      && cachedDocument
    ) {
      return;
    }

    void connectorSelectionsStore.loadNode(selectedNodeId, selectedConnectorProfile);
  });

  async function handleConnectorSelectionChange(detail: {
    nodeId: string;
    slotId: string;
    selectedDaughterboardId: string | null;
  }): Promise<void> {
    try {
      const saved = await applyConnectorSelectionChange(detail);

      if (!saved) {
        errorDialog = {
          title: 'Failed to Update Connector Selection',
          message: 'The selected node does not have a connector profile loaded yet.',
        };
      }
    } catch (error) {
      errorDialog = {
        title: 'Failed to Save Connector Selection',
        message: String(error ?? 'Unknown error'),
      };
    }
  }

  // Dynamic window title — reflects current layout file name and dirty state.
  // Using $derived (not computed inside $effect) so Svelte 5 reliably tracks
  // all reactive dependencies: _layout, _path, _dirty, and the SvelteMap edits.
  let windowTitle = $derived(activeLayoutLabel ? `Bowties::LCC - ${activeLayoutLabel}` : 'Bowties::LCC');
  $effect(() => {
    getCurrentWebviewWindow().setTitle(windowTitle).catch(() => {});
  });

  // Spec 013 / S6: render the layout picker when no layout is active. The
  // picker fully gates access to the toolbar and main content. It does not
  // re-appear when the user disconnects from the bus — disconnecting keeps
  // the active layout open in offline mode.
  let pickerActive = $derived(
    !startupBootstrapPending
    && !$layoutOpenInProgress
    && !layoutStore.activeContext
  );
</script>


<div class="app-shell">

  {#if pickerActive}
    <!-- ═══ LAYOUT PICKER (Spec 013 / S6) ═══ -->
    <LayoutPicker
      entries={knownLayoutsStore.entries}
      loaded={knownLayoutsStore.loaded}
      busy={knownLayoutsStore.busy}
      onOpen={handlePickerOpen}
      onBrowse={handlePickerBrowse}
      onCreate={handlePickerCreate}
      onRemove={handlePickerRemove}
    />
  {:else}

  <!-- ═══ TOOLBAR (connected only) ═══ -->
  {#if layoutStore.isConnected || layoutStore.isOfflineMode}
    <div class="toolbar" role="toolbar" aria-label="Main toolbar">
      <div class="toolbar-left">
        <!-- Segmented mode control: Config | Bowties -->
        <div
          class="mode-control"
          role="group"
          aria-label="View mode"
        >
          <button
            class="toolbar-seg"
            class:toolbar-btn-active={activeTab === 'config'}
            aria-pressed={activeTab === 'config'}
            onclick={() => activeTab = 'config'}
            onkeydown={handleModeKeydown}
            title="Configuration view"
          >
            <span class="tb-icon">⚙</span>
            <span>Config</span>
          </button>
          <button
            class="toolbar-seg"
            class:toolbar-btn-active={activeTab === 'bowties'}
            aria-pressed={activeTab === 'bowties'}
            onclick={() => activeTab = 'bowties'}
            onkeydown={handleModeKeydown}
            title="Bowtie connections view"
          >
            <span class="tb-icon">🎀</span>
            <span>Bowties</span>
          </button>
        </div>
        {#if layoutStore.isConnected && (configAcquisition.readingRemaining || unreadCount > 0)}
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button
            class="toolbar-btn"
            onclick={() => configAcquisition.readRemaining()}
            disabled={probing || configAcquisition.readingRemaining}
            title="Read configuration values for nodes not yet read"
          >
            <span class="tb-icon" class:tb-spin={configAcquisition.readingRemaining}>⟳</span>
            <span>{configAcquisition.readingRemaining ? 'Reading…' : `Read Remaining (${unreadCount})`}</span>
          </button>
        {/if}
        <SaveControls toolbar={true} bind:this={saveControlsRef} onSave={() => saveCurrentCaptureToFile(false)} onSaveAs={() => saveCurrentCaptureToFile(true)} />
      </div>
      <div class="toolbar-right">
        {#if layoutStore.isConnected}
          <button
            class="toolbar-status-btn"
            onclick={disconnect}
            title="Disconnect from {syncSessionOrchestrator.connectionLabel}"
            aria-label="Disconnect from {syncSessionOrchestrator.connectionLabel}"
          >
            <span class="status-dot status-connected" aria-hidden="true"></span>
            <span class="status-text">{syncSessionOrchestrator.connectionLabel}</span>
            <span class="status-disconnect-hint" aria-hidden="true">Disconnect</span>
          </button>
        {:else}
          <button
            class="toolbar-status-btn toolbar-status-btn--offline"
            onclick={() => showConnectionDialog = true}
            title="Offline. Click to connect."
            aria-label="Offline. Click to connect."
          >
            <span class="status-dot status-offline" aria-hidden="true"></span>
            <span class="status-text">Offline</span>
          </button>
        {/if}
      </div>
    </div>
  {/if}

  <!-- ═══ DISCOVERY PROGRESS MODAL ═══ -->
  <DiscoveryProgressModal
    visible={configAcquisition.discoveryModalVisible}
    phase={configAcquisition.discoveryPhase}
    readProgress={configAcquisition.readProgress}
    isCancelling={configAcquisition.isCancelling}
    nodeReadStates={configAcquisition.nodeReadStates}
    onCancel={() => configAcquisition.cancel()}
  />

  <!-- ═══ SYNC PANEL MODAL ═══ -->
  <SyncPanel bind:visible={syncPanelVisible} />

  {#if $layoutOpenInProgress}
    <div class="layout-loading-backdrop" role="presentation">
      <div class="layout-loading-dialog" role="dialog" aria-modal="true" aria-live="polite" aria-label="Loading layout">
        <div class="layout-loading-spinner" aria-hidden="true"></div>
        <h2>Opening layout</h2>
        <p>{$layoutOpenStatusText}</p>
      </div>
    </div>
  {/if}

  <!-- ═══ ERROR BANNER ═══ -->
  {#if errorMessage}
    <div class="error-banner" role="alert">
      <span class="error-banner-text">⚠ {errorMessage}</span>
      <button class="error-banner-close" onclick={() => errorMessage = ''} aria-label="Dismiss error">✕</button>
    </div>
  {/if}

  <!-- ═══ OFFLINE BANNER ═══ -->
  {#if layoutStore.isOfflineMode}
    <OfflineBanner
      capturedAt={layoutStore.activeContext?.capturedAt ?? null}
      layoutId={activeLayoutLabel}
      isConnected={layoutStore.isConnected}
      isSyncDismissed={syncPanelStore.isDismissed}
      onsyncrequest={forceSyncPanel}
    />
  {/if}

  <!-- ═══ MAIN CONTENT ═══ -->
  <div class="main-content">
    {#if startupBootstrapPending}
      <div class="startup-placeholder" aria-live="polite">
        <p>Loading Bowties…</p>
      </div>
    {:else if !layoutStore.isConnected && !layoutStore.hasLayoutFile && nodes.length === 0}
      <div class="connect-area">
        <ConnectionManager on:connected={handleConnected} />
      </div>

    {:else if nodes.length === 0}
      <div class="empty-area">
        <p class="empty-status">No nodes found.</p>
        <p class="empty-hint">Click <strong>Refresh Nodes</strong> in the toolbar to scan the network again.</p>
      </div>

    {:else if activeTab === 'bowties'}
      <!-- Feature 006: Bowties catalog in-page tab (no navigation) -->
      <BowtieCatalogPanel
        highlightedEventIdHex={bowtieFocusStore.highlightedEventIdHex}
        onReadConfig={() => configAcquisition.readRemaining()}
        hasUnreadNodes={layoutStore.isConnected && unreadCount > 0}
        readingConfig={configAcquisition.readingRemaining}
        {unreadCount}
        nodesCount={nodes.length}
      />

    {:else}
      <!-- FR-001: two-panel layout — resizable sidebar + scrollable main area -->
      <div class="config-layout" style="--config-sidebar-width: {$sidebarWidthStore}px">
        <ConfigSidebar
          on:readNodeConfig={(e) => configAcquisition.readSingleNode(e.detail.nodeId)}
        />
        <SidebarResizeHandle
          currentWidth={$sidebarWidthStore}
          onresize={(width) => sidebarWidthStore.setWidth(width)}
        />
        <div class="config-main">
          {#if showConfigCta || showCaptureRemainingCta}
            <div class="config-cta-panel">
              <h2 class="cta-title">Node Configuration</h2>
              <p class="cta-desc">
                {nodes.length} {nodes.length === 1 ? 'node' : 'nodes'} discovered.
                Click below to read their configuration.
              </p>
              <button
                class="cta-btn"
                onclick={() => configAcquisition.readRemaining()}
                disabled={configAcquisition.readingRemaining}
              >
                Read Node Configuration
              </button>
              {#if unreadCount > 0}
                <span class="cta-badge">{unreadCount} unread</span>
              {/if}
            </div>
          {:else if selectedUnreadNodeId}
            <div class="config-cta-panel">
              <h2 class="cta-title">{selectedUnreadNodeName}</h2>
              {#if partialCaptureNodesStore.has(selectedUnreadNodeId)}
                <MissingCaptureBadge text="(Not captured)" />
              {/if}
              <p class="cta-desc">
                Configuration has not been read from this node yet.
              </p>
              <button
                class="cta-btn"
                onclick={() => configAcquisition.readSingleNode(selectedUnreadNodeId)}
                disabled={configAcquisition.readingRemaining}
              >
                Read Configuration
              </button>
            </div>
          {:else}
            {#if layoutStore.hasLayoutFile && selectedNodeIdAny && partialCaptureNodesStore.has(selectedNodeIdAny)}
              <div class="partial-capture-note">
                <MissingCaptureBadge text="(Not captured)" />
                <span>Some values for this node were not captured and remain read-only offline.</span>
              </div>
            {/if}
            {#if selectedNodeIdAny && isPlaceholderInput(selectedNodeIdAny)}
              <!-- S8.5 / T11 — in-pane shortcut for deleting the selected placeholder. -->
              <div class="placeholder-pane-actions">
                <button
                  class="placeholder-delete-btn"
                  data-testid="delete-placeholder-board-btn"
                  onclick={() => { pendingDeletePlaceholderKey = selectedNodeIdAny; }}
                >
                  Delete Placeholder Board…
                </button>
              </div>
            {/if}
            <SegmentView on:changeConnectorSelection={(e) => handleConnectorSelectionChange(e.detail)} />
          {/if}
        </div>
      </div>
    {/if}
  </div>

  {/if}
</div>

{#if showConnectionDialog}
  <div
    class="connect-overlay"
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-label="Connect to LCC network"
    onkeydown={(e) => { if (e.key === 'Escape') showConnectionDialog = false; }}
  >
    <div class="connect-modal">
      <ConnectionManager on:connected={handleConnected} />
      <div class="connect-modal-actions">
        <button class="active-layout-btn" onclick={() => showConnectionDialog = false}>Close</button>
      </div>
    </div>
  </div>
{/if}

<!-- CDI XML Viewer Modal -->
<CdiXmlViewer
  visible={cdiInspection.viewerVisible}
  nodeId={cdiInspection.viewerNodeId}
  xmlContent={cdiInspection.viewerXmlContent}
  status={cdiInspection.viewerStatus}
  errorMessage={cdiInspection.viewerErrorMessage}
  onClose={() => cdiInspection.closeViewer()}
/>

<!-- CDI Re-download Dialog — compact download-only dialog from menu-redownload-cdi -->
{#if cdiInspection.redownloadVisible && cdiInspection.redownloadNodeId && cdiInspection.redownloadNodeName}
  <CdiRedownloadDialog
    nodeId={cdiInspection.redownloadNodeId}
    nodeName={cdiInspection.redownloadNodeName}
    onClose={() => cdiInspection.closeRedownload()}
  />
{/if}

<!-- CDI Download Dialog — shown when nodes lack a cached CDI after discovery -->
{#if configAcquisition.cdiDownloadDialogVisible}
  <CdiDownloadDialog
    nodes={configAcquisition.cdiMissingNodes}
    downloading={configAcquisition.cdiDownloading}
    downloadedCount={configAcquisition.cdiDownloadedCount}
    onDownload={() => configAcquisition.downloadMissingCdi()}
    onCancel={() => configAcquisition.cancelDownload()}
  />
{/if}

<!-- T050: Prompt-to-save guard dialog (FR-024) -->
{#if unsavedDialog}
  <div
    class="unsaved-overlay"
    role="dialog"
    aria-modal="true"
    aria-label="Unsaved changes warning"
  >
    <div class="unsaved-dialog">
      <h3 class="unsaved-title">Unsaved Changes</h3>
      <p class="unsaved-body">{unsavedDialog.message}</p>
      <div class="unsaved-actions">
        <button
          class="unsaved-btn unsaved-btn-secondary"
          onclick={() => { unsavedDialog = null; }}
        >Cancel</button>
        <button
          class="unsaved-btn unsaved-btn-danger"
          onclick={() => {
            const proceed = unsavedDialog?.proceed;
            unsavedDialog = null;
            proceed?.();
          }}
        >{unsavedDialog.confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

{#if errorDialog}
  <ErrorDialog
    title={errorDialog.title}
    message={errorDialog.message}
    onClose={() => { errorDialog = null; }}
  />
{/if}

{#if showAddBoardDialog}
  <AddBoardDialog
    onCancel={() => { showAddBoardDialog = false; }}
    onAdded={() => { showAddBoardDialog = false; }}
  />
{/if}

{#if pendingDeletePlaceholderKey}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div
    class="unsaved-overlay"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) pendingDeletePlaceholderKey = null; }}
  >
    <div
      class="unsaved-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="delete-placeholder-title"
    >
      <h3 id="delete-placeholder-title" class="unsaved-title">Delete placeholder board?</h3>
      <p class="unsaved-body">
        This will remove the placeholder board and any unsaved configuration
        changes for it. This cannot be undone.
      </p>
      <div class="unsaved-actions">
        <button
          class="unsaved-btn unsaved-btn-secondary"
          onclick={() => { pendingDeletePlaceholderKey = null; }}
        >
          Cancel
        </button>
        <button
          class="unsaved-btn unsaved-btn-danger"
          data-testid="confirm-delete-placeholder"
          onclick={async () => {
            const key = pendingDeletePlaceholderKey;
            pendingDeletePlaceholderKey = null;
            if (!key) return;
            try {
              await deletePlaceholderBoard({ nodeKey: key, confirm: async () => true });
            } catch (e) {
              console.error('Failed to delete placeholder board:', e);
              errorDialog = { title: 'Delete failed', message: String(e) };
            }
          }}
        >
          Delete
        </button>
      </div>
    </div>
  </div>
{/if}

<SaveProgressDialog />

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: white;
    overflow: hidden;
  }

  /* ─── App Shell ─────────────────────────────────────── */

  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .active-layout-btn {
    border: 1px solid #cbd5e1;
    background: #ffffff;
    color: #1e293b;
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 12px;
  }

  .active-layout-btn:hover {
    background: #f1f5f9;
  }

  .startup-placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #475569;
    font-size: 14px;
  }

  .partial-capture-note {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 12px;
    padding: 8px 10px;
    border: 1px solid #fecaca;
    border-radius: 8px;
    background: #fff1f2;
    color: #7f1d1d;
    font-size: 12px;
  }

  .status-offline {
    background: #f59e0b;
  }

  .connect-overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.38);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 60;
  }

  .connect-modal {
    width: min(560px, 95vw);
    max-height: 90vh;
    overflow: auto;
    border-radius: 12px;
    background: #ffffff;
    border: 1px solid #cbd5e1;
    box-shadow: 0 20px 60px rgba(15, 23, 42, 0.28);
    padding: 14px;
  }

  .connect-modal-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 10px;
  }

  .connect-modal :global(.cm-card) {
    background: transparent;
    border: 0;
    border-radius: 0;
    box-shadow: none;
    padding: 0;
    min-width: 0;
    width: 100%;
    max-width: 100%;
  }

  /* ─── Status indicator (toolbar) ───────────────────── */

  .toolbar-status-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    font-size: 12px;
    color: #555;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, border-color 0.12s, color 0.12s, box-shadow 0.12s;
  }

  .toolbar-status-btn:hover {
    background: #fee2e2;
    border-color: #fca5a5;
    color: #b91c1c;
  }

  .toolbar-status-btn--offline:hover {
    background: #eff6ff;
    border-color: #93c5fd;
    color: #1d4ed8;
  }

  .toolbar-status-btn:not(.toolbar-status-btn--offline):hover .status-text {
    display: none;
  }

  .status-disconnect-hint {
    display: none;
  }

  .toolbar-status-btn:hover .status-disconnect-hint {
    display: inline;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .status-connected    { background: #10b981; }

  @keyframes status-pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.4; }
  }

  /* ─── Toolbar ───────────────────────────────────────── */

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #f3f4f6;
    border-bottom: 1px solid #d1d5db;
    padding: 0 8px;
    height: 40px;
    flex-shrink: 0;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .layout-loading-backdrop {
    position: fixed;
    inset: 0;
    z-index: 2200;
    background: rgba(15, 23, 42, 0.24);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .layout-loading-dialog {
    width: min(360px, calc(100vw - 40px));
    background: #fff;
    color: #1f2937;
    border: 1px solid #d1d5db;
    border-radius: 10px;
    box-shadow: 0 16px 38px rgba(15, 23, 42, 0.24);
    padding: 18px 18px 16px;
    text-align: center;
  }

  .layout-loading-dialog h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.01em;
  }

  .layout-loading-dialog p {
    margin: 8px 0 0;
    font-size: 13px;
    color: #4b5563;
  }

  .layout-loading-spinner {
    width: 18px;
    height: 18px;
    margin: 0 auto 10px;
    border-radius: 50%;
    border: 2px solid #dbeafe;
    border-top-color: #2563eb;
    animation: layout-loading-spin 0.85s linear infinite;
  }

  @keyframes layout-loading-spin {
    to { transform: rotate(360deg); }
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    font-size: 13px;
    color: #374151;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, border-color 0.12s, box-shadow 0.12s;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: #f0f4ff;
    border-color: #c7d2fe;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .toolbar-btn:disabled {
    background: #fafafa;
    border-color: #ebebeb;
    box-shadow: none;
    color: #bbb;
    cursor: not-allowed;
    pointer-events: none;
  }

  .tb-icon {
    font-size: 15px;
  }

  .tb-spin {
    display: inline-block;
    animation: tb-rotate 1s linear infinite;
  }

  @keyframes tb-rotate {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  .toolbar-sep {
    width: 1px;
    height: 20px;
    background: #d1d5db;
    margin: 0 4px;
  }

  /* Active (pressed) state for toggle toolbar buttons */
  .toolbar-btn-active {
    background: #eff6ff;
    border-color: #6366f1 !important;
    color: #4338ca !important;
  }

  /* ── Segmented mode control (Config | Bowties capsule) ── */

  .mode-control {
    display: flex;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    overflow: hidden;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  .toolbar-seg {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    background: #ffffff;
    border: none;
    border-radius: 0;
    font-size: 13px;
    color: #374151;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, color 0.12s;
  }

  .toolbar-seg + .toolbar-seg {
    border-left: 1px solid #e0e0e0;
  }

  .toolbar-seg:hover:not(:disabled) {
    background: #f0f4ff;
    color: #4338ca;
  }

  .toolbar-seg.toolbar-btn-active {
    background: #eff6ff;
    color: #4338ca;
    font-weight: 500;
  }



  /* ─── Error Banner ──────────────────────────────────── */

  .error-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    height: 32px;
    background: #fee2e2;
    border-bottom: 1px solid #fecaca;
    flex-shrink: 0;
  }

  .error-banner-text {
    flex: 1;
    font-size: 12px;
    color: #dc2626;
  }

  .error-banner-close {
    background: none;
    border: none;
    color: #dc2626;
    cursor: pointer;
    font-size: 14px;
    padding: 2px 4px;
    border-radius: 3px;
  }

  .error-banner-close:hover {
    background: #fecaca;
  }

  /* ─── Main Content ──────────────────────────────────── */

  .main-content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ─── Connect form ──────────────────────────────────── */

  .connect-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
  }

  /* ─── Empty / loading state ─────────────────────────── */

  .empty-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: #6b7280;
    gap: 4px;
  }

  .empty-status {
    margin: 0;
    font-size: 14px;
  }

  .empty-hint {
    margin: 4px 0 0 0;
    font-size: 13px;
  }

  /* ─── Config Layout (two-panel: sidebar + main) ─────── */

  .config-layout {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
    overflow: hidden;
  }

  .config-main {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* T050: Unsaved changes dialog (FR-024) */
  .unsaved-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .unsaved-dialog {
    background: #fff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
    padding: 20px 24px;
    width: 380px;
    max-width: 95vw;
  }

  .unsaved-title {
    margin: 0 0 10px;
    font-size: 0.95rem;
    font-weight: 600;
    color: #1f2937;
  }

  .unsaved-body {
    margin: 0 0 16px;
    font-size: 0.85rem;
    color: #6b7280;
    line-height: 1.5;
  }

  .unsaved-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .unsaved-btn {
    padding: 6px 14px;
    font-size: 0.82rem;
    font-weight: 500;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.15s;
  }

  .unsaved-btn-secondary {
    color: #374151;
    background: #fff;
    border-color: #d1d5db;
  }

  .unsaved-btn-secondary:hover {
    background: #f9fafb;
  }

  .unsaved-btn-danger {
    color: #fff;
    background: #dc2626;
    border-color: #dc2626;
  }

  .unsaved-btn-danger:hover {
    background: #b91c1c;
  }

  /* Spec 014 / S8.5 / T11: in-pane "Delete Placeholder Board" action. */
  .placeholder-pane-actions {
    display: flex;
    justify-content: flex-end;
    padding: 8px 12px 0;
  }

  .placeholder-delete-btn {
    padding: 4px 10px;
    font-size: 12px;
    font-weight: 500;
    color: #b91c1c;
    background: #fff;
    border: 1px solid #fca5a5;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .placeholder-delete-btn:hover {
    background: #fef2f2;
    border-color: #f87171;
  }

  /* ─── Read Configuration CTA Panel ─────────────────── */

  .config-cta-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    height: 100%;
    padding: 48px 32px;
    text-align: center;
  }

  .cta-title {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: #1e293b;
  }

  .cta-desc {
    margin: 0;
    font-size: 14px;
    color: #64748b;
    max-width: 360px;
    line-height: 1.6;
  }

  .cta-btn {
    padding: 10px 24px;
    font-size: 14px;
    font-weight: 500;
    background: #2563eb;
    color: #fff;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .cta-btn:hover:not(:disabled) {
    background: #1d4ed8;
  }

  .cta-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .cta-badge {
    font-size: 12px;
    color: #94a3b8;
  }
</style>

