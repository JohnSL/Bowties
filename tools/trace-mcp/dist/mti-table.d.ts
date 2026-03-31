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
export type MtiCategory = "global" | "addressed" | "datagram" | "can-control" | "event" | "unknown";
export interface MtiInfo {
    name: string;
    category: MtiCategory;
    /** For 'addressed' messages, actual payload starts at data[2] */
    addressed: boolean;
    /** Frame carries an 8-byte EventID as its primary payload */
    hasEventId: boolean;
    /** For datagram-type frames (DatagramOnly/First/Middle/Final),
     *  dest alias is in header bits 23:12 instead of data[0:1] */
    isDatagramFrame: boolean;
}
/** Map from 17-bit variable-field value → MtiInfo */
export declare const MTI_TABLE: ReadonlyMap<number, MtiInfo>;
/**
 * Decode a raw 29-bit CAN header into its MTI and alias information.
 *
 * Returns:
 *  - srcAlias   – 12-bit source alias
 *  - variableField – bits 28:12 (17-bit value)
 *  - mtiValue   – canonical MTI value (datagram middle-bits zeroed)
 *  - mtiInfo    – looked-up descriptor, or undefined if unknown
 *  - destAlias  – present for datagram frames (from header bits 23:12)
 *  - isCidFrame – true for CAN Check ID frames (variable-field 0x14000..0x17FFF)
 */
export interface DecodedHeader {
    srcAlias: number;
    variableField: number;
    mtiValue: number;
    mtiInfo: MtiInfo | undefined;
    destAlias?: number;
    isCidFrame: boolean;
}
export declare function decodeHeader(header: number): DecodedHeader;
/**
 * For addressed messages, extract the destination alias from the first two
 * data bytes and return the remaining payload.
 *
 * Encoding: dest_alias = ((data[0] & 0x0F) << 8) | data[1]
 */
export declare function extractAddrDest(data: number[]): {
    destAlias: number;
    payload: number[];
};
//# sourceMappingURL=mti-table.d.ts.map