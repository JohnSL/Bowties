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
import { type DecodedHeader } from "./mti-table.js";
/** A single decoded CAN frame from one trace line */
export interface ParsedFrame {
    /** Wall-clock timestamp as written by JMRI */
    timestamp: string;
    /** Raw 29-bit CAN header value */
    header: number;
    /** Decoded header fields */
    decoded: DecodedHeader;
    /** Raw data bytes (all of them, including the dest-alias bytes for addressed msgs) */
    rawBytes: number[];
    /** For addressed messages: destination alias extracted from rawBytes[0:1] */
    addrDestAlias?: number;
    /** Remaining payload after dest bytes stripped (addressed msgs only) */
    addressedPayload?: number[];
    /** R = received by JMRI bridge, S = sent by JMRI bridge */
    direction: "R" | "S";
    /** The pre-decoded text JMRI appended after "R:" / "S:" */
    jmriText: string;
    /**
     * NodeIDs extracted from jmriText in the format "XX.XX.XX.XX.XX.XX".
     * Index 0 is typically the source node, index 1 (if present) the dest node.
     */
    jmriNodeIds: string[];
    /** Human-readable node names from "Show Name for Node" continuation lines */
    srcName?: string;
    destName?: string;
    /** Event name appended by "Event Name" option */
    eventName?: string;
    /** Original raw line text */
    rawLine: string;
}
/** Outcome of parsing an entire trace text */
export interface ParseResult {
    frames: ParsedFrame[];
    /** Lines that could not be parsed (blank lines excluded) */
    unparsedLines: string[];
}
/**
 * Parse a full JMRI CAN Monitor trace (one or many lines).
 * Handles multi-line entries where "Show Name for Node" adds continuation lines.
 */
export declare function parseTrace(text: string): ParseResult;
/**
 * Parse a single trace line. Returns null if the line is blank or unparseable.
 */
export declare function parseLine(line: string): ParsedFrame | null;
/**
 * Produce a human-readable one-line summary of a parsed frame.
 * Used by the decode_frame MCP tool.
 */
export declare function describeFrame(frame: ParsedFrame): string;
//# sourceMappingURL=trace-parser.d.ts.map