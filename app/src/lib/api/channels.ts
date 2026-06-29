import { invoke } from '@tauri-apps/api/core';

/**
 * Spec 018 / S2 (ADR-0013) channel schema. Replaces the Spec 015
 * `{channelType, hardwareRef}` shape with role / style / ownership /
 * binding. No backward-compat: pre-018 layouts fail to load (FR-009).
 */

/** The state-vocabulary contract a facility slot binds by. */
export type ChannelRole = 'block-occupancy' | 'lamp-indicator';

/** Lifecycle classification — who creates and destroys this channel. */
export type ChannelOwnership = 'hardware-owned' | 'user-owned';

/** Discriminated union of binding shapes; `kind` MUST match the style. */
export type ChannelBinding =
  | { kind: 'connectorInput'; nodeKey: string; connector: string; input: number }
  | { kind: 'lampRow'; nodeKey: string; rowOrdinal: number };

/** A single information channel in the layout inventory. */
export interface InformationChannel {
  id: string;
  name: string;
  role: ChannelRole;
  /** Style id; looked up in the profile YAML style catalog. */
  style: string;
  ownership: ChannelOwnership;
  binding: ChannelBinding;
}

/** Fetch the channel inventory for the active layout. */
export async function listChannels(): Promise<InformationChannel[]> {
  return invoke<InformationChannel[]>('list_channels');
}

/** Append new channels to the active layout's channel inventory. */
export async function createChannels(channels: InformationChannel[]): Promise<InformationChannel[]> {
  return invoke<InformationChannel[]>('create_channels', { channels });
}

/** Rename a single channel by ID. Persists immediately to channels.yaml. */
export async function renameChannel(id: string, newName: string): Promise<void> {
  return invoke<void>('rename_channel', { id, newName });
}

/** Delete channels by their IDs from the active layout's channel inventory. */
export async function deleteChannels(ids: string[]): Promise<void> {
  return invoke<void>('delete_channels', { ids });
}
