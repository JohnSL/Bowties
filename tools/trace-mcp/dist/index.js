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
// ─── Tool: decode_diagnostic_frames ─────────────────────────────────────────
const DiagFrameSchema = z.object({
    direction: z.enum(["rx", "tx"]),
    frame: z.string(),
    timestampMs: z.number(),
});
const KnownNodeSchema = z.object({
    nodeId: z.string(),
    snipName: z.string().optional(),
});
server.tool("decode_diagnostic_frames", "Decode the recentFrameActivity array from a Bowties diagnostic report. Returns decoded frames grouped by protocol conversation (discovery, SNIP, datagram/memory-config, events). timestampMs counts DOWN from report capture time.", {
    frames: z.array(DiagFrameSchema).describe("The recentFrameActivity array from the diagnostic JSON"),
    knownNodes: z.array(KnownNodeSchema).optional().describe("Optional: nodes[] from the discovery section to seed the alias map"),
}, ({ frames: diagFrames, knownNodes }) => {
    // Parse all frames as bare GridConnect
    const parsed = diagFrames.map(df => {
        const frame = parseLine(df.frame);
        return {
            ...frame,
            diagDirection: df.direction,
            diagTimestampMs: df.timestampMs,
        };
    }).filter(f => f.timestamp !== undefined || f.header !== undefined);
    // Build alias→nodeId map from Verified Node ID frames in the buffer
    const aliasToNode = new Map();
    // Seed from VerifiedNodeID frames (MTI 0x19170/0x19171): data = 6-byte NodeID
    for (const pf of parsed) {
        if (!pf)
            continue;
        const mtiVal = pf.decoded?.mtiValue;
        if ((mtiVal === 0x19170 || mtiVal === 0x19171) && pf.rawBytes && pf.rawBytes.length === 6) {
            const nodeId = pf.rawBytes.map(b => b.toString(16).toUpperCase().padStart(2, "0")).join("");
            const srcAlias = pf.decoded.srcAlias;
            aliasToNode.set(srcAlias, { nodeId });
        }
    }
    // Overlay knownNodes (from discovery.nodes[])
    if (knownNodes) {
        for (const kn of knownNodes) {
            const nodeIdUpper = kn.nodeId.toUpperCase().replace(/[.\-\s]/g, "");
            // Find matching alias
            for (const [alias, entry] of aliasToNode) {
                if (entry.nodeId === nodeIdUpper) {
                    entry.snipName = kn.snipName;
                }
            }
            // If not found by NodeID, check if we can match from frame data
        }
    }
    // Group frames by conversation type
    const conversations = [];
    const discoveryFrames = [];
    const snipExchanges = [];
    const datagramFrames = [];
    const eventFrames = [];
    const otherFrames = [];
    for (const pf of parsed) {
        if (!pf || !pf.decoded) {
            otherFrames.push(pf);
            continue;
        }
        const cat = pf.decoded.mtiInfo?.category;
        const mtiName = pf.decoded.mtiInfo?.name ?? "Unknown";
        const srcAlias = pf.decoded.srcAlias;
        const destAlias = pf.decoded.destAlias ?? pf.addrDestAlias;
        const resolveName = (alias) => {
            if (alias === undefined)
                return undefined;
            const entry = aliasToNode.get(alias);
            if (entry?.snipName)
                return `${entry.snipName} (0x${alias.toString(16).toUpperCase().padStart(3, "0")})`;
            if (entry?.nodeId)
                return `${entry.nodeId} (0x${alias.toString(16).toUpperCase().padStart(3, "0")})`;
            return `0x${alias.toString(16).toUpperCase().padStart(3, "0")}`;
        };
        const row = {
            direction: pf.diagDirection,
            msBeforeCapture: pf.diagTimestampMs,
            mti: mtiName,
            src: resolveName(srcAlias),
            dest: resolveName(destAlias),
            dataHex: pf.rawBytes?.map(b => b.toString(16).toUpperCase().padStart(2, "0")).join(" ") ?? "",
        };
        if (mtiName.includes("Verify Node") || mtiName.includes("Verified Node") || mtiName.includes("Initialization")) {
            discoveryFrames.push(row);
        }
        else if (mtiName.includes("SNIP")) {
            snipExchanges.push(row);
        }
        else if (cat === "datagram" || mtiName.includes("Datagram")) {
            datagramFrames.push(row);
        }
        else if (cat === "event" || pf.decoded.mtiInfo?.hasEventId) {
            eventFrames.push(row);
        }
        else {
            otherFrames.push(row);
        }
    }
    if (discoveryFrames.length)
        conversations.push({ type: "discovery", frames: discoveryFrames });
    if (snipExchanges.length)
        conversations.push({ type: "snip", frames: snipExchanges });
    if (datagramFrames.length)
        conversations.push({ type: "datagram/memory-config", frames: datagramFrames });
    if (eventFrames.length)
        conversations.push({ type: "events", frames: eventFrames });
    if (otherFrames.length)
        conversations.push({ type: "other", frames: otherFrames });
    // Summary
    const summary = {
        totalFrames: diagFrames.length,
        parsed: parsed.filter(p => p !== null).length,
        aliasMap: Object.fromEntries([...aliasToNode.entries()].map(([alias, entry]) => [
            `0x${alias.toString(16).toUpperCase().padStart(3, "0")}`,
            entry,
        ])),
        conversations,
        anomalies: [],
    };
    // Detect anomalies
    if (datagramFrames.length === 0) {
        summary.anomalies.push("No datagram frames in buffer — CDI/config reads not captured (either not attempted or ring buffer too small)");
    }
    const txCount = diagFrames.filter(f => f.direction === "tx").length;
    const rxCount = diagFrames.filter(f => f.direction === "rx").length;
    if (txCount === 0) {
        summary.anomalies.push("No TX frames — Bowties may not have sent any requests yet");
    }
    if (rxCount === 0) {
        summary.anomalies.push("No RX frames — possible serial connection issue");
    }
    return {
        content: [{ type: "text", text: JSON.stringify(summary, null, 2) }],
    };
});
// ─── Tool: analyze_diagnostic ───────────────────────────────────────────────
server.tool("analyze_diagnostic", "Analyze a complete Bowties diagnostic report JSON. Determines the session phase, interprets stats, decodes frame activity, flags anomalies, and suggests what additional data is needed.", {
    report: z.string().describe("The full diagnostic report JSON as a string"),
}, ({ report }) => {
    let json;
    try {
        json = JSON.parse(report);
    }
    catch (e) {
        return { content: [{ type: "text", text: JSON.stringify({ error: "Invalid JSON", detail: String(e) }) }] };
    }
    const stats = (json.stats ?? json);
    const log = (json.log ?? []);
    const recentFrames = (json.recentFrameActivity ?? []);
    const errors = (stats.errors ?? []);
    const cdiDownloads = (stats.cdiDownloads ?? stats.cdi_downloads ?? {});
    const configReads = (stats.configReads ?? stats.config_reads ?? {});
    const eventRoleExchange = stats.eventRoleExchange ?? stats.event_role_exchange ?? null;
    const discovery = (stats.discovery ?? {});
    const nodes = (discovery.nodes ?? []);
    const probes = (discovery.probes ?? []);
    // Determine session phase
    let phase;
    const hasCdiErrors = errors.some(e => e.phase === "cdi-download");
    const hasCdiDownloads = Object.keys(cdiDownloads).length > 0;
    const hasConfigReads = Object.keys(configReads).length > 0;
    if (hasCdiDownloads && hasConfigReads) {
        phase = "normal-session";
    }
    else if (hasCdiDownloads && !hasConfigReads) {
        phase = "cdi-complete-config-not-started";
    }
    else if (hasCdiErrors) {
        phase = "cdi-failed";
    }
    else if (eventRoleExchange !== null) {
        phase = "post-event-roles";
    }
    else if (nodes.length > 0) {
        phase = "discovery-complete-no-actions";
    }
    else {
        phase = "just-connected";
    }
    // Connection info
    const connection = {
        adapter: stats.adapterType,
        label: stats.connectionLabel,
        baudRate: stats.baudRate,
        flowControl: stats.flowControl,
        device: stats.device,
        appVersion: stats.appVersion,
        connectedAt: stats.connectedAt,
    };
    // Discovery summary
    const discoverySummary = {
        nodeCount: nodes.length,
        nodes: nodes.map(n => ({
            nodeId: n.nodeId ?? n.node_id,
            snipName: n.snipName ?? n.snip_name,
            msAfterConnect: n.msAfterConnect ?? n.ms_after_connect,
            snipQueryDurationMs: n.snipQueryDurationMs ?? n.snip_query_duration_ms,
        })),
        probeCount: probes.length,
        probes: probes.map(p => ({
            triggeredBy: p.triggeredBy ?? p.triggered_by,
            nodesRespondedCount: p.nodesRespondedCount ?? p.nodes_responded_count,
        })),
    };
    // Error analysis
    const errorSummary = errors.map(e => ({
        phase: e.phase,
        errorType: e.error_type ?? e.errorType,
        nodeId: e.node_id ?? e.nodeId,
        detail: e.detail,
    }));
    // Frame activity summary (quick counts without full decode)
    let frameSummary = { totalFrames: recentFrames.length };
    if (recentFrames.length > 0) {
        const txCount = recentFrames.filter(f => f.direction === "tx").length;
        const rxCount = recentFrames.filter(f => f.direction === "rx").length;
        frameSummary = {
            totalFrames: recentFrames.length,
            tx: txCount,
            rx: rxCount,
            oldestMs: Math.max(...recentFrames.map(f => f.timestampMs)),
            newestMs: Math.min(...recentFrames.map(f => f.timestampMs)),
            note: "Use decode_diagnostic_frames for full frame-by-frame analysis",
        };
    }
    // Anomalies
    const anomalies = [];
    if (phase === "just-connected") {
        anomalies.push("Report captured immediately after connect — no CDI or config operations attempted yet");
    }
    if (nodes.length === 0 && recentFrames.some(f => {
        const m = f.frame.match(/^:X([0-9a-fA-F]{8})N/i);
        if (!m)
            return false;
        const hdr = parseInt(m[1], 16);
        const mtiVal = (hdr >>> 12) & 0x1FFFF;
        return mtiVal === 0x19170 || mtiVal === 0x19171;
    })) {
        anomalies.push("Verified Node ID frames present in ring buffer but discovery.nodes is empty — report captured before async processing completed");
    }
    if (probes.every(p => (p.nodesRespondedCount ?? p.nodes_responded_count) === 0) && nodes.length > 0) {
        anomalies.push("probes[].nodesRespondedCount is 0 despite nodes being discovered — known diagnostic tracking bug (non-blocking)");
    }
    if (hasCdiErrors) {
        const timeouts = errors.filter(e => (e.error_type ?? e.errorType) === "timeout");
        if (timeouts.length > 0) {
            anomalies.push(`${timeouts.length} CDI download timeout(s) — node(s) not responding to Memory Config datagrams`);
        }
    }
    // Suggested next steps
    const suggestions = [];
    if (phase === "just-connected" || phase === "discovery-complete-no-actions") {
        suggestions.push("Ask user to trigger CDI download (click a node), wait for timeout, then re-capture diagnostic");
    }
    if (hasCdiErrors) {
        suggestions.push("Call decode_diagnostic_frames to check if datagram request frames were sent");
        suggestions.push("Ask user to test same node via TCP hub to isolate serial vs node issue");
    }
    const result = {
        phase,
        phaseDescription: {
            "just-connected": "Report captured right after connection — no CDI/config attempted",
            "discovery-complete-no-actions": "Nodes found, but no CDI download or config read yet",
            "post-event-roles": "Event roles exchanged but no CDI/config operations",
            "cdi-failed": "CDI download was attempted and failed",
            "cdi-complete-config-not-started": "CDI downloaded but config fields not read yet",
            "normal-session": "Full session with CDI and config reads",
        }[phase],
        connection,
        discovery: discoverySummary,
        errors: errorSummary,
        cdiDownloadCount: Object.keys(cdiDownloads).length,
        configReadCount: Object.keys(configReads).length,
        frameActivity: frameSummary,
        anomalies,
        suggestions,
        log: log.slice(0, 10), // First 10 log entries
    };
    return {
        content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
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