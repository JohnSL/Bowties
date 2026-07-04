export type ConnectorSelectionStatus = 'selected' | 'none' | 'unknown';

export type ConnectorConstraintScalar = string | number;

export type ConnectorConstraintEffect =
  | 'show'
  | 'hide'
  | 'disable'
  | 'allowValues'
  | 'denyValues'
  | 'readOnly';

export type EmptyConnectorEffect = 'hide' | 'disable' | 'allowValues';

export interface EmptyConnectorBehaviorView {
  effect: EmptyConnectorEffect;
  allowedValues?: ConnectorConstraintScalar[];
}

export interface ConnectorConstraintRuleView {
  targetPath: string;
  resolvedPath: string[];
  effect: ConnectorConstraintEffect;
  lineOrdinals?: number[];
  replicationOrdinals?: number[];
  allowedValues?: ConnectorConstraintScalar[];
  allowedValueLabels?: string[];
  deniedValues?: ConnectorConstraintScalar[];
  explanation?: string;
}

export interface SlotSupportedDaughterboardView {
  daughterboardId: string;
  validityRules?: ConnectorConstraintRuleView[];
}

export interface ConnectorSlotView {
  slotId: string;
  label: string;
  order: number;
  allowNoneInstalled: boolean;
  supportedDaughterboardIds: string[];
  affectedPaths: string[];
  resolvedAffectedPaths?: string[][];
  baseBehaviorWhenEmpty?: EmptyConnectorBehaviorView;
  supportedDaughterboardConstraints?: SlotSupportedDaughterboardView[];
}

export interface EventMappingEntry {
  /**
   * Producer-side leaf ordinal under the resolved binding path prefix. Set by
   * styles whose role is producer-flavoured (e.g. `bod-block-detector-input`).
   * Mutually exclusive with `consumerLeafIndex`.
   */
  producerLeafIndex?: number;
  /**
   * Consumer-side leaf ordinal under the resolved binding path prefix. Set by
   * styles whose role is consumer-flavoured (e.g. `single-led-direct-lamp`).
   * Mutually exclusive with `producerLeafIndex`.
   */
  consumerLeafIndex?: number;
}

export interface ChannelInputMapping {
  channelType: string;
  /**
   * Spec 018 / S2 (ADR-0013): style id used to populate the channel's
   * `style` field at auto-create time. Required for any subsystem whose
   * channels are auto-created; optional in the type for older fixtures.
   */
  style?: string;
  inputs: number[];
  eventMapping?: Record<string, EventMappingEntry>;
}

export interface DaughterboardView {
  daughterboardId: string;
  displayName: string;
  kind?: string;
  description?: string;
  validityRules?: ConnectorConstraintRuleView[];
  channelInputs?: ChannelInputMapping[];
}

export interface ConnectorProfileView {
  nodeId: string;
  carrierKey: string;
  slots: ConnectorSlotView[];
  supportedDaughterboards?: DaughterboardView[];
}

export interface ConnectorSelection {
  slotId: string;
  selectedDaughterboardId?: string;
  status: ConnectorSelectionStatus;
}

export interface ConnectorSelectionDocument {
  nodeId: string;
  carrierKey: string;
  slotSelections: ConnectorSelection[];
  updatedAt?: string;
}

export interface CompatibilityPreviewRequest {
  nodeId: string;
  changedSlotId: string;
  slotSelections: ConnectorSelection[];
}

export interface FilteredTarget {
  targetPath: string;
  effect: ConnectorConstraintEffect;
  allowedValues?: string[];
}

export interface StagedRepair {
  targetPath: string;
  space?: number;
  offset?: string;
  baselineValue: string;
  plannedValue: string;
  reason: string;
  originSlotId: string;
}

export interface CompatibilityPreviewResponse {
  nodeId: string;
  changedSlotId: string;
  filteredTargets: FilteredTarget[];
  stagedRepairs: StagedRepair[];
  warnings: string[];
}