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
export {};
//# sourceMappingURL=cli.d.ts.map