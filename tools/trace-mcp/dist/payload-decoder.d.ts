/**
 * Datagram payload decoder.
 *
 * Decodes the reassembled byte payload of LCC datagrams into human-readable
 * structured objects.
 *
 * Protocols covered:
 *   - Memory Configuration (0x20) — read, write, address space info, CDI
 *   - SNIP Request/Reply  (0xDE / 0xE4)
 *   - PIP  Request/Reply  (0x84 / 0x82)  [Protocol Identification Protocol]
 *   - Traction            (0x30 family)
 *   - Unknown             (raw hex)
 *
 * Reference: OpenLCB Standards S-9.7.4.x
 */
import type { Confidence } from "./datagram-reassembler.js";
export interface DecodedPayload {
    protocol: string;
    command: string;
    fields: Record<string, string | number | boolean>;
    summary: string;
    confidence: Confidence;
}
/**
 * Decode a datagram payload given as an array of bytes.
 * The first byte identifies the protocol.
 */
export declare function decodeDatagramPayload(bytes: number[]): DecodedPayload;
//# sourceMappingURL=payload-decoder.d.ts.map