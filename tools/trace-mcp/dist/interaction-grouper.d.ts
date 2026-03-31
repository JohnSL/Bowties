/**
 * Interaction grouper for LCC/OpenLCB traces.
 *
 * Groups raw frames and assembled datagrams into typed logical interactions:
 *   memory-config  – request datagram → ACK → reply datagram → ACK
 *   snip           – SNIP Request addressed frame + multi-frame SNIP Response
 *   pip            – Protocol Support Inquiry + Reply (single frames each)
 *   verify         – Verify Node ID (addressed or global) + Verified Node ID responses
 *   traction       – Traction Control Request + Reply
 *   event-identification – Identify Events + Producer/Consumer Identified responses
 *   alias-negotiation    – CID 4-7 + RID (+ optional AMD/AME/AMR)
 *   event          – PCER, Learn Event, and standalone Identified frames
 *   datagram-ack   – Datagram Received OK / Rejected (orphaned or standalone)
 *   datagram       – Datagram not matched to any protocol pattern
 *   other          – Everything else
 *
 * Each interaction has a stable groupIndex and frameIndices array enabling
 * progressive drill-down via get_frames(). Pre-computed timing fields support
 * latency analysis without LLM arithmetic.
 */
import type { ParsedFrame } from "./trace-parser.js";
import type { AssembledDatagram } from "./datagram-reassembler.js";
import type { AliasMap } from "./alias-resolver.js";
export type InteractionType = "alias-negotiation" | "event" | "event-identification" | "snip" | "pip" | "verify" | "memory-config" | "traction" | "datagram-ack" | "datagram" | "other";
export interface InteractionTiming {
    /** Memory config: last frame of request datagram → Datagram Received OK from responder */
    requestToAckMs?: number | null;
    /** Memory config: request ACK → first frame of reply datagram */
    ackToReplyMs?: number | null;
    /** Memory config: last frame of reply datagram → Datagram Received OK from requester */
    replyToAckMs?: number | null;
    /** Simple 1:1 pairs (SNIP, PIP, Verify, Traction): request frame → first reply frame */
    requestToReplyMs?: number | null;
    /** All types: last frame of this interaction → first frame of next same-type same-src */
    gapToNextMs?: number | null;
}
export interface Interaction {
    groupIndex: number;
    type: InteractionType;
    /** Alias of the node that initiated the interaction */
    srcAlias: number;
    /** Alias of the destination node (undefined for broadcast) */
    destAlias?: number;
    /** Indices into the frames[] array for all frames in this interaction */
    frameIndices: number[];
    /** false when boundary frames are outside the trace window */
    complete: boolean;
    /** Human-readable one-line description */
    summary: string;
    /** Decoded fields relevant to this interaction type */
    fields: Record<string, string | number | boolean | string[] | null>;
    /** Pre-computed timing measurements (only applicable fields present) */
    timing: InteractionTiming;
}
export declare function buildInteractionGroups(frames: ParsedFrame[], datagrams: AssembledDatagram[], _aliasMap: AliasMap): Interaction[];
//# sourceMappingURL=interaction-grouper.d.ts.map