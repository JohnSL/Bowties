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
// MTI value constants (variable-field values)
const MTI_ONLY = 0x1A000;
const MTI_FIRST = 0x1B000;
const MTI_MIDDLE = 0x1C000;
const MTI_FINAL = 0x1D000;
// ─── Assembler ───────────────────────────────────────────────────────────────
/**
 * Reassemble all datagrams from a parsed trace.
 *
 * Returns an array of assembled datagrams, each potentially complete or partial.
 */
export function reassembleDatagrams(frames) {
    const inProgress = new Map(); // keyed by srcAlias
    const completed = [];
    for (let frameIdx = 0; frameIdx < frames.length; frameIdx++) {
        const frame = frames[frameIdx];
        const { decoded, rawBytes, timestamp } = frame;
        const { mtiValue, srcAlias, destAlias } = decoded;
        if (mtiValue === MTI_ONLY) {
            // Single-frame datagram: complete immediately
            completed.push({
                srcAlias,
                destAlias: destAlias ?? 0,
                bytes: [...rawBytes],
                complete: true,
                timestamps: [timestamp],
                frameIndices: [frameIdx],
                confidence: "full",
            });
            continue;
        }
        if (mtiValue === MTI_FIRST) {
            // Start a new datagram; discard any previously incomplete one from same src
            if (inProgress.has(srcAlias)) {
                // Previous datagram was never finished — emit as partial
                const prev = inProgress.get(srcAlias);
                completed.push(makePartial(prev));
            }
            inProgress.set(srcAlias, {
                srcAlias,
                destAlias: destAlias ?? 0,
                bytes: [...rawBytes],
                timestamps: [timestamp],
                frameIndices: [frameIdx],
                hadFirst: true,
            });
            continue;
        }
        if (mtiValue === MTI_MIDDLE) {
            let buf = inProgress.get(srcAlias);
            if (!buf) {
                // Orphaned middle frame — create a partial buffer
                buf = {
                    srcAlias,
                    destAlias: destAlias ?? 0,
                    bytes: [],
                    timestamps: [],
                    frameIndices: [],
                    hadFirst: false,
                };
                inProgress.set(srcAlias, buf);
            }
            buf.bytes.push(...rawBytes);
            buf.timestamps.push(timestamp);
            buf.frameIndices.push(frameIdx);
            continue;
        }
        if (mtiValue === MTI_FINAL) {
            let buf = inProgress.get(srcAlias);
            if (!buf) {
                // Orphaned final frame
                buf = {
                    srcAlias,
                    destAlias: destAlias ?? 0,
                    bytes: [],
                    timestamps: [],
                    frameIndices: [],
                    hadFirst: false,
                };
            }
            buf.bytes.push(...rawBytes);
            buf.timestamps.push(timestamp);
            buf.frameIndices.push(frameIdx);
            inProgress.delete(srcAlias);
            completed.push({
                srcAlias,
                destAlias: destAlias ?? buf.destAlias,
                bytes: buf.bytes,
                complete: buf.hadFirst,
                timestamps: buf.timestamps,
                frameIndices: buf.frameIndices,
                confidence: buf.hadFirst ? "full" : "partial",
            });
            continue;
        }
    }
    // Any datagrams still in progress at end-of-trace → emit as partial
    for (const buf of inProgress.values()) {
        completed.push(makePartial(buf));
    }
    return completed;
}
function makePartial(buf) {
    return {
        srcAlias: buf.srcAlias,
        destAlias: buf.destAlias,
        bytes: buf.bytes,
        complete: false,
        timestamps: buf.timestamps,
        frameIndices: buf.frameIndices,
        confidence: "partial",
    };
}
//# sourceMappingURL=datagram-reassembler.js.map