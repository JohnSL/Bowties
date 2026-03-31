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
export type AliasConfidence = "confirmed" | "inferred";
export interface AliasEntry {
    nodeId: string;
    confidence: AliasConfidence;
}
export type AliasMap = Map<number, AliasEntry>;
/**
 * Build an alias → NodeID map from a list of already-parsed frames.
 * Pass in the full trace frames array.
 */
export declare function buildAliasMap(frames: ParsedFrame[]): AliasMap;
/**
 * Resolve an alias to a human-readable string: "NodeID (confidence)".
 * Falls back to "alias:0xXXX" if not found.
 */
export declare function resolveAlias(alias: number, map: AliasMap): string;
/**
 * Serialise the alias map to a plain object suitable for JSON / MCP responses.
 */
export declare function aliasMapToObject(map: AliasMap): Record<string, {
    nodeId: string;
    confidence: AliasConfidence;
}>;
//# sourceMappingURL=alias-resolver.d.ts.map