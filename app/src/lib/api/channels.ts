import { invoke } from '@tauri-apps/api/core';

/** The kind of information a channel carries. */
export type ChannelType = 'block-occupancy';

/** Identifies the hardware backing a channel. */
export interface HardwareReference {
  nodeKey: string;
  connector: string;
  input: number;
}

/** A single information channel in the layout inventory. */
export interface InformationChannel {
  id: string;
  name: string;
  channelType: ChannelType;
  hardwareRef: HardwareReference;
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
