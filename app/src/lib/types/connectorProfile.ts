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
  allowedValues?: ConnectorConstraintScalar[];
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

export interface DaughterboardView {
  daughterboardId: string;
  displayName: string;
  kind?: string;
  description?: string;
  validityRules?: ConnectorConstraintRuleView[];
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