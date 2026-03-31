/**
 * Alias → NodeID resolver.
 *
 * Builds a best-effort map from 12-bit CAN alias to 6-byte NodeID by scanning
 * a parsed trace for two evidence sources (in priority order):
 *
 *  1. Raw boot frames present in the trace window:
 *       - Initialization Complete (MTI 0x19100): data bytes 0-5 are the NodeID
 *       - Verified Node ID        (MTI 0x19170): data bytes 0-5 are the NodeID
 *     These give confirmed mappings.
 *
 *  2. JMRI decoded text column: JMRI normally resolves aliases and writes
 *     "SRC_NODEID - DEST_NODEID MessageType" in the decoded text.
 *     We pair the NodeID at position 0 with srcAlias from the header, and
 *     position 1 (if present) with the dest alias.
 *     These give inferred mappings (JMRI may be wrong, but usually is not).
 *
 * Because JMRI records are present on nearly every addressed frame, inferred
 * mappings cover 99 % of traces even when boot frames are absent.
 */

import type { ParsedFrame } from "./trace-parser.js";

// ─── Types ──────────────────────────────────────────────────────────────────

export type AliasConfidence = "confirmed" | "inferred";

export interface AliasEntry {
  nodeId: string;          // "XX.XX.XX.XX.XX.XX"
  confidence: AliasConfidence;
}

export type AliasMap = Map<number, AliasEntry>;

// MTI values for boot frames
const MTI_INIT_COMPLETE = 0x19100;
const MTI_INIT_SIMPLE   = 0x19101;
const MTI_VERIFIED_NODE = 0x19170;
const MTI_VERIFIED_SIMPLE = 0x19171;

// ─── Helpers ────────────────────────────────────────────────────────────────

function bytesToNodeId(bytes: number[]): string | null {
  if (bytes.length < 6) return null;
  return bytes.slice(0, 6)
    .map(b => b.toString(16).toUpperCase().padStart(2, "0"))
    .join(".");
}

function setIfBetter(map: AliasMap, alias: number, nodeId: string, confidence: AliasConfidence): void {
  const existing = map.get(alias);
  // confirmed > inferred; don't overwrite confirmed with inferred
  if (!existing || confidence === "confirmed" || existing.confidence === "inferred") {
    map.set(alias, { nodeId, confidence });
  }
}

// ─── Main ─────────────────────────────────────────────────────────────────

/**
 * Build an alias → NodeID map from a list of already-parsed frames.
 * Pass in the full trace frames array.
 */
export function buildAliasMap(frames: ParsedFrame[]): AliasMap {
  const map: AliasMap = new Map();

  for (const frame of frames) {
    const { decoded, rawBytes, jmriNodeIds } = frame;
    const { srcAlias, mtiValue, destAlias } = decoded;

    // ── Source 1: boot frames ──
    if (
      mtiValue === MTI_INIT_COMPLETE ||
      mtiValue === MTI_INIT_SIMPLE   ||
      mtiValue === MTI_VERIFIED_NODE ||
      mtiValue === MTI_VERIFIED_SIMPLE
    ) {
      const nodeId = bytesToNodeId(rawBytes);
      if (nodeId) setIfBetter(map, srcAlias, nodeId, "confirmed");
    }

    // ── Source 2: JMRI decoded text ──
    // jmriNodeIds[0] is the src node, jmriNodeIds[1] (if present) is the dest
    if (jmriNodeIds.length > 0) {
      setIfBetter(map, srcAlias, jmriNodeIds[0], "inferred");
    }
    if (jmriNodeIds.length > 1) {
      // Determine dest alias
      const dAlias = destAlias ?? frame.addrDestAlias;
      if (dAlias !== undefined) {
        setIfBetter(map, dAlias, jmriNodeIds[1], "inferred");
      }
    }
  }

  return map;
}

/**
 * Resolve an alias to a human-readable string: "NodeID (confidence)".
 * Falls back to "alias:0xXXX" if not found.
 */
export function resolveAlias(alias: number, map: AliasMap): string {
  const entry = map.get(alias);
  if (!entry) return `alias:0x${alias.toString(16).toUpperCase().padStart(3, "0")}`;
  const tag = entry.confidence === "confirmed" ? "" : " (inferred)";
  return `${entry.nodeId}${tag}`;
}

/**
 * Serialise the alias map to a plain object suitable for JSON / MCP responses.
 */
export function aliasMapToObject(
  map: AliasMap
): Record<string, { nodeId: string; confidence: AliasConfidence }> {
  const out: Record<string, { nodeId: string; confidence: AliasConfidence }> = {};
  for (const [alias, entry] of map.entries()) {
    out[`0x${alias.toString(16).toUpperCase().padStart(3, "0")}`] = entry;
  }
  return out;
}
