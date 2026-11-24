/**
 * Tauri IPC wrappers for configuration write operations.
 *
 * Per contracts/tauri-ipc.md TypeScript signatures — spec 007.
 */

import { invoke } from '@tauri-apps/api/core';
import type { WriteResult } from '$lib/types/nodeTree';
import { toCanonicalNodeKey, type NodeKeyInput } from '$lib/utils/nodeKey';

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
  nodeId: NodeKeyInput,
  address: number,
  space: number,
  data: number[],
): Promise<WriteResult> {
  return await invoke<WriteResult>('write_config_value', {
    nodeId: toCanonicalNodeKey(nodeId),
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
export async function sendUpdateComplete(nodeId: NodeKeyInput): Promise<void> {
  return await invoke<void>('send_update_complete', { nodeId: toCanonicalNodeKey(nodeId) });
}

/**
 * Trigger an action element: write `value` to the node's memory at the
 * given space/address. This is a fire-once write that bypasses the
 * Save / Pending Change pipeline.
 *
 * @param nodeId   Node ID in dotted-hex.
 * @param space    Address space byte.
 * @param address  Absolute memory address.
 * @param size     Size in bytes (1, 2, 4, or 8).
 * @param value    Integer value to write.
 */
export async function triggerAction(
  nodeId: NodeKeyInput,
  space: number,
  address: number,
  size: number,
  value: number,
): Promise<void> {
  return await invoke<void>('trigger_action', { nodeId: toCanonicalNodeKey(nodeId), space, address, size, value });
}
