/**
 * Tauri IPC wrappers for configuration write operations.
 *
 * Per contracts/tauri-ipc.md TypeScript signatures — spec 007.
 */

import { invoke } from '@tauri-apps/api/core';
import type { WriteResult, TreeConfigValue } from '$lib/types/nodeTree';

/**
 * Write raw bytes to a node's configuration memory.
 *
 * Corresponds to the `write_config_value` Tauri command.
 *
 * @param nodeId   Node ID in dotted-hex (e.g. `"05.01.01.01.03.00"`).
 * @param address  Absolute memory address within the given address space.
 * @param space    Address space byte (e.g. `0xFD` = Configuration).
 * @param data     Raw bytes to write (1–64 bytes per call; chunking handled
 *                 automatically by the Rust backend for larger arrays).
 * @returns        `WriteResult` with success/error detail and retry count.
 */
export async function writeConfigValue(
  nodeId: string,
  address: number,
  space: number,
  data: number[],
): Promise<WriteResult> {
  return await invoke<WriteResult>('write_config_value', {
    nodeId,
    address,
    space,
    data,
  });
}

/**
 * Send an Update Complete datagram to a node.
 *
 * Call this after all writes for a save batch are finished to signal
 * the node to reload its configuration from memory.
 *
 * Corresponds to the `send_update_complete` Tauri command.
 *
 * @param nodeId  Node ID in dotted-hex.
 */
export async function sendUpdateComplete(nodeId: string): Promise<void> {
  return await invoke<void>('send_update_complete', { nodeId });
}

// ── Modified value commands ──────────────────────────────────────────────────

/**
 * Set a modified (pending) value on a leaf in the Rust-side tree.
 *
 * If the new value matches the committed value, the modification is
 * automatically cleared (revert). Emits `node-tree-updated`.
 */
export async function setModifiedValue(
  nodeId: string,
  address: number,
  space: number,
  value: TreeConfigValue,
): Promise<boolean> {
  return await invoke<boolean>('set_modified_value', {
    nodeId,
    address,
    space,
    value,
  });
}

/** Result of writing all modified values. */
export interface WriteModifiedResult {
  total: number;
  succeeded: number;
  failed: number;
}

/**
 * Write all pending modifications across all loaded trees to their nodes.
 *
 * Handles marking write state, sending Update Complete, and committing
 * values on success. Emits `node-tree-updated` for affected nodes.
 */
export async function writeModifiedValues(): Promise<WriteModifiedResult> {
  return await invoke<WriteModifiedResult>('write_modified_values');
}

/**
 * Discard all pending modifications, reverting to committed values.
 *
 * @param nodeId If provided, discard only for that node. Otherwise all nodes.
 * @returns Number of nodes affected.
 */
export async function discardModifiedValues(nodeId?: string): Promise<number> {
  return await invoke<number>('discard_modified_values', { nodeId: nodeId ?? null });
}

/**
 * Check whether any loaded tree has pending modifications.
 */
export async function hasModifiedValues(): Promise<boolean> {
  return await invoke<boolean>('has_modified_values');
}

/**
 * Trigger an action element: write `value` to the node's memory at the
 * given space/address. This is a fire-once write that bypasses the
 * modified-value pipeline.
 *
 * @param nodeId   Node ID in dotted-hex.
 * @param space    Address space byte.
 * @param address  Absolute memory address.
 * @param size     Size in bytes (1, 2, 4, or 8).
 * @param value    Integer value to write.
 */
export async function triggerAction(
  nodeId: string,
  space: number,
  address: number,
  size: number,
  value: number,
): Promise<void> {
  return await invoke<void>('trigger_action', { nodeId, space, address, size, value });
}
