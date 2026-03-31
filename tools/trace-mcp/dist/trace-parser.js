/**
 * JMRI CAN Monitor trace parser.
 *
 * Parses lines produced by JMRI's LCC Monitor window with these settings:
 *   ✓ Show timestamps
 *   ✓ Show raw data
 *   ✓ Show Name for Node  (adds "  Src: …" / "    Dest: …" continuation lines)
 *   ✓ Event Name          (appends "    Name: …" on PCER lines)
 *   ✗ Event Uses          (not supported)
 *
 * Primary line format:
 *   HH:MM:SS.mmm: [[<8hex-header>] <space-separated-bytes>]  R|S: <decoded-text>
 *
 * Continuation lines (from "Show Name for Node") start with 2+ spaces:
 *   "  Src: NodeName"
 *   "    Dest: NodeName"
 *   These are attached to the preceding ParsedFrame as metadata.
 */
import { decodeHeader, extractAddrDest } from "./mti-table.js";
// ─── Regex ──────────────────────────────────────────────────────────────────
/**
 * Matches a primary trace line:
 *   group 1: timestamp  "HH:MM:SS.mmm"
 *   group 2: header     8 hex digits
 *   group 3: raw bytes  space-separated hex pairs, possibly padded with spaces
 *   group 4: direction  "R" or "S"
 *   group 5: JMRI decoded text (rest of line)
 */
const PRIMARY_LINE_RE = /^(\d{2}:\d{2}:\d{2}\.\d{3}):\s+\[\[([0-9a-fA-F]{1,8})\]([\s0-9a-fA-F]*)\]\s+([RS]):\s*(.*)$/;
/** Extracts all dotted-hex NodeIDs:  "09.00.99.05.01.C0" */
const NODE_ID_RE = /\b([0-9A-Fa-f]{2}(?:\.[0-9A-Fa-f]{2}){5})\b/g;
/** Continuation line: "  Src: name" or "    Dest: name" */
const SRC_LINE_RE = /^\s{2,}Src:\s*(.+)$/;
const DEST_LINE_RE = /^\s{2,}Dest:\s*(.+)$/;
/** "    Name: FastClock DCS240" at end of PCER line (same line) */
const EVENT_NAME_RE = /\s{2,}Name:\s*(.+)$/;
// ─── Helpers ────────────────────────────────────────────────────────────────
function parseHexBytes(raw) {
    const matches = raw.match(/[0-9a-fA-F]{2}/g);
    return matches ? matches.map(h => parseInt(h, 16)) : [];
}
function extractNodeIds(text) {
    const ids = [];
    let m;
    NODE_ID_RE.lastIndex = 0;
    while ((m = NODE_ID_RE.exec(text)) !== null) {
        ids.push(m[1].toUpperCase());
    }
    return ids;
}
// ─── Main parser ─────────────────────────────────────────────────────────────
/**
 * Parse a full JMRI CAN Monitor trace (one or many lines).
 * Handles multi-line entries where "Show Name for Node" adds continuation lines.
 */
export function parseTrace(text) {
    const lines = text.split(/\r?\n/);
    const frames = [];
    const unparsedLines = [];
    for (const line of lines) {
        if (line.trim() === "")
            continue;
        // Check for continuation lines (attach to last frame)
        if (frames.length > 0 && /^\s{2}/.test(line)) {
            const last = frames[frames.length - 1];
            const srcMatch = SRC_LINE_RE.exec(line);
            if (srcMatch) {
                last.srcName = srcMatch[1].trim();
                continue;
            }
            const destMatch = DEST_LINE_RE.exec(line);
            if (destMatch) {
                last.destName = destMatch[1].trim();
                continue;
            }
            // Other indented lines: ignore (e.g. future formats)
            continue;
        }
        const m = PRIMARY_LINE_RE.exec(line);
        if (!m) {
            unparsedLines.push(line);
            continue;
        }
        const [, timestamp, headerHex, bytesRaw, dirStr, jmriTextRaw] = m;
        const header = parseInt(headerHex, 16);
        const rawBytes = parseHexBytes(bytesRaw);
        const direction = dirStr;
        // Extract "    Name: …" from jmriText if present (Event Name option)
        let jmriText = jmriTextRaw;
        let eventName;
        const nameMatch = EVENT_NAME_RE.exec(jmriText);
        if (nameMatch) {
            eventName = nameMatch[1].trim();
            jmriText = jmriText.slice(0, nameMatch.index).trim();
        }
        const decoded = decodeHeader(header);
        const jmriNodeIds = extractNodeIds(jmriText);
        // For addressed messages, extract dest alias from data bytes
        let addrDestAlias;
        let addressedPayload;
        if (decoded.mtiInfo?.addressed && decoded.destAlias === undefined && rawBytes.length >= 2) {
            const result = extractAddrDest(rawBytes);
            addrDestAlias = result.destAlias;
            addressedPayload = result.payload;
        }
        const frame = {
            timestamp,
            header,
            decoded,
            rawBytes,
            addrDestAlias,
            addressedPayload,
            direction,
            jmriText,
            jmriNodeIds,
            eventName,
            rawLine: line,
        };
        frames.push(frame);
    }
    return { frames, unparsedLines };
}
/**
 * Parse a single trace line. Returns null if the line is blank or unparseable.
 */
export function parseLine(line) {
    const result = parseTrace(line);
    return result.frames[0] ?? null;
}
/**
 * Produce a human-readable one-line summary of a parsed frame.
 * Used by the decode_frame MCP tool.
 */
export function describeFrame(frame) {
    const { decoded, rawBytes, addrDestAlias, jmriNodeIds, direction, timestamp } = frame;
    const dir = direction === "R" ? "recv" : "sent";
    const srcStr = `alias:0x${decoded.srcAlias.toString(16).toUpperCase().padStart(3, "0")}`;
    const srcNode = jmriNodeIds[0] ? ` (${jmriNodeIds[0]})` : "";
    const mtiName = decoded.mtiInfo?.name ?? `Unknown MTI 0x${decoded.mtiValue.toString(16).toUpperCase()}`;
    // Destination
    let destStr = "";
    if (decoded.destAlias !== undefined) {
        const destNode = jmriNodeIds[1] ? ` (${jmriNodeIds[1]})` : "";
        destStr = ` → alias:0x${decoded.destAlias.toString(16).toUpperCase().padStart(3, "0")}${destNode}`;
    }
    else if (addrDestAlias !== undefined) {
        const destNode = jmriNodeIds[1] ? ` (${jmriNodeIds[1]})` : "";
        destStr = ` → alias:0x${addrDestAlias.toString(16).toUpperCase().padStart(3, "0")}${destNode}`;
    }
    const bytesStr = rawBytes.length > 0
        ? ` [${rawBytes.map(b => b.toString(16).toUpperCase().padStart(2, "0")).join(" ")}]`
        : "";
    return `${timestamp} ${dir} ${srcStr}${srcNode} ${mtiName}${destStr}${bytesStr}`;
}
//# sourceMappingURL=trace-parser.js.map