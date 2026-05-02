import type {
  ConnectorProfileView,
  ConnectorSelectionDocument,
  ConnectorSlotView,
} from '$lib/types/connectorProfile';

export interface ConnectorSlotOption {
  description: string | null;
  label: string;
  value: string;
}

export interface ConnectorSlotSelectorViewModel {
  slotId: string;
  label: string;
  selectedDaughterboardId: string | null;
  options: ConnectorSlotOption[];
}

export function buildConnectorSlotSelectors(
  profile: ConnectorProfileView | null,
  document: ConnectorSelectionDocument | null,
): ConnectorSlotSelectorViewModel[] {
  if (!profile) {
    return [];
  }

  const descriptions = new Map(
    (profile.supportedDaughterboards ?? []).map((daughterboard) => [
      daughterboard.daughterboardId,
      daughterboard,
    ]),
  );
  const selectedBySlot = new Map(
    (document?.slotSelections ?? []).map((selection) => [
      selection.slotId,
      selection.selectedDaughterboardId ?? null,
    ]),
  );

  return [...profile.slots]
    .sort((left, right) => left.order - right.order)
    .map((slot) => ({
      slotId: slot.slotId,
      label: slot.label,
      selectedDaughterboardId: selectedBySlot.get(slot.slotId) ?? null,
      options: [
        ...(slot.allowNoneInstalled
          ? [{ value: '', label: 'None installed', description: null }]
          : []),
        ...slot.supportedDaughterboardIds
          .map((daughterboardId) => {
            const daughterboard = descriptions.get(daughterboardId);
            return {
              value: daughterboardId,
              label: daughterboard?.displayName ?? daughterboardId,
              description: daughterboard?.description ?? null,
            };
          })
          .sort((left, right) => left.label.localeCompare(right.label, undefined, { sensitivity: 'base' })),
      ],
    }));
}

export function buildSegmentConnectorSlotSelectors(
  profile: ConnectorProfileView | null,
  document: ConnectorSelectionDocument | null,
  segmentName: string,
): ConnectorSlotSelectorViewModel[] {
  if (!profile) {
    return [];
  }

  const filteredProfile: ConnectorProfileView = {
    ...profile,
    slots: profile.slots.filter((slot) => slotAppliesToSegment(slot, segmentName)),
  };

  return buildConnectorSlotSelectors(filteredProfile, document);
}

function slotAppliesToSegment(slot: ConnectorSlotView, segmentName: string): boolean {
  return slot.affectedPaths.some((path) => (
    path === segmentName || path.startsWith(`${segmentName}/`)
  ));
}