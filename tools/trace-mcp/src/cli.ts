#!/usr/bin/env node
/**
 * CLI wrapper around the LCC trace analysis tools.
 *
 * Usage:
 *   node dist/cli.js <trace-file> [options]
 *   node dist/cli.js --clipboard   [options]
 *
 * Options:
 *   --summary          Print node inventory and message counts
 *   --memory-ops       Print all memory config read/write operations
 *   --datagrams        Print reassembled datagram payloads
 *   --timeline         Print chronological message timeline
 *   --node <id>        Filter --timeline to a NodeID or alias (e.g. 0x646)
 *   --space <hex>      Filter --memory-ops to an address space (e.g. 0xFD)
 */

import { readFile } from "node:fs/promises";
import clipboard from "clipboardy";
import { parseTrace } from "./trace-parser.js";
import { buildAliasMap, resolveAlias, aliasMapToObject } from "./alias-resolver.js";
import { reassembleDatagrams } from "./datagram-reassembler.js";
import { decodeDatagramPayload } from "./payload-decoder.js";

// ─── Args ────────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);

function flag(name: string): boolean {
  return args.includes(name);
}
function option(name: string): string | undefined {
  const idx = args.indexOf(name);
  return idx >= 0 ? args[idx + 1] : undefined;
}

const useClipboard = flag("--clipboard");
const filePath     = args.find(a => !a.startsWith("-"));
const showSummary  = flag("--summary");
const showMemOps   = flag("--memory-ops");
const showDgs      = flag("--datagrams");
const showTimeline = flag("--timeline");
const nodeFilter   = option("--node");
const spaceFilter  = option("--space");
// Default: show everything if no mode flag given
const showAll = !showSummary && !showMemOps && !showDgs && !showTimeline;

async function main(): Promise<void> {
  let trace: string;

  if (useClipboard) {
    trace = await clipboard.read();
    console.log("# Read trace from clipboard\n");
  } else if (filePath) {
    trace = await readFile(filePath, "utf-8");
    console.log(`# Trace file: ${filePath}\n`);
  } else {
    process.stderr.write("Usage: lcc-trace <file> [options]\n       lcc-trace --clipboard [options]\n");
    process.exit(1);
    return; // TypeScript flow narrowing
  }

  const { frames, unparsedLines } = parseTrace(trace);
  const aliasMap = buildAliasMap(frames);

  console.log(`Parsed ${frames.length} frames, ${unparsedLines.length} unparsed lines.\n`);

  // Summary
  if (showSummary || showAll) {
    console.log("## Node Inventory\n");
    const nodes = aliasMapToObject(aliasMap);
    for (const [alias, entry] of Object.entries(nodes)) {
      console.log(`  ${alias}  ${entry.nodeId}  [${entry.confidence}]`);
    }
    console.log();
  }

  // Memory operations
  if (showMemOps || showAll) {
    console.log("## Memory Configuration Operations\n");
    const filterSpace = spaceFilter ? parseInt(spaceFilter.replace(/^0x/i, ""), 16) : undefined;
    const datagrams = reassembleDatagrams(frames);

    let count = 0;
    for (const dg of datagrams) {
      if (!dg.bytes.length || dg.bytes[0] !== 0x20) continue;
      const decoded = decodeDatagramPayload(dg.bytes);
      if (decoded.protocol !== "Memory Configuration") continue;
      if (filterSpace !== undefined) {
        const sp = parseInt(String(decoded.fields.addressSpace ?? "0").replace(/^0x/i, ""), 16);
        if (sp !== filterSpace) continue;
      }
      const src = resolveAlias(dg.srcAlias, aliasMap);
      const dst = resolveAlias(dg.destAlias, aliasMap);
      const completeMark = dg.complete ? "" : " [PARTIAL]";
      console.log(`  [${dg.timestamps[0]}]${completeMark} ${src} → ${dst}`);
      console.log(`    ${decoded.summary}`);
      count++;
    }
    if (count === 0) console.log("  (none found)");
    console.log();
  }

  // Datagrams
  if (showDgs) {
    console.log("## Reassembled Datagrams\n");
    const datagrams = reassembleDatagrams(frames);
    for (const dg of datagrams) {
      const src = resolveAlias(dg.srcAlias, aliasMap);
      const dst = resolveAlias(dg.destAlias, aliasMap);
      const completeMark = dg.complete ? "complete" : "PARTIAL";
      console.log(`  [${dg.timestamps[0]}] (${completeMark}) ${src} → ${dst}`);
      const decoded = decodeDatagramPayload(dg.bytes);
      console.log(`    ${decoded.summary}`);
    }
    console.log();
  }

  // Timeline
  if (showTimeline || showAll) {
    console.log("## Message Timeline\n");

    let filterAlias: number | undefined;
    if (nodeFilter) {
      const asAlias = parseInt(nodeFilter.replace(/^0x/i, ""), 16);
      if (!isNaN(asAlias)) {
        filterAlias = asAlias;
      } else {
        const upper = nodeFilter.toUpperCase();
        for (const [alias, entry] of aliasMap) {
          if (entry.nodeId === upper) { filterAlias = alias; break; }
        }
      }
    }

    for (const f of frames) {
      if (
        filterAlias !== undefined &&
        f.decoded.srcAlias !== filterAlias &&
        f.decoded.destAlias !== filterAlias &&
        f.addrDestAlias !== filterAlias
      ) {
        continue;
      }
      const mtiName = f.decoded.mtiInfo?.name ?? `Unknown(0x${f.decoded.mtiValue.toString(16).toUpperCase()})`;
      const src = resolveAlias(f.decoded.srcAlias, aliasMap);
      const dAlias = f.decoded.destAlias ?? f.addrDestAlias;
      const destStr = dAlias !== undefined ? ` → ${resolveAlias(dAlias, aliasMap)}` : "";
      const dir = f.direction === "R" ? "RECV" : "SENT";
      console.log(`  ${f.timestamp} [${dir}] ${src}${destStr}  ${mtiName}`);
    }
    console.log();
  }
}

main().catch(err => {
  process.stderr.write(`Error: ${err}\n`);
  process.exit(1);
});
