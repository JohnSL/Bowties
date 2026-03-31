/**
 * LCC/OpenLCB Message Type Identifier (MTI) table.
 *
 * Values are the 17-bit "variable field" (bits 28:12 of the 29-bit CAN header),
 * i.e. `header >>> 12` for non-datagram frames.
 *
 * Sources:
 *  - lcc-rs/src/protocol/mti.rs  (primary)
 *  - OpenLCB_Java/src/org/openlcb/MessageTypeIdentifier.java (gap-fill for
 *    PCER, LearnEvent, Traction, Stream variants)
 *
 * Java formula for variable-field value: 0x19000 | java_mti
 * where java_mti = (upper<<12)|(priorityGroup<<10)|(typeNumber<<5)|(modifier&3)
 *                  | (addressed ? 0x08 : 0) | (hasEventID ? 0x04 : 0)
 *                  | (isSimpleMode ? 0x10 : 0)
 */
function mti(name, category, addressed = false, hasEventId = false, isDatagramFrame = false) {
    return { name, category, addressed, hasEventId, isDatagramFrame };
}
/** Map from 17-bit variable-field value → MtiInfo */
export const MTI_TABLE = new Map([
    // ── Initialization / Node Identity ─────────────────────────────────────
    [0x19100, mti("Initialization Complete", "global")],
    [0x19101, mti("Initialization Complete Simple", "global")],
    [0x19490, mti("Verify Node ID Global", "global")],
    [0x19488, mti("Verify Node ID Addressed", "addressed", true)],
    [0x19170, mti("Verified Node ID", "global")],
    [0x19171, mti("Verified Node ID Simple", "global")],
    // ── Error / Protocol ────────────────────────────────────────────────────
    [0x19068, mti("Optional Interaction Rejected", "addressed", true)],
    [0x190A8, mti("Terminate Due to Error", "addressed", true)],
    [0x19828, mti("Protocol Support Inquiry", "addressed", true)],
    [0x19668, mti("Protocol Support Reply", "addressed", true)],
    // ── Consumer Messages ───────────────────────────────────────────────────
    [0x198F4, mti("Identify Consumers", "event", false, true)],
    [0x194A4, mti("Consumer Range Identified", "event", false, true)],
    [0x194C4, mti("Consumer Identified Valid", "event", false, true)],
    [0x194C5, mti("Consumer Identified Invalid", "event", false, true)],
    [0x194C7, mti("Consumer Identified Unknown", "event", false, true)],
    // ── Producer Messages ───────────────────────────────────────────────────
    [0x19914, mti("Identify Producers", "event", false, true)],
    [0x19524, mti("Producer Range Identified", "event", false, true)],
    [0x19544, mti("Producer Identified Valid", "event", false, true)],
    [0x19545, mti("Producer Identified Invalid", "event", false, true)],
    [0x19547, mti("Producer Identified Unknown", "event", false, true)],
    // ── Event Exchange ──────────────────────────────────────────────────────
    [0x19970, mti("Identify Events Global", "global")],
    [0x19968, mti("Identify Events Addressed", "addressed", true)],
    [0x19594, mti("Learn Event", "event", false, true)],
    [0x195B4, mti("Producer/Consumer Event Report", "event", false, true)],
    // CAN-only multi-frame PCER variants
    [0x19F16, mti("PCER First Frame", "event", false, true)],
    [0x19F15, mti("PCER Middle Frame", "event", false, true)],
    [0x19F14, mti("PCER Last Frame", "event", false, true)],
    // ── Datagram ────────────────────────────────────────────────────────────
    [0x1A000, mti("Datagram Only", "datagram", false, false, true)],
    [0x1B000, mti("Datagram First", "datagram", false, false, true)],
    [0x1C000, mti("Datagram Middle", "datagram", false, false, true)],
    [0x1D000, mti("Datagram Final", "datagram", false, false, true)],
    [0x1C480, mti("Datagram", "datagram", false, false, true)],
    [0x19A28, mti("Datagram Received OK", "addressed", true)],
    [0x19A48, mti("Datagram Rejected", "addressed", true)],
    // ── Simple Node Identification (SNIP) ───────────────────────────────────
    [0x19DE8, mti("SNIP Request", "addressed", true)],
    [0x19A08, mti("SNIP Response", "addressed", true)],
    // ── Traction ─────────────────────────────────────────────────────────────
    [0x195EB, mti("Traction Control Request", "addressed", true)],
    [0x191E9, mti("Traction Control Reply", "addressed", true)],
    [0x195EA, mti("Traction Proxy Request", "addressed", true)],
    [0x191E8, mti("Traction Proxy Reply", "addressed", true)],
    // ── Stream ───────────────────────────────────────────────────────────────
    [0x19CC8, mti("Stream Initiate Request", "addressed", true)],
    [0x19868, mti("Stream Initiate Reply", "addressed", true)],
    [0x19C88, mti("Stream Data Proceed", "addressed", true)],
    [0x19888, mti("Stream Data Complete", "addressed", true)],
    // ── CAN Control (alias allocation) ──────────────────────────────────────
    // CID frames are matched by pattern, not exact value (they embed NodeID bits).
    // RID / AMD / AME / AMR are exact matches.
    [0x10700, mti("Reserve ID (RID)", "can-control")],
    [0x10701, mti("Alias Map Definition (AMD)", "can-control")],
    [0x10702, mti("Alias Map Enquiry (AME)", "can-control")],
    [0x10703, mti("Alias Map Reset (AMR)", "can-control")],
]);
export function decodeHeader(header) {
    const srcAlias = header & 0xFFF;
    const variableField = (header >>> 12) & 0x1FFFF; // 17-bit
    const topNibble = variableField >>> 12; // 5-bit frame-type indicator
    // CAN control: CID frames (0x14..0x17 = check-ID frames 4-7)
    if (topNibble >= 0x14 && topNibble <= 0x17) {
        return {
            srcAlias,
            variableField,
            mtiValue: variableField,
            mtiInfo: { name: `CID ${topNibble - 0x10} Frame`, category: "can-control", addressed: false, hasEventId: false, isDatagramFrame: false },
            isCidFrame: true,
        };
    }
    // CAN control: RID / AMD / AME / AMR (exact match)
    if (topNibble === 0x10) {
        const mtiInfo = MTI_TABLE.get(variableField);
        return { srcAlias, variableField, mtiValue: variableField, mtiInfo, isCidFrame: false };
    }
    // Datagram frames: dest alias in bits 23:12
    if (topNibble >= 0x1A && topNibble <= 0x1D) {
        const mtiValue = topNibble << 12; // 0x1A000..0x1D000
        const destAlias = variableField & 0xFFF;
        const mtiInfo = MTI_TABLE.get(mtiValue);
        return { srcAlias, variableField, mtiValue, mtiInfo, destAlias, isCidFrame: false };
    }
    // Standard OpenLCB global/addressed message (topNibble typically 0x19)
    const mtiInfo = MTI_TABLE.get(variableField);
    return { srcAlias, variableField, mtiValue: variableField, mtiInfo, isCidFrame: false };
}
/**
 * For addressed messages, extract the destination alias from the first two
 * data bytes and return the remaining payload.
 *
 * Encoding: dest_alias = ((data[0] & 0x0F) << 8) | data[1]
 */
export function extractAddrDest(data) {
    if (data.length < 2) {
        return { destAlias: 0, payload: data };
    }
    const destAlias = ((data[0] & 0x0F) << 8) | data[1];
    return { destAlias, payload: data.slice(2) };
}
//# sourceMappingURL=mti-table.js.map