import { invoke } from '@tauri-apps/api/core';

export type SlotKind = 'producer' | 'consumer';

/// Channel role identifier (open-ended union; backend may add new ones).
export type ChannelRole = 'block-occupancy' | 'lamp-indicator' | string;

export interface SlotDefinition {
  label: string;
  kind: SlotKind;
  requiredRole: ChannelRole;
}

export interface StateMapping {
  producerState: string;
  consumerCommand: string;
}

export interface BehaviorTemplate {
  templateId: string;
  displayName: string;
  slots: SlotDefinition[];
  mapping: StateMapping[];
}

/// Fetch the hardcoded behavior template registry from the backend.
/// Called once at app start by `behaviorTemplatesStore`.
export async function listBehaviorTemplates(): Promise<BehaviorTemplate[]> {
  return invoke<BehaviorTemplate[]>('list_behavior_templates');
}
