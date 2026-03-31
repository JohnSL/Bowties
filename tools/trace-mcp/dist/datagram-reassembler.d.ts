/**
 * Multi-frame datagram reassembler.
 *
 * LCC datagrams can span up to ~70 bytes across multiple CAN frames:
 *   DatagramFirst  (0x1B000) → DatagramMiddle* (0x1C000) → DatagramFinal (0x1D000)
 * or a single-frame datagram:
 *   DatagramOnly   (0x1A000)
 *
 * This module is ported from lcc-rs/src/protocol/datagram.rs and adapted for
 * use with pre-parsed trace frames (ParsedFrame).
 *
 * Fragment-safe design: if a Middle or Final frame arrives without a preceding
 * First frame (because the trace window starts mid-datagram), the assembler
 * emits a partial result with `complete: false` rather than discarding the bytes.
 */
import type { ParsedFrame } from "./trace-parser.js";
export type Confidence = "full" | "partial" | "inferred";
export interface AssembledDatagram {
    /** Source alias */
    srcAlias: number;
    /** Destination alias */
    destAlias: number;
    /** Reassembled payload bytes */
    bytes: number[];
    /** true if all frames were received (First/Middle/Final or Only) */
    complete: boolean;
    /** Timestamps of the contributing frames */
    timestamps: string[];
    /** Indices into the original frames[] array for all contributing frames */
    frameIndices: number[];
    confidence: Confidence;
}
/**
 * Reassemble all datagrams from a parsed trace.
 *
 * Returns an array of assembled datagrams, each potentially complete or partial.
 */
export declare function reassembleDatagrams(frames: ParsedFrame[]): AssembledDatagram[];
//# sourceMappingURL=datagram-reassembler.d.ts.map