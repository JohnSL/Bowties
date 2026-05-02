import {
  getConnectorProfile,
} from '$lib/api/connectorProfiles';
import { layoutStore } from '$lib/stores/layout.svelte';
import type {
  LayoutConnectorSelectionRecord,
  LayoutConnectorSelections,
  LayoutNodeHardwareSelectionSet,
} from '$lib/types/bowtie';
import type {
  ConnectorProfileView,
  ConnectorSelection,
  ConnectorSelectionDocument,
  ConnectorSelectionStatus,
} from '$lib/types/connectorProfile';
import { normalizeNodeId } from '$lib/utils/nodeId';

function toLayoutSelectionRecord(selection: ConnectorSelection): LayoutConnectorSelectionRecord {
  return {
    selectedDaughterboardId: selection.selectedDaughterboardId,
    status: selection.status,
  };
}

function toLayoutSelectionSet(document: ConnectorSelectionDocument): LayoutNodeHardwareSelectionSet {
  return {
    carrierKey: document.carrierKey,
    slotSelections: Object.fromEntries(
      document.slotSelections.map((selection) => [selection.slotId, toLayoutSelectionRecord(selection)]),
    ),
    updatedAt: document.updatedAt,
  };
}

function fromLayoutSelectionSet(
  nodeId: string,
  selectionSet: LayoutNodeHardwareSelectionSet,
): ConnectorSelectionDocument {
  return {
    nodeId,
    carrierKey: selectionSet.carrierKey,
    slotSelections: Object.entries(selectionSet.slotSelections)
      .map(([slotId, selection]) => ({
        slotId,
        selectedDaughterboardId: selection.selectedDaughterboardId,
        status: selection.status ?? 'unknown',
      }))
      .sort((left, right) => left.slotId.localeCompare(right.slotId)),
    updatedAt: selectionSet.updatedAt,
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

class ConnectorSelectionsStore {
  private _profiles = $state<Map<string, ConnectorProfileView>>(new Map());
  private _documents = $state<Map<string, ConnectorSelectionDocument>>(new Map());
  private _loading = $state<Set<string>>(new Set());
  private _errors = $state<Map<string, string>>(new Map());

  get profiles(): Map<string, ConnectorProfileView> {
    return this._profiles;
  }

  get documents(): Map<string, ConnectorSelectionDocument> {
    return this._documents;
  }

  get errors(): Map<string, string> {
    return this._errors;
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

  hydrateFromLayout(layout: { connectorSelections: LayoutConnectorSelections } | null): void {
    const nextDocuments = new Map<string, ConnectorSelectionDocument>();

    for (const [nodeId, selectionSet] of Object.entries(layout?.connectorSelections ?? {})) {
      nextDocuments.set(
        normalizeNodeId(nodeId),
        fromLayoutSelectionSet(nodeId, selectionSet),
      );
    }

    this._documents = nextDocuments;
  }

  reset(): void {
    this._profiles = new Map();
    this._documents = new Map();
    this._loading = new Set();
    this._errors = new Map();
  }

  async loadNode(
    nodeId: string,
    profileOverride: ConnectorProfileView | null = null,
  ): Promise<ConnectorSelectionDocument | null> {
    const nodeKey = normalizeNodeId(nodeId);
    if (this._loading.has(nodeKey)) {
      return this._documents.get(nodeKey) ?? null;
    }

    this._loading = new Set([...this._loading, nodeKey]);
    this._errors = new Map(this._errors);
    this._errors.delete(nodeKey);

    try {
      const profile = profileOverride ?? await getConnectorProfile(nodeId);
      const layoutSelectionSet = layoutStore.getConnectorSelections(nodeId);
      const layoutDocument = layoutSelectionSet
        ? fromLayoutSelectionSet(nodeId, layoutSelectionSet)
        : null;
      const existingDocument = this._documents.get(nodeKey) ?? null;
      const document = profile
        ? reconcileDocumentWithProfile(profile, existingDocument ?? layoutDocument ?? createDefaultDocument(profile))
        : null;

      this._profiles = new Map(this._profiles);
      if (profile) {
        this._profiles.set(nodeKey, profile);
      } else {
        this._profiles.delete(nodeKey);
      }

      this._documents = new Map(this._documents);
      if (document) {
        this._documents.set(nodeKey, document);
      } else {
        this._documents.delete(nodeKey);
      }

      return document;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this._errors = new Map(this._errors);
      this._errors.set(nodeKey, message);
      return null;
    } finally {
      this._loading = new Set(this._loading);
      this._loading.delete(nodeKey);
    }
  }

  async saveDocument(document: ConnectorSelectionDocument): Promise<ConnectorSelectionDocument> {
    const saved: ConnectorSelectionDocument = {
      ...document,
      updatedAt: document.updatedAt ?? new Date().toISOString(),
    };
    const nodeKey = normalizeNodeId(saved.nodeId);

    this._documents = new Map(this._documents);
    this._documents.set(nodeKey, saved);
    layoutStore.upsertConnectorSelections(saved.nodeId, toLayoutSelectionSet(saved));
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