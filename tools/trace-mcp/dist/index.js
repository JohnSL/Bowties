#!/usr/bin/env node
/**
 * LCC Trace MCP Server
 *
 * Exposes 9 tools for analysing JMRI CAN Monitor traces in Copilot Chat.
 * Run as an MCP server via stdio transport.
 *
 * Usage in .vscode/mcp.json:
 * {
 *   "servers": {
 *     "lcc-trace": {
 *       "type": "stdio",
 *       "command": "node",
 *       "args": ["${workspaceFolder}/tools/trace-mcp/dist/index.js"]
 *     }
 *   }
 * }
 */
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { readFile } from "node:fs/promises";
import clipboard from "clipboardy";
import { parseTrace, parseLine, describeFrame } from "./trace-parser.js";
import { buildAliasMap, resolveAlias, aliasMapToObject } from "./alias-resolver.js";
import { reassembleDatagrams } from "./datagram-reassembler.js";
import { buildInteractionGroups } from "./interaction-grouper.js";
import { decodeDatagramPayload } from "./payload-decoder.js";
// ─── Server ──────────────────────────────────────────────────────────────────
const server = new McpServer({
    name: "lcc-trace",
    version: "0.1.0",
});
let traceCache = null;
function loadAndCache(text) {
    const { frames, unparsedLines } = parseTrace(text);
    const aliasMap = buildAliasMap(frames);
    const datagrams = reassembleDatagrams(frames);
    const groups = buildInteractionGroups(frames, datagrams, aliasMap);
    traceCache = { frames, unparsedLines, aliasMap, groups };
    const hasBootFrames = frames.some(f => f.decoded.mtiValue === 0x19100 || f.decoded.mtiValue === 0x19101);
    const firstTs = frames[0]?.timestamp;
    const lastTs = frames[frames.length - 1]?.timestamp;
    const groupCounts = {};
    for (const g of groups)
        groupCounts[g.type] = (groupCounts[g.type] ?? 0) + 1;
    return {
        loaded: true,
        lineCount: text.split(/\r?\n/).filter(l => l.trim()).length,
        frameCount: frames.length,
        unparsedLineCount: unparsedLines.length,
        timeRange: firstTs && lastTs ? `${firstTs} \u2013 ${lastTs}` : null,
        hasBootFrames,
        groupCounts,
        note: "Trace cached. Call summarize_session, list_groups, or timeline to analyse.",
    };
}
// ─── Tool: read_trace_file ────────────────────────────────────────────────────
server.tool("read_trace_file", "Read a saved JMRI LCC Monitor trace from a file on disk. Returns the raw trace text.", { path: z.string().describe("Absolute path to the trace file") }, async ({ path }) => {
    const text = await readFile(path, "utf-8");
    const summary = loadAndCache(text);
    return {
        content: [{ type: "text", text: JSON.stringify({ path, ...summary }) }],
    };
});
// ─── Tool: read_clipboard ────────────────────────────────────────────────────
server.tool("read_clipboard", "Read the current clipboard contents. Use this when the user has copied a trace from JMRI but hasn't pasted it in the chat.", {}, async () => {
    const text = await clipboard.read();
    const summary = loadAndCache(text);
    return {
        content: [{ type: "text", text: JSON.stringify(summary) }],
    };
});
// ─── Tool: load_trace ────────────────────────────────────────────────────────
server.tool("load_trace", "Load a trace pasted directly into the chat. Parses and caches it server-side; analysis tools (summarize_session, list_groups, timeline) will use this cached trace automatically.", { trace: z.string().describe("Full text of the JMRI LCC Monitor trace") }, ({ trace }) => {
    const summary = loadAndCache(trace);
    return {
        content: [{ type: "text", text: JSON.stringify(summary) }],
    };
});
// ─── Tool: parse_line ────────────────────────────────────────────────────────
server.tool("parse_line", "Parse a single JMRI LCC Monitor trace line into its structured fields: timestamp, header, src/dest alias, MTI, data bytes, direction. Works on any single line with no prior context needed.", { line: z.string().describe("A single raw trace line from the JMRI LCC Monitor") }, ({ line }) => {
    const frame = parseLine(line);
    if (!frame) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "Could not parse line", line }) }] };
    }
    const result = {
        timestamp: frame.timestamp,
        direction: frame.direction,
        header: `0x${frame.header.toString(16).toUpperCase().padStart(8, "0")}`,
        srcAlias: `0x${frame.decoded.srcAlias.toString(16).toUpperCase().padStart(3, "0")}`,
        destAlias: frame.decoded.destAlias !== undefined
            ? `0x${frame.decoded.destAlias.toString(16).toUpperCase().padStart(3, "0")}`
            : frame.addrDestAlias !== undefined
                ? `0x${frame.addrDestAlias.toString(16).toUpperCase().padStart(3, "0")} (from data)`
                : null,
        mtiValue: `0x${frame.decoded.mtiValue.toString(16).toUpperCase().padStart(5, "0")}`,
        mtiName: frame.decoded.mtiInfo?.name ?? "Unknown",
        frameCategory: frame.decoded.mtiInfo?.category ?? "unknown",
        rawBytes: frame.rawBytes.map(b => b.toString(16).toUpperCase().padStart(2, "0")),
        jmriText: frame.jmriText,
        jmriNodeIds: frame.jmriNodeIds,
        srcName: frame.srcName,
        destName: frame.destName,
        eventName: frame.eventName,
    };
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
});
// ─── Tool: decode_frame ──────────────────────────────────────────────────────
server.tool("decode_frame", "Decode a single JMRI trace line and return a human-readable one-line description of what the frame is and what it means. No prior context needed.", { line: z.string().describe("A single raw trace line from the JMRI LCC Monitor") }, ({ line }) => {
    const frame = parseLine(line);
    if (!frame) {
        return { content: [{ type: "text", text: `Could not parse line: ${line}` }] };
    }
    const description = describeFrame(frame);
    const extra = [];
    if (frame.srcName)
        extra.push(`Src name: ${frame.srcName}`);
    if (frame.destName)
        extra.push(`Dest name: ${frame.destName}`);
    if (frame.eventName)
        extra.push(`Event: ${frame.eventName}`);
    const text = extra.length > 0 ? `${description}\n  ${extra.join("\n  ")}` : description;
    return { content: [{ type: "text", text }] };
});
// ─── Tool: summarize_session ─────────────────────────────────────────────────
server.tool("summarize_session", "Summarise the currently loaded LCC trace: alias\u2192NodeID map, message counts per node, interaction group counts, and whether the trace starts from node boot. Call read_clipboard, read_trace_file, or load_trace first.", {}, () => {
    if (!traceCache) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "No trace loaded. Call read_clipboard, read_trace_file, or load_trace first." }) }] };
    }
    const { frames, unparsedLines, aliasMap, groups } = traceCache;
    // Count message types per source alias
    const perNode = {};
    for (const frame of frames) {
        const aliasKey = `0x${frame.decoded.srcAlias.toString(16).toUpperCase().padStart(3, "0")}`;
        const mtiName = frame.decoded.mtiInfo?.name ?? `Unknown(0x${frame.decoded.mtiValue.toString(16).toUpperCase()})`;
        if (!perNode[aliasKey])
            perNode[aliasKey] = {};
        perNode[aliasKey][mtiName] = (perNode[aliasKey][mtiName] ?? 0) + 1;
    }
    const hasBootFrames = frames.some(f => f.decoded.mtiValue === 0x19100 || f.decoded.mtiValue === 0x19101);
    const firstTs = frames[0]?.timestamp;
    const lastTs = frames[frames.length - 1]?.timestamp;
    const groupCounts = {};
    for (const g of groups)
        groupCounts[g.type] = (groupCounts[g.type] ?? 0) + 1;
    const result = {
        frameCount: frames.length,
        unparsedLineCount: unparsedLines.length,
        timeRange: firstTs && lastTs ? `${firstTs} \u2013 ${lastTs}` : null,
        hasBootFrames,
        aliasCoverageNote: hasBootFrames
            ? "Alias map built from boot frames (confirmed) and JMRI text (inferred)"
            : "No boot frames in trace window \u2014 alias map inferred entirely from JMRI decoded text",
        nodes: aliasMapToObject(aliasMap),
        groupCounts,
        messagesPerNode: perNode,
    };
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
});
// ─── Tool: list_groups ───────────────────────────────────────────────────────
const INTERACTION_TYPES = [
    "alias-negotiation", "event", "event-identification", "snip", "pip",
    "verify", "memory-config", "traction", "datagram-ack", "datagram", "other",
];
server.tool("list_groups", "List interaction groups in the loaded trace. Requires at least one filter (type and/or nodeId). Returns protocol pairs (memory-config, snip, pip, verify, traction), events, alias negotiation, and individual frames. Each row includes groupIndex and frameIndices for drill-down via get_frames. Pre-computed timing fields (requestToAckMs, ackToReplyMs, replyToAckMs, gapToNextMs) enable latency analysis.", {
    type: z.enum(INTERACTION_TYPES).optional().describe("Filter by interaction type: memory-config | snip | pip | verify | traction | event | event-identification | alias-negotiation | datagram-ack | datagram | other"),
    nodeId: z.string().optional().describe("Filter to a specific node: NodeID like '09.00.99.05.01.C0' or alias like '0x646'"),
}, ({ type, nodeId }) => {
    if (!traceCache) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "No trace loaded. Call read_clipboard, read_trace_file, or load_trace first." }) }] };
    }
    if (!type && !nodeId) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "Provide at least one filter: type and/or nodeId." }) }] };
    }
    const { groups, aliasMap } = traceCache;
    // Resolve nodeId filter to an alias number
    let filterAlias;
    if (nodeId) {
        const asAlias = parseInt(nodeId.replace(/^0x/i, ""), 16);
        if (!isNaN(asAlias) && asAlias <= 0xFFF) {
            filterAlias = asAlias;
        }
        else {
            const upper = nodeId.toUpperCase();
            for (const [alias, entry] of aliasMap) {
                if (entry.nodeId === upper) {
                    filterAlias = alias;
                    break;
                }
            }
        }
    }
    let filtered = groups;
    if (type)
        filtered = filtered.filter(g => g.type === type);
    if (filterAlias !== undefined) {
        filtered = filtered.filter(g => g.srcAlias === filterAlias || g.destAlias === filterAlias);
    }
    const rows = filtered.map(g => {
        const row = {
            groupIndex: g.groupIndex,
            type: g.type,
            src: resolveAlias(g.srcAlias, aliasMap),
        };
        if (g.destAlias !== undefined)
            row.dest = resolveAlias(g.destAlias, aliasMap);
        row.summary = g.summary;
        if (!g.complete)
            row.complete = false;
        if (Object.keys(g.fields).length > 0)
            row.fields = g.fields;
        // Only include non-null timing values
        const t = {};
        for (const [k, v] of Object.entries(g.timing)) {
            if (v != null)
                t[k] = v;
        }
        if (Object.keys(t).length > 0)
            row.timing = t;
        row.frameCount = g.frameIndices.length;
        row.frameIndices = g.frameIndices;
        return row;
    });
    return {
        content: [{
                type: "text",
                text: JSON.stringify({ groupCount: rows.length, groups: rows }, null, 2),
            }],
    };
});
// ─── Tool: get_frames ─────────────────────────────────────────────────────────
server.tool("get_frames", "Get fully decoded details for specific frames by their indices (from list_groups or timeline). Use for drill-down after identifying interesting frames in a summary. Pass the frameIndices array from any list_groups result.", {
    frameIndices: z.array(z.number()).describe("Array of frame indices to retrieve"),
}, ({ frameIndices }) => {
    if (!traceCache) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "No trace loaded. Call read_clipboard, read_trace_file, or load_trace first." }) }] };
    }
    const { frames, aliasMap } = traceCache;
    const result = frameIndices
        .filter(i => i >= 0 && i < frames.length)
        .map(i => {
        const f = frames[i];
        const src = resolveAlias(f.decoded.srcAlias, aliasMap);
        const dAlias = f.decoded.destAlias ?? f.addrDestAlias;
        const dest = dAlias !== undefined ? resolveAlias(dAlias, aliasMap) : null;
        const row = {
            frameIndex: i,
            timestamp: f.timestamp,
            direction: f.direction,
            header: `0x${f.header.toString(16).toUpperCase().padStart(8, "0")}`,
            srcAlias: `0x${f.decoded.srcAlias.toString(16).toUpperCase().padStart(3, "0")}`,
            src,
            mtiValue: `0x${f.decoded.mtiValue.toString(16).toUpperCase().padStart(5, "0")}`,
            mtiName: f.decoded.mtiInfo?.name ?? "Unknown",
            rawBytes: f.rawBytes.map(b => b.toString(16).toUpperCase().padStart(2, "0")),
        };
        if (dest !== null) {
            row.destAlias = dAlias !== undefined
                ? `0x${dAlias.toString(16).toUpperCase().padStart(3, "0")}` : null;
            row.dest = dest;
        }
        if (f.addressedPayload) {
            row.addressedPayload = f.addressedPayload.map(b => b.toString(16).toUpperCase().padStart(2, "0"));
        }
        if (f.jmriText)
            row.jmriText = f.jmriText;
        if (f.srcName)
            row.srcName = f.srcName;
        if (f.destName)
            row.destName = f.destName;
        if (f.eventName)
            row.eventName = f.eventName;
        return row;
    });
    return {
        content: [{
                type: "text",
                text: JSON.stringify({ frameCount: result.length, frames: result }, null, 2),
            }],
    };
});
// ─── Tool: decode_datagram ────────────────────────────────────────────────────
server.tool("decode_datagram", "Decode a single datagram payload given as a hex string (space or no-space separated). Identifies the protocol (Memory Config, SNIP, PIP, Traction, etc.) and extracts fields. Fully context-free — no trace needed.", { bytesHex: z.string().describe("Hex bytes e.g. '20 41 00 00 02 64 08' or '204100000264 08'") }, ({ bytesHex }) => {
    const matches = bytesHex.match(/[0-9a-fA-F]{2}/g);
    if (!matches) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "No valid hex bytes found", input: bytesHex }) }] };
    }
    const bytes = matches.map(h => parseInt(h, 16));
    const decoded = decodeDatagramPayload(bytes);
    return {
        content: [
            {
                type: "text",
                text: JSON.stringify({ byteCount: bytes.length, bytes: matches.join(" "), ...decoded }, null, 2),
            },
        ],
    };
});
// ─── Tool: timeline ────────────────────────────────────────────────────────────
server.tool("timeline", "Show a chronological timeline of messages in the currently loaded trace, optionally filtered to a specific node (by NodeID or alias). Each entry shows the timestamp, node, message type, and destination. Call read_clipboard, read_trace_file, or load_trace first.", {
    nodeId: z.string().optional().describe("Filter to a specific node: a NodeID like '09.00.99.05.01.C0' or alias like '0x646'"),
}, ({ nodeId }) => {
    if (!traceCache) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "No trace loaded. Call read_clipboard, read_trace_file, or load_trace first." }) }] };
    }
    const { frames, aliasMap } = traceCache;
    // Resolve filter alias if supplied
    let filterAlias;
    if (nodeId) {
        const asAlias = parseInt(nodeId.replace(/^0x/i, ""), 16);
        if (!isNaN(asAlias)) {
            filterAlias = asAlias;
        }
        else {
            // Try to find by NodeID string
            const upper = nodeId.toUpperCase();
            for (const [alias, entry] of aliasMap) {
                if (entry.nodeId === upper) {
                    filterAlias = alias;
                    break;
                }
            }
        }
    }
    const events = frames
        .filter(f => filterAlias === undefined || f.decoded.srcAlias === filterAlias || f.decoded.destAlias === filterAlias || f.addrDestAlias === filterAlias)
        .map(f => {
        const mtiName = f.decoded.mtiInfo?.name ?? `Unknown(0x${f.decoded.mtiValue.toString(16).toUpperCase()})`;
        const src = resolveAlias(f.decoded.srcAlias, aliasMap);
        const dAlias = f.decoded.destAlias ?? f.addrDestAlias;
        const dest = dAlias !== undefined ? resolveAlias(dAlias, aliasMap) : null;
        return {
            timestamp: f.timestamp,
            direction: f.direction,
            src,
            dest,
            mti: mtiName,
            jmriText: f.jmriText || null,
            eventName: f.eventName ?? null,
        };
    });
    return {
        content: [
            {
                type: "text",
                text: JSON.stringify({ eventCount: events.length, filter: nodeId ?? "all nodes", events }, null, 2),
            },
        ],
    };
});
// ─── Start ────────────────────────────────────────────────────────────────────
async function main() {
    const transport = new StdioServerTransport();
    await server.connect(transport);
}
main().catch(err => {
    process.stderr.write(`LCC Trace MCP server error: ${err}\n`);
    process.exit(1);
});
//# sourceMappingURL=index.js.map