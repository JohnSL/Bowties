/**
 * Pure derivation of native-menu item enable bits from a snapshot of app state.
 *
 * Single owner for the rules that decide which `File`/`Connection` menu items
 * are enabled. The route's reactive `$effect` builds a {@link MenuEnableInputs}
 * snapshot from its stores and passes the result to the `update_menu_state` IPC.
 * Keeping the rules here (instead of inline in the route effect) makes them
 * unit-testable and gives the menu-enable policy one home.
 *
 * Note: the keyboard-shortcut guards in the route intentionally use slightly
 * looser rules (e.g. they do not require an active layout to honor Ctrl+O) and
 * are a separate concern, so they are not derived here.
 */
export interface MenuEnableInputs {
  /** Live bus session is connected. */
  connected: boolean;
  /** A blocking workflow (probe / read-remaining) is in progress. */
  busy: boolean;
  /** A node is currently selected (segment node id or selected node id). */
  hasSelection: boolean;
  /** A config segment (not just a node) is selected. */
  hasSelectedSegment: boolean;
  /** The selected node has cached CDI available. */
  selectedNodeHasCdi: boolean;
  /** The selected node is an in-memory placeholder board. */
  selectedIsPlaceholder: boolean;
  /** The selected node is still present in the roster. */
  selectedInRoster: boolean;
  /** A layout file is loaded (struct present). */
  layoutLoaded: boolean;
  /** The loaded layout struct has unsaved edits. */
  layoutDirty: boolean;
  /** Bowtie metadata has unsaved edits. */
  metaDirty: boolean;
  /** A layout context is active (offline or live). */
  hasActiveLayout: boolean;
  /** The active context is backed by a layout file on disk. */
  hasLayoutFile: boolean;
  /** Aggregate "any in-memory change" signal (ADR-0011 facade). */
  hasInMemoryEdits: boolean;
  /** Count of pending offline changes awaiting sync to the bus. */
  pendingSyncCount: number;
}

export interface MenuEnableState {
  canViewCdi: boolean;
  canRedownloadCdi: boolean;
  canOpenLayout: boolean;
  canCloseLayout: boolean;
  canSaveLayout: boolean;
  canSaveLayoutAs: boolean;
  canSyncToBus: boolean;
  canAddPlaceholderBoard: boolean;
  canDeletePlaceholderBoard: boolean;
}

export function computeMenuEnableState(inputs: MenuEnableInputs): MenuEnableState {
  const {
    connected,
    busy,
    hasSelection,
    hasSelectedSegment,
    selectedNodeHasCdi,
    selectedIsPlaceholder,
    selectedInRoster,
    layoutLoaded,
    layoutDirty,
    metaDirty,
    hasActiveLayout,
    hasLayoutFile,
    hasInMemoryEdits,
    pendingSyncCount,
  } = inputs;

  const offlineActive = hasActiveLayout && hasLayoutFile;

  return {
    // Re-download CDI is available whenever any node is selected.
    canRedownloadCdi: connected && !busy && hasSelection,
    // View CDI is available when a segment is selected (segment ⇒ CDI exists)
    // or a node with cached CDI is selected.
    canViewCdi:
      connected && !busy && (hasSelectedSegment || (hasSelection && selectedNodeHasCdi)),
    // Open / Save As are meaningless while the layout picker is visible
    // (no active layout), so they gate on an active context.
    canOpenLayout: !busy && hasActiveLayout,
    canCloseLayout: hasActiveLayout,
    // ADR-0011: offline edits read the aggregate facade; the non-offline Save
    // gate uses struct/metadata dirty flags.
    canSaveLayout:
      !busy && ((offlineActive && hasInMemoryEdits) || (layoutLoaded && (layoutDirty || metaDirty))),
    canSaveLayoutAs: !busy && hasActiveLayout,
    canSyncToBus: connected && offlineActive && pendingSyncCount > 0,
    canAddPlaceholderBoard: !busy && offlineActive,
    canDeletePlaceholderBoard:
      !busy && offlineActive && hasSelection && selectedIsPlaceholder && selectedInRoster,
  };
}
