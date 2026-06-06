import {
  getConnectorProfile,
} from '$lib/api/connectorProfiles';
import { setNodeModeSelection } from '$lib/api/layout';
import { layoutStore } from '$lib/stores/layout.svelte';
import type { LayoutFile } from '$lib/types/bowtie';
import type {
  ConnectorProfileView,
  ConnectorSelectionDocument,
  ConnectorSelectionStatus,
} from '$lib/types/connectorProfile';
import { normalizeNodeId } from '$lib/utils/nodeId';

/**
 * Build a ConnectorSelectionDocument from the unified
 * `nodeModeSelections[nodeKey]` map (Spec 014 / S6).
 *
 * The Tower-LCC profile uses an identity mapping: each connector slot id
 * (e.g. `connector-a`) is also a Configuration Mode id, and each
 * daughterboard id (e.g. `BOD4-CP`) is its variant id. So the per-node
 * selection map can be projected back onto the slot list directly.
 */
function fromNodeModeSelections(
  nodeId: string,
  profile: ConnectorProfileView,
  selections: Record<string, string>,
): ConnectorSelectionDocument {
  return {
    nodeId,
    carrierKey: profile.carrierKey,
    slotSelections: [...profile.slots]
      .sort((left, right) => left.order - right.order)
      .map((slot) => {
        const variantId = selections[slot.slotId];
        return {
          slotId: slot.slotId,
          selectedDaughterboardId: variantId,
          status: variantId
            ? (slot.supportedDaughterboardIds.includes(variantId)
                ? ('selected' satisfies ConnectorSelectionStatus)
                : ('unknown' satisfies ConnectorSelectionStatus))
            : ('none' satisfies ConnectorSelectionStatus),
        };
      }),
    updatedAt: undefined,
  };
}

function createDefaultDocument(profile: ConnectorProfileView): ConnectorSelectionDocument {
  return {
    nodeId: profile.nodeId,
    carrierKey: profile.carrierKey,
    slotSelections: [...profile.slots]
      .sort((left, right) => left.order - right.order)
      .map((slot) => ({
        slotId: slot.slotId,
        selectedDaughterboardId: undefined,
        status: 'none' satisfies ConnectorSelectionStatus,
      })),
    updatedAt: undefined,
  };
}

function reconcileDocumentWithProfile(
  profile: ConnectorProfileView,
  document: ConnectorSelectionDocument | null,
): ConnectorSelectionDocument {
  const selectionsBySlot = new Map(
    (document?.slotSelections ?? []).map((selection) => [selection.slotId, selection]),
  );

  return {
    nodeId: profile.nodeId,
    carrierKey: profile.carrierKey,
    slotSelections: [...profile.slots]
      .sort((left, right) => left.order - right.order)
      .map((slot) => {
        const existing = selectionsBySlot.get(slot.slotId);
        const selectedDaughterboardId = existing?.selectedDaughterboardId;
        return {
          slotId: slot.slotId,
          selectedDaughterboardId,
          status: existing?.status ?? (selectedDaughterboardId ? 'selected' : 'none'),
        };
      }),
    updatedAt: document?.updatedAt,
  };
}

function deriveUnknownSelectionWarnings(
  profile: ConnectorProfileView | null,
  document: ConnectorSelectionDocument | null,
): string[] {
  if (!profile || !document) {
    return [];
  }

  const slotMap = new Map(profile.slots.map((slot) => [slot.slotId, slot]));
  return document.slotSelections.flatMap((selection) => {
    if (selection.status !== 'unknown' && selection.status !== 'selected') {
      return [];
    }

    const slot = slotMap.get(selection.slotId);
    if (!slot) {
      return [];
    }

    const selectedId = selection.selectedDaughterboardId;
    if (!selectedId) {
      return [];
    }

    if (selection.status === 'selected' && slot.supportedDaughterboardIds.includes(selectedId)) {
      return [];
    }

    return [`${slot.label} preserves unknown daughterboard "${selectedId}" from saved layout metadata.`];
  });
}

class ConnectorSelectionsStore {
  private _profiles = $state<Map<string, ConnectorProfileView>>(new Map());
  private _documents = $state<Map<string, ConnectorSelectionDocument>>(new Map());
  private _loading = $state<Set<string>>(new Set());
  private _errors = $state<Map<string, string>>(new Map());
  private _previewWarnings = $state<Map<string, string[]>>(new Map());
  revision = $state(0);

  get profiles(): Map<string, ConnectorProfileView> {
    return this._profiles;
  }

  get documents(): Map<string, ConnectorSelectionDocument> {
    return this._documents;
  }

  get errors(): Map<string, string> {
    return this._errors;
  }

  get totalWarningCount(): number {
    let total = 0;
    for (const nodeId of new Set([...this._profiles.keys(), ...this._previewWarnings.keys()])) {
      total += this.getWarnings(nodeId).length;
    }
    return total;
  }

  getWarnings(nodeId: string): string[] {
    const nodeKey = normalizeNodeId(nodeId);
    const previewWarnings = this._previewWarnings.get(nodeKey) ?? [];
    const unknownWarnings = deriveUnknownSelectionWarnings(
      this._profiles.get(nodeKey) ?? null,
      this._documents.get(nodeKey) ?? null,
    );
    return [...unknownWarnings, ...previewWarnings];
  }

  getProfile(nodeId: string): ConnectorProfileView | null {
    return this._profiles.get(normalizeNodeId(nodeId)) ?? null;
  }

  getDocument(nodeId: string): ConnectorSelectionDocument | null {
    return this._documents.get(normalizeNodeId(nodeId)) ?? null;
  }

  isLoading(nodeId: string): boolean {
    return this._loading.has(normalizeNodeId(nodeId));
  }

  getError(nodeId: string): string | null {
    return this._errors.get(normalizeNodeId(nodeId)) ?? null;
  }

  /**
   * Reset cached documents on layout open/close (Spec 014 / S6).
   *
   * Documents are no longer projected up-front from the layout file —
   * they're built lazily by `loadNode()` when a profile is also available
   * (the identity mapping needs the slot list to know which mode ids to
   * read out of `nodeModeSelections`). This hook still exists so the
   * orchestrator can clear cross-layout state on open.
   */
  hydrateFromLayout(_layout: LayoutFile | null): void {
    this._documents = new Map();
    this._previewWarnings = new Map();
    this.revision += 1;
  }

  reset(): void {
    this._profiles = new Map();
    this._documents = new Map();
    this._loading = new Set();
    this._errors = new Map();
    this._previewWarnings = new Map();
    this.revision += 1;
  }

  setCompatibilityWarnings(nodeId: string, warnings: string[]): void {
    const nodeKey = normalizeNodeId(nodeId);

    const nextWarnings = new Map(this._previewWarnings);
    if (warnings.length > 0) {
      nextWarnings.set(nodeKey, [...warnings]);
    } else {
      nextWarnings.delete(nodeKey);
    }

    this._previewWarnings = nextWarnings;
    this.revision += 1;
  }

  private clearNodeState(nodeKey: string): void {
    const nextProfiles = new Map(this._profiles);
    nextProfiles.delete(nodeKey);
    this._profiles = nextProfiles;

    const nextDocuments = new Map(this._documents);
    nextDocuments.delete(nodeKey);
    this._documents = nextDocuments;

    const nextWarnings = new Map(this._previewWarnings);
    nextWarnings.delete(nodeKey);
    this._previewWarnings = nextWarnings;
  }

  async loadNode(
    nodeId: string,
    profileOverride: ConnectorProfileView | null | undefined = undefined,
  ): Promise<ConnectorSelectionDocument | null> {
    const nodeKey = normalizeNodeId(nodeId);
    if (this._loading.has(nodeKey)) {
      return this._documents.get(nodeKey) ?? null;
    }

    const nextLoading = new Set(this._loading);
    nextLoading.add(nodeKey);
    this._loading = nextLoading;

    const nextErrors = new Map(this._errors);
    nextErrors.delete(nodeKey);
    this._errors = nextErrors;

    try {
      const profile = profileOverride !== undefined
        ? profileOverride
        : await getConnectorProfile(nodeId);
      const nodeModeSelections = layoutStore.getNodeModeSelections(nodeId);
      const layoutDocument = profile && nodeModeSelections
        ? fromNodeModeSelections(nodeId, profile, nodeModeSelections)
        : null;
      const existingDocument = this._documents.get(nodeKey) ?? null;
      const document = profile
        ? reconcileDocumentWithProfile(profile, existingDocument ?? layoutDocument ?? createDefaultDocument(profile))
        : null;

      if (profile && document) {
        const nextProfiles = new Map(this._profiles);
        nextProfiles.set(nodeKey, profile);
        this._profiles = nextProfiles;

        const nextDocuments = new Map(this._documents);
        nextDocuments.set(nodeKey, document);
        this._documents = nextDocuments;
      } else {
        this.clearNodeState(nodeKey);
      }
      this.revision += 1;

      return document;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const failedErrors = new Map(this._errors);
      failedErrors.set(nodeKey, message);
      this._errors = failedErrors;
      this.revision += 1;
      return null;
    } finally {
      const settledLoading = new Set(this._loading);
      settledLoading.delete(nodeKey);
      this._loading = settledLoading;
    }
  }

  /**
   * Persist a connector selection document by emitting one
   * `set_node_mode_selection` IPC per slot that has a daughterboard set
   * (Spec 014 / S6). The Tower-LCC profile uses an identity mapping so
   * `slotId` is also `modeId` and `selectedDaughterboardId` is also
   * `variantId`.
   *
   * Note (deselection): there is no backend "clear selection" delta yet —
   * a slot whose `selectedDaughterboardId` is undefined is preserved in
   * the in-memory document but not pushed to the backend. Clearing must
   * be handled by a future Configuration Mode delta variant.
   */
  async saveDocument(document: ConnectorSelectionDocument): Promise<ConnectorSelectionDocument> {
    const saved: ConnectorSelectionDocument = {
      ...document,
      updatedAt: document.updatedAt ?? new Date().toISOString(),
    };
    const nodeKey = normalizeNodeId(saved.nodeId);

    const nextDocuments = new Map(this._documents);
    nextDocuments.set(nodeKey, saved);
    this._documents = nextDocuments;
    this.revision += 1;

    for (const selection of saved.slotSelections) {
      if (!selection.selectedDaughterboardId) {
        continue;
      }
      try {
        await setNodeModeSelection(
          nodeKey,
          selection.slotId,
          selection.selectedDaughterboardId,
        );
      } catch (error) {
        // Save failures are surfaced as console warnings rather than the
        // node-level error channel (which represents "can't load this
        // connector" — a different concern). The in-memory document is
        // already updated, so the UI continues to reflect user intent.
        const message = error instanceof Error ? error.message : String(error);
        console.warn(
          `[connectorSelections] set_node_mode_selection failed for ${nodeKey}/${selection.slotId}: ${message}`,
        );
      }
    }

    return saved;
  }

  async updateSlotSelection(
    nodeId: string,
    slotId: string,
    selectedDaughterboardId: string | null,
  ): Promise<ConnectorSelectionDocument | null> {
    const current = this.getDocument(nodeId) ?? await this.loadNode(nodeId);
    if (!current) {
      return null;
    }

    const nextStatus: ConnectorSelectionStatus = selectedDaughterboardId ? 'selected' : 'none';
    const nextDocument: ConnectorSelectionDocument = {
      ...current,
      slotSelections: current.slotSelections.map((selection) => {
        if (selection.slotId !== slotId) {
          return selection;
        }

        return {
          ...selection,
          selectedDaughterboardId: selectedDaughterboardId ?? undefined,
          status: nextStatus,
        };
      }),
      updatedAt: undefined,
    };

    return this.saveDocument(nextDocument);
  }
}

export const connectorSelectionsStore = new ConnectorSelectionsStore();