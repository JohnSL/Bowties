/**
 * Tests for LayoutStore — connection-aware offline mode semantics.
 *
 * State matrix (4 combinations of layout-open × connected):
 *
 * | Layout File | Connected | isOfflineMode | hasLayoutFile | User Sees                         |
 * |-------------|-----------|---------------|---------------|-----------------------------------|
 * | No          | No        | false         | false         | Connect area, no save controls    |
 * | No          | Yes       | false         | false         | Online editing, save → hardware   |
 * | Yes         | No        | true          | true          | Offline editing, save → file      |
 * | Yes         | Yes       | false         | true          | Online + layout, save → hardware, |
 * |             |           |               |               | sync panel triggers               |
 *
 * These tests verify the store-level computed properties that drive:
 * - Edit routing (modifiedValue vs offlineChangesStore)
 * - Save routing (writeModifiedValues vs saveLayoutFile)
 * - Sync panel triggering
 * - UI visibility (connect area, save controls, config CTA, partial badge)
 * - Save Layout menu availability
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock Tauri and API dependencies so the LayoutStore class can be imported
// without hitting real IPC calls.
vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(),
  open: vi.fn(),
}));
vi.mock('$lib/api/bowties', () => ({
  loadLayout: vi.fn(),
  saveLayout: vi.fn(),
  getRecentLayout: vi.fn(),
  setRecentLayout: vi.fn(),
  buildBowtieCatalog: vi.fn(),
}));
vi.mock('$lib/constants/layoutFiles', () => ({
  OFFLINE_LAYOUT_DEFAULT_FILENAME: 'layout.bowties-layout.yaml',
  offlineLayoutDialogFilter: () => ({ name: 'Layout', extensions: ['yaml'] }),
}));

import { layoutStore } from './layout.svelte';
import type { ActiveLayoutContext } from './layout.svelte';

// ── Helpers ────────────────────────────────────────────────────────────────────

function setOfflineLayoutContext(): void {
  layoutStore.setActiveContext({
    layoutId: 'test-layout',
    rootPath: '/tmp/test.layout',
    mode: 'offline_file',
    pendingOfflineChangeCount: 0,
  });
}

function setLegacyLayoutContext(): void {
  layoutStore.setActiveContext({
    layoutId: 'legacy-layout',
    rootPath: '/tmp/legacy.yaml',
    mode: 'legacy_file',
    pendingOfflineChangeCount: 0,
  });
}

beforeEach(() => {
  layoutStore.reset();
  layoutStore.setConnected(false);
});

describe('connector selection metadata', () => {
  it('creates an empty connector selection map for new layouts', () => {
    layoutStore.newLayout();

    expect(layoutStore.layout?.connectorSelections).toEqual({});
  });

  it('upserts connector selections using normalized node ids', () => {
    layoutStore.newLayout();

    layoutStore.upsertConnectorSelections('05.02.01.02.03.00', {
      carrierKey: 'rr-cirkits::tower-lcc',
      slotSelections: {
        'connector-a': {
          selectedDaughterboardId: 'BOD4-CP',
          status: 'selected',
        },
      },
      updatedAt: '2026-05-02T12:00:00Z',
    });

    expect(layoutStore.getConnectorSelections('050201020300')).toEqual({
      carrierKey: 'rr-cirkits::tower-lcc',
      slotSelections: {
        'connector-a': {
          selectedDaughterboardId: 'BOD4-CP',
          status: 'selected',
        },
      },
      updatedAt: '2026-05-02T12:00:00Z',
    });
    expect(layoutStore.layout?.connectorSelections).toHaveProperty('050201020300');
  });

  it('clears dirty state when connector selection round-trips back to saved snapshot', () => {
    layoutStore.newLayout();

    layoutStore.upsertConnectorSelections('05.02.01.02.03.00', {
      carrierKey: 'rr-cirkits::tower-lcc',
      slotSelections: {
        'connector-a': {
          selectedDaughterboardId: 'BOD4-CP',
          status: 'selected',
        },
      },
      updatedAt: '2026-05-02T12:00:00Z',
    });

    expect(layoutStore.isDirty).toBe(true);

    layoutStore.removeConnectorSelections('05.02.01.02.03.00');

    expect(layoutStore.isDirty).toBe(false);
    expect(layoutStore.layout?.connectorSelections).toEqual({});
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// STATE MATRIX: isOfflineMode / hasLayoutFile / isConnected
// ═══════════════════════════════════════════════════════════════════════════════

describe('State matrix: no layout, disconnected', () => {
  it('isOfflineMode is false', () => {
    expect(layoutStore.isOfflineMode).toBe(false);
  });

  it('hasLayoutFile is false', () => {
    expect(layoutStore.hasLayoutFile).toBe(false);
  });

  it('isConnected is false', () => {
    expect(layoutStore.isConnected).toBe(false);
  });

  it('activeContext is null', () => {
    expect(layoutStore.activeContext).toBeNull();
  });
});

describe('State matrix: no layout, connected', () => {
  beforeEach(() => {
    layoutStore.setConnected(true);
  });

  it('isOfflineMode is false', () => {
    expect(layoutStore.isOfflineMode).toBe(false);
  });

  it('hasLayoutFile is false', () => {
    expect(layoutStore.hasLayoutFile).toBe(false);
  });

  it('isConnected is true', () => {
    expect(layoutStore.isConnected).toBe(true);
  });
});

describe('State matrix: layout open, disconnected (pure offline editing)', () => {
  beforeEach(() => {
    setOfflineLayoutContext();
  });

  it('isOfflineMode is true — edits route to offlineChangesStore', () => {
    expect(layoutStore.isOfflineMode).toBe(true);
  });

  it('hasLayoutFile is true', () => {
    expect(layoutStore.hasLayoutFile).toBe(true);
  });

  it('isConnected is false', () => {
    expect(layoutStore.isConnected).toBe(false);
  });
});

describe('State matrix: layout open, connected (online with layout)', () => {
  beforeEach(() => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
  });

  it('isOfflineMode is false — edits route to hardware (modifiedValue)', () => {
    expect(layoutStore.isOfflineMode).toBe(false);
  });

  it('hasLayoutFile is true — layout-dependent features still work', () => {
    expect(layoutStore.hasLayoutFile).toBe(true);
  });

  it('isConnected is true', () => {
    expect(layoutStore.isConnected).toBe(true);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// TRANSITIONS: connection state changes with layout open
// ═══════════════════════════════════════════════════════════════════════════════

describe('Transition: go online with layout open', () => {
  it('switches from offline editing to online editing', () => {
    setOfflineLayoutContext();
    expect(layoutStore.isOfflineMode).toBe(true);

    layoutStore.setConnected(true);
    expect(layoutStore.isOfflineMode).toBe(false);
    expect(layoutStore.hasLayoutFile).toBe(true);
  });
});

describe('Transition: go offline with layout open', () => {
  it('switches back to offline editing mode', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    expect(layoutStore.isOfflineMode).toBe(false);

    layoutStore.setConnected(false);
    expect(layoutStore.isOfflineMode).toBe(true);
    expect(layoutStore.hasLayoutFile).toBe(true);
  });
});

describe('Transition: open layout while connected', () => {
  it('hasLayoutFile becomes true but isOfflineMode stays false', () => {
    layoutStore.setConnected(true);
    expect(layoutStore.hasLayoutFile).toBe(false);

    setOfflineLayoutContext();
    expect(layoutStore.hasLayoutFile).toBe(true);
    expect(layoutStore.isOfflineMode).toBe(false);
  });
});

describe('Transition: close layout while connected', () => {
  it('both hasLayoutFile and isOfflineMode become false', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);

    layoutStore.setActiveContext(null);
    expect(layoutStore.hasLayoutFile).toBe(false);
    expect(layoutStore.isOfflineMode).toBe(false);
  });
});

describe('Transition: close layout while disconnected', () => {
  it('isOfflineMode becomes false when layout is closed', () => {
    setOfflineLayoutContext();
    expect(layoutStore.isOfflineMode).toBe(true);

    layoutStore.setActiveContext(null);
    expect(layoutStore.isOfflineMode).toBe(false);
    expect(layoutStore.hasLayoutFile).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// UI VISIBILITY DECISIONS (driven by computed properties)
// ═══════════════════════════════════════════════════════════════════════════════

describe('UI: connect area visibility', () => {
  // Connect area shows when: !connected && !hasLayoutFile && nodes.length === 0

  it('shows when no layout and disconnected', () => {
    const showConnectArea = !layoutStore.isConnected && !layoutStore.hasLayoutFile;
    expect(showConnectArea).toBe(true);
  });

  it('hides when connected (no layout)', () => {
    layoutStore.setConnected(true);
    const showConnectArea = !layoutStore.isConnected && !layoutStore.hasLayoutFile;
    expect(showConnectArea).toBe(false);
  });

  it('hides when layout is open (disconnected)', () => {
    setOfflineLayoutContext();
    const showConnectArea = !layoutStore.isConnected && !layoutStore.hasLayoutFile;
    expect(showConnectArea).toBe(false);
  });
});

describe('UI: save controls visibility', () => {
  // SaveControls show when: connected || isOfflineMode

  it('hidden when disconnected, no layout', () => {
    const showSaveControls = layoutStore.isConnected || layoutStore.isOfflineMode;
    expect(showSaveControls).toBe(false);
  });

  it('shows when connected, no layout', () => {
    layoutStore.setConnected(true);
    const showSaveControls = layoutStore.isConnected || layoutStore.isOfflineMode;
    expect(showSaveControls).toBe(true);
  });

  it('shows when layout open, disconnected (offline)', () => {
    setOfflineLayoutContext();
    const showSaveControls = layoutStore.isConnected || layoutStore.isOfflineMode;
    expect(showSaveControls).toBe(true);
  });

  it('shows when layout open, connected (online)', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    const showSaveControls = layoutStore.isConnected || layoutStore.isOfflineMode;
    expect(showSaveControls).toBe(true);
  });
});

describe('UI: "Read Config" CTA visibility', () => {
  // showConfigCta requires: !hasLayoutFile (among other conditions)

  it('can show when no layout file', () => {
    expect(layoutStore.hasLayoutFile).toBe(false);
  });

  it('hidden when layout open (offline)', () => {
    setOfflineLayoutContext();
    expect(layoutStore.hasLayoutFile).toBe(true);
  });

  it('hidden when layout open (connected)', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    expect(layoutStore.hasLayoutFile).toBe(true);
  });
});

describe('UI: sync panel triggering', () => {
  // Sync should trigger when: hasLayoutFile && pendingCount > 0 (after discovery settles)

  it('can trigger when layout open and connected', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    expect(layoutStore.hasLayoutFile).toBe(true);
  });

  it('does not trigger without layout file', () => {
    layoutStore.setConnected(true);
    expect(layoutStore.hasLayoutFile).toBe(false);
  });

  it('can trigger after going offline then online again', () => {
    setOfflineLayoutContext();
    expect(layoutStore.hasLayoutFile).toBe(true);

    layoutStore.setConnected(true);
    expect(layoutStore.hasLayoutFile).toBe(true);

    layoutStore.setConnected(false);
    layoutStore.setConnected(true);
    expect(layoutStore.hasLayoutFile).toBe(true);
  });
});

describe('UI: Save Layout menu availability', () => {
  // canSaveLayout requires: offlineActive = activeContext && hasLayoutFile

  it('Save Layout available when layout open offline with edits', () => {
    setOfflineLayoutContext();
    const offlineActive = !!layoutStore.activeContext && layoutStore.hasLayoutFile;
    expect(offlineActive).toBe(true);
  });

  it('Save Layout available when layout open online with edits', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    const offlineActive = !!layoutStore.activeContext && layoutStore.hasLayoutFile;
    expect(offlineActive).toBe(true);
  });

  it('Save Layout not available when no layout open', () => {
    layoutStore.setConnected(true);
    const offlineActive = !!layoutStore.activeContext && layoutStore.hasLayoutFile;
    expect(offlineActive).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// EDIT ROUTING DECISIONS
// ═══════════════════════════════════════════════════════════════════════════════

describe('Edit routing: isOfflineMode drives TreeLeafRow path', () => {
  // TreeLeafRow uses: layoutStore.isOfflineMode || isNodeOffline

  it('routes to offline store when layout open + disconnected', () => {
    setOfflineLayoutContext();
    const routeToOffline = layoutStore.isOfflineMode;
    expect(routeToOffline).toBe(true);
  });

  it('routes to hardware when layout open + connected', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    const routeToOffline = layoutStore.isOfflineMode;
    expect(routeToOffline).toBe(false);
  });

  it('routes to hardware when no layout + connected', () => {
    layoutStore.setConnected(true);
    const routeToOffline = layoutStore.isOfflineMode;
    expect(routeToOffline).toBe(false);
  });
});

describe('Save routing: isOfflineMode drives SaveControls path', () => {
  // SaveControls.handleSave: if (layoutStore.isOfflineMode) → file save, else → hardware write

  it('saves to file when layout open + disconnected', () => {
    setOfflineLayoutContext();
    expect(layoutStore.isOfflineMode).toBe(true);
  });

  it('saves to hardware when layout open + connected', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    expect(layoutStore.isOfflineMode).toBe(false);
  });

  it('saves to hardware when connected without layout', () => {
    layoutStore.setConnected(true);
    expect(layoutStore.isOfflineMode).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// OFFLINE CHANGES FLUSH GUARD
// ═══════════════════════════════════════════════════════════════════════════════

describe('Offline changes flush guard', () => {
  // saveCurrentCaptureToFile: if (layoutStore.isOfflineMode) → flushPendingToBackend

  it('flushes offline changes only when truly offline with layout', () => {
    setOfflineLayoutContext();
    expect(layoutStore.isOfflineMode).toBe(true);
  });

  it('does NOT flush when layout open but connected (edits went to hardware)', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);
    expect(layoutStore.isOfflineMode).toBe(false);
  });
});

// ═══════════════════════════════════════════════════════════════════════════════
// RESET / LIFECYCLE
// ═══════════════════════════════════════════════════════════════════════════════

describe('reset() clears layout state but not connection', () => {
  it('clears layout-related state', () => {
    setOfflineLayoutContext();
    layoutStore.setConnected(true);

    layoutStore.reset();

    expect(layoutStore.hasLayoutFile).toBe(false);
    expect(layoutStore.isOfflineMode).toBe(false);
    expect(layoutStore.activeContext).toBeNull();
  });

  it('preserves connection status across reset', () => {
    layoutStore.setConnected(true);
    layoutStore.reset();
    expect(layoutStore.isConnected).toBe(true);
  });
});

describe('setActiveContext with legacy_file mode', () => {
  it('legacy mode does not enable hasLayoutFile', () => {
    setLegacyLayoutContext();
    expect(layoutStore.hasLayoutFile).toBe(false);
    expect(layoutStore.isOfflineMode).toBe(false);
  });
});
