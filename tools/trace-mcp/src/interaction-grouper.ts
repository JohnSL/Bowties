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

// ─── Types ────────────────────────────────────────────────────────────────────

export type InteractionType =
  | "alias-negotiation"
  | "event"
  | "event-identification"
  | "snip"
  | "pip"
  | "verify"
  | "memory-config"
  | "traction"
  | "datagram-ack"
  | "datagram"
  | "other";

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

// ─── MTI Constants ────────────────────────────────────────────────────────────

const MTI_DG_RECEIVED_OK          = 0x19A28;
const MTI_DG_REJECTED             = 0x19A48;
const MTI_PCER                    = 0x195B4;
const MTI_LEARN_EVENT             = 0x19594;
const MTI_IDENTIFY_EVENTS_GLOBAL  = 0x19970;
const MTI_IDENTIFY_EVENTS_ADDR    = 0x19968;
const MTI_VERIFY_GLOBAL           = 0x19490;
const MTI_VERIFY_ADDR             = 0x19488;
const MTI_VERIFIED_NODE           = 0x19170;
const MTI_VERIFIED_SIMPLE         = 0x19171;
const MTI_SNIP_REQUEST            = 0x19DE8;
const MTI_SNIP_RESPONSE           = 0x19A08;
const MTI_PIP_REQUEST             = 0x19828;
const MTI_PIP_REPLY               = 0x19668;
const MTI_TRACTION_REQUEST        = 0x195EB;
const MTI_TRACTION_REPLY          = 0x191E9;
const MTI_INIT_COMPLETE           = 0x19100;
const MTI_INIT_SIMPLE             = 0x19101;
const MTI_RID                     = 0x10700;
const MTI_AMD                     = 0x10701;
const MTI_AME                     = 0x10702;
const MTI_AMR                     = 0x10703;

const IDENTIFIED_MTIS = new Set([
  0x194A4, 0x194C4, 0x194C5, 0x194C7,  // Consumer Range/Valid/Invalid/Unknown
  0x19524, 0x19544, 0x19545, 0x19547,  // Producer Range/Valid/Invalid/Unknown
]);

// ─── Memory Config Classification ────────────────────────────────────────────
//
// Encoding (from OpenLCB S-9.7.4.2 Memory Configuration Protocol):
//   Requests:  0x40-0x43 = Read, 0x44-0x47 = Write, 0x84/0x86/0x88/0x8A = queries
//              0xA8 = Unfreeze, 0xAA = Freeze, 0xDE = Update Complete
//   Replies:   0x48-0x4F = Read Fail, 0x50-0x57 = Read/Write Reply OK,
//              0x58-0x5F = Write Fail, 0x85/0x87/0x89/0x8B = query replies
//
// Low 2 bits of request/reply cmd encode the address space:
//   0x00 = space given separately, 0x01 = 0xFD, 0x02 = 0xFE, 0x03 = 0xFF

function isMemConfigDatagram(dg: AssembledDatagram): boolean {
  return dg.bytes.length >= 2 && dg.bytes[0] === 0x20;
}

function isMemConfigReplyCmd(cmd: number): boolean {
  return (cmd >= 0x48 && cmd <= 0x5F) ||
    cmd === 0x85 || cmd === 0x87 || cmd === 0x89 || cmd === 0x8B;
}

function isMemConfigNoReplyCmd(cmd: number): boolean {
  // Fire-and-forget: only a transport ACK is expected, no reply datagram
  return cmd === 0xA8 || cmd === 0xAA || cmd === 0xDE;
}

function memCfgCmdName(cmd: number): string {
  const base = cmd & 0xFC;
  const space = cmd & 0x03;
  const spaceSuffix = space === 0 ? "" : space === 1 ? " (0xFD)" : space === 2 ? " (0xFE)" : " (0xFF)";
  if (base === 0x40) return `Read${spaceSuffix}`;
  if (base === 0x44) return `Write${spaceSuffix}`;
  if (base === 0x48) return `Read Reply Fail${spaceSuffix}`;
  if (base === 0x50) return `Read Reply OK${spaceSuffix}`;
  if (base === 0x54) return `Write Reply OK${spaceSuffix}`;
  if (base === 0x58) return `Write Reply Fail${spaceSuffix}`;
  const EXACT: Record<number, string> = {
    0x84: "Get Config Options",     0x85: "Get Config Options Reply",
    0x86: "Get Address Space Info", 0x87: "Get Address Space Info Reply",
    0x88: "Lock / Reserve",         0x89: "Lock Reply",
    0x8A: "Get Unique ID",          0x8B: "Get Unique ID Reply",
    0xA8: "Unfreeze / Reboot",      0xAA: "Freeze / Reset",
    0xDE: "Update Complete",
  };
  return EXACT[cmd] ?? `Cmd 0x${cmd.toString(16).toUpperCase()}`;
}

function spaceName(space: number): string {
  switch (space) {
    case 0xFF: return "0xFF (CDI)";
    case 0xFE: return "0xFE (All Memory)";
    case 0xFD: return "0xFD (Configuration)";
    case 0xFC: return "0xFC (Accel Scratch Pad)";
    case 0xFB: return "0xFB (Train Search)";
    default:   return `0x${space.toString(16).toUpperCase().padStart(2, "0")}`;
  }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function tsToMs(ts: string): number {
  const m = ts.match(/^(\d{2}):(\d{2}):(\d{2})\.(\d{3})$/);
  if (!m) return 0;
  return (parseInt(m[1]) * 3600 + parseInt(m[2]) * 60 + parseInt(m[3])) * 1000 + parseInt(m[4]);
}

function diffMs(earlier: string, later: string): number {
  return tsToMs(later) - tsToMs(earlier);
}

function toHex2(b: number): string {
  return b.toString(16).toUpperCase().padStart(2, "0");
}

function eventIdFromBytes(bytes: number[]): string {
  return bytes.slice(0, 8).map(toHex2).join(".");
}

/**
 * Multi-frame addressed message type indicator from high nibble of rawBytes[0]:
 *   0 = Only (single-frame), 1 = First, 2 = Last, 3 = Middle
 */
function addrFrameType(f: ParsedFrame): "only" | "first" | "middle" | "last" {
  if (!f.rawBytes.length) return "only";
  const ind = (f.rawBytes[0] >> 4) & 0x03;
  if (ind === 1) return "first";
  if (ind === 2) return "last";
  if (ind === 3) return "middle";
  return "only";
}

// ─── Main Grouper ─────────────────────────────────────────────────────────────

export function buildInteractionGroups(
  frames: ParsedFrame[],
  datagrams: AssembledDatagram[],
  _aliasMap: AliasMap,
): Interaction[] {
  const result: Interaction[] = [];

  // Frame index sets for membership tracking
  const consumed   = new Set<number>();  // non-datagram frames assigned to an interaction
  const consumedDg = new Set<number>();  // datagram indices already handled

  // Map every frame that belongs to a datagram
  const dgFrameSet = new Set<number>();
  for (const dg of datagrams) {
    for (const fi of dg.frameIndices) dgFrameSet.add(fi);
  }

  /**
   * Find the first non-datagram, non-consumed frame satisfying matchFn,
   * with timestamp in [minTs, maxTs).
   */
  function findFrame(
    startIdx: number,
    minTs: string,
    maxTs: string | null,
    matchFn: (f: ParsedFrame, i: number) => boolean,
  ): number | undefined {
    const minMs = tsToMs(minTs);
    const maxMs = maxTs ? tsToMs(maxTs) : Infinity;
    for (let i = startIdx; i < frames.length; i++) {
      if (consumed.has(i) || dgFrameSet.has(i)) continue;
      const fMs = tsToMs(frames[i].timestamp);
      if (fMs < minMs) continue;
      if (fMs > maxMs) break;
      if (matchFn(frames[i], i)) return i;
    }
    return undefined;
  }

  // ── Step 1: Memory Configuration ──────────────────────────────────────────
  // Pair request datagrams with reply datagrams; find associated ACK frames.

  interface DgEntry { di: number; ts: number; firstTs: string; lastTs: string; }
  const memReqs: DgEntry[] = [];
  const memReps: DgEntry[] = [];

  for (let di = 0; di < datagrams.length; di++) {
    const dg = datagrams[di];
    if (!isMemConfigDatagram(dg)) continue;
    const cmd = dg.bytes[1];
    const entry: DgEntry = {
      di,
      ts: tsToMs(dg.timestamps[0]),
      firstTs: dg.timestamps[0],
      lastTs: dg.timestamps[dg.timestamps.length - 1],
    };
    (isMemConfigReplyCmd(cmd) ? memReps : memReqs).push(entry);
  }

  memReqs.sort((a, b) => a.ts - b.ts);
  memReps.sort((a, b) => a.ts - b.ts);

  const usedRepDis = new Set<number>();

  for (const req of memReqs) {
    const dgReq = datagrams[req.di];
    const cmd = dgReq.bytes[1];
    const noReply = isMemConfigNoReplyCmd(cmd);

    // Find first matching reply: from opposite direction, after this request.
    // Use frame index ordering (not timestamp) because request and reply can
    // share the same millisecond timestamp in fast USB-to-CAN traces.
    let matchRep: DgEntry | undefined;
    const reqLastFrameIdx = dgReq.frameIndices[dgReq.frameIndices.length - 1] ?? 0;
    if (!noReply) {
      matchRep = memReps.find(rep =>
        !usedRepDis.has(rep.di) &&
        datagrams[rep.di].srcAlias === dgReq.destAlias &&
        datagrams[rep.di].destAlias === dgReq.srcAlias &&
        datagrams[rep.di].frameIndices[0] > reqLastFrameIdx,
      );
      if (matchRep) usedRepDis.add(matchRep.di);
    }

    const frameIndices = [...dgReq.frameIndices];
    if (matchRep) frameIndices.push(...datagrams[matchRep.di].frameIndices);

    // Request ACK: Datagram Received OK from responder → requester
    const reqAckIdx = findFrame(
      reqLastFrameIdx,
      req.lastTs,
      matchRep?.firstTs ?? null,
      (f) =>
        (f.decoded.mtiValue === MTI_DG_RECEIVED_OK || f.decoded.mtiValue === MTI_DG_REJECTED) &&
        f.decoded.srcAlias === dgReq.destAlias &&
        f.addrDestAlias === dgReq.srcAlias,
    );
    if (reqAckIdx !== undefined) { frameIndices.push(reqAckIdx); consumed.add(reqAckIdx); }

    // Reply ACK: Datagram Received OK from requester → responder
    let replyAckIdx: number | undefined;
    if (matchRep) {
      const repLastFrameIdx = datagrams[matchRep.di].frameIndices[datagrams[matchRep.di].frameIndices.length - 1] ?? 0;
      replyAckIdx = findFrame(
        repLastFrameIdx,
        matchRep.lastTs,
        null,
        (f) =>
          (f.decoded.mtiValue === MTI_DG_RECEIVED_OK || f.decoded.mtiValue === MTI_DG_REJECTED) &&
          f.decoded.srcAlias === dgReq.srcAlias &&
          f.addrDestAlias === dgReq.destAlias,
      );
      if (replyAckIdx !== undefined) { frameIndices.push(replyAckIdx); consumed.add(replyAckIdx); }
    }

    frameIndices.sort((a, b) => a - b);
    consumedDg.add(req.di);
    if (matchRep) consumedDg.add(matchRep.di);

    // Timing
    const timing: InteractionTiming = {};
    if (reqAckIdx !== undefined) {
      timing.requestToAckMs = diffMs(req.lastTs, frames[reqAckIdx].timestamp);
    }
    if (reqAckIdx !== undefined && matchRep) {
      timing.ackToReplyMs = diffMs(frames[reqAckIdx].timestamp, matchRep.firstTs);
    }
    if (replyAckIdx !== undefined && matchRep) {
      timing.replyToAckMs = diffMs(matchRep.lastTs, frames[replyAckIdx].timestamp);
    }

    // Fields from request
    const fields: Interaction["fields"] = { command: memCfgCmdName(cmd) };
    if (dgReq.bytes.length >= 6) {
      const addr = ((dgReq.bytes[2] << 24) | (dgReq.bytes[3] << 16) |
                    (dgReq.bytes[4] << 8)  |  dgReq.bytes[5]) >>> 0;
      fields.address = `0x${addr.toString(16).toUpperCase().padStart(8, "0")}`;
      const spaceSuffix = cmd & 0x03;
      const SPACE_MAP: Record<number, number> = { 1: 0xFD, 2: 0xFE, 3: 0xFF };
      const addrSpace = spaceSuffix !== 0 ? SPACE_MAP[spaceSuffix]! : (dgReq.bytes[6] ?? 0);
      fields.addressSpace = spaceName(addrSpace);
      const countOffset = spaceSuffix !== 0 ? 6 : 7;
      if (dgReq.bytes.length > countOffset) fields.byteCount = dgReq.bytes[countOffset];
    }

    // Fields from reply
    if (matchRep) {
      const rep = datagrams[matchRep.di];
      const repCmd = rep.bytes[1];
      fields.replyCommand = memCfgCmdName(repCmd);
      if ((repCmd >= 0x48 && repCmd <= 0x4F) || (repCmd >= 0x58 && repCmd <= 0x5F)) {
        if (rep.bytes.length >= 8) {
          const errCode = (rep.bytes[6] << 8) | rep.bytes[7];
          fields.errorCode = `0x${errCode.toString(16).toUpperCase().padStart(4, "0")}`;
        }
      } else {
        const spaceSuffix = repCmd & 0x03;
        const dataStart = spaceSuffix !== 0 ? 6 : 7;
        const data = rep.bytes.slice(dataStart);
        if (data.length > 0) {
          fields.dataByteCount = data.length;
          fields.dataBytes = data.length <= 16
            ? data.map(toHex2).join(" ")
            : `${data.slice(0, 16).map(toHex2).join(" ")}… (${data.length} total)`;
        }
      }
    }

    const addrStr = String(fields.address ?? "?");
    const spaceStr = String(fields.addressSpace ?? "?");
    const summary = matchRep
      ? `${memCfgCmdName(cmd)} addr=${addrStr} space=${spaceStr}`
      : `${memCfgCmdName(cmd)} addr=${addrStr} space=${spaceStr} (${noReply ? "ACK only" : "no reply"})`;

    result.push({
      groupIndex: 0,
      type: "memory-config",
      srcAlias: dgReq.srcAlias,
      destAlias: dgReq.destAlias,
      frameIndices,
      complete: !!(matchRep || noReply),
      summary,
      fields,
      timing,
    });
  }

  // Orphaned memory-config reply datagrams (no matching request)
  for (const rep of memReps) {
    if (usedRepDis.has(rep.di)) continue;
    const dg = datagrams[rep.di];
    consumedDg.add(rep.di);
    result.push({
      groupIndex: 0,
      type: "memory-config",
      srcAlias: dg.srcAlias,
      destAlias: dg.destAlias,
      frameIndices: [...dg.frameIndices],
      complete: false,
      summary: `Memory Config Reply (no matching request)`,
      fields: { replyCommand: memCfgCmdName(dg.bytes[1]) },
      timing: {},
    });
  }

  // ── Step 2: SNIP ──────────────────────────────────────────────────────────
  // SNIP Request is a single addressed frame. SNIP Response uses multi-frame
  // addressed encoding: high nibble of rawBytes[0] = 0=Only, 1=First, 2=Last, 3=Middle.

  const pendingSnipReqs = new Map<string, number>(); // `${srcAlias}-${destAlias}` → frameIdx

  for (let i = 0; i < frames.length; i++) {
    const f = frames[i];
    if (consumed.has(i) || dgFrameSet.has(i)) continue;

    if (f.decoded.mtiValue === MTI_SNIP_REQUEST && f.addrDestAlias !== undefined) {
      consumed.add(i);
      pendingSnipReqs.set(`${f.decoded.srcAlias}-${f.addrDestAlias}`, i);
      continue;
    }

    if (f.decoded.mtiValue !== MTI_SNIP_RESPONSE) continue;
    if (f.addrDestAlias === undefined) continue;

    const ft = addrFrameType(f);
    if (ft !== "only" && ft !== "first") continue; // orphaned middle/last handled in step 12

    const responderAlias = f.decoded.srcAlias;
    const requesterAlias = f.addrDestAlias;
    const reqKey = `${requesterAlias}-${responderAlias}`;
    const reqFrameIdx = pendingSnipReqs.get(reqKey);
    if (reqFrameIdx !== undefined) pendingSnipReqs.delete(reqKey);

    const frameIndices: number[] = [];
    if (reqFrameIdx !== undefined) frameIndices.push(reqFrameIdx);
    frameIndices.push(i);
    consumed.add(i);

    let complete = ft === "only";

    if (ft === "first") {
      // Collect middle + last frames from same node pair
      for (let j = i + 1; j < frames.length; j++) {
        const nf = frames[j];
        if (consumed.has(j) || dgFrameSet.has(j)) continue;
        if (nf.decoded.mtiValue !== MTI_SNIP_RESPONSE) break;
        if (nf.decoded.srcAlias !== responderAlias || nf.addrDestAlias !== requesterAlias) break;
        frameIndices.push(j);
        consumed.add(j);
        if (addrFrameType(nf) === "last") { complete = true; i = j; break; }
      }
    }

    const timing: InteractionTiming = {};
    if (reqFrameIdx !== undefined) {
      const firstRepIdx = frameIndices.find(fi => fi !== reqFrameIdx);
      if (firstRepIdx !== undefined) {
        timing.requestToReplyMs = diffMs(frames[reqFrameIdx].timestamp, frames[firstRepIdx].timestamp);
      }
    }

    // Summary: use JMRI decoded text from the last response frame (it includes full node info)
    const lastRespFrame = frames[frameIndices[frameIndices.length - 1]];
    let summary = "SNIP";
    const contentMatch = lastRespFrame.jmriText?.match(/content '([^']+)'/);
    if (contentMatch) {
      const parts = contentMatch[1].split(",");
      const nodeName = (parts[6] ?? parts[1] ?? parts[0] ?? "").trim();
      if (nodeName) summary = `SNIP: ${nodeName}`;
    } else if (lastRespFrame.srcName) {
      summary = `SNIP: ${lastRespFrame.srcName}`;
    } else if (reqFrameIdx !== undefined) {
      summary = "SNIP Request + Response";
    }

    result.push({
      groupIndex: 0,
      type: "snip",
      srcAlias: reqFrameIdx !== undefined ? frames[reqFrameIdx].decoded.srcAlias : requesterAlias,
      destAlias: responderAlias,
      frameIndices: frameIndices.sort((a, b) => a - b),
      complete,
      summary,
      fields: {},
      timing,
    });
  }

  // Orphaned SNIP requests
  for (const [, reqIdx] of pendingSnipReqs) {
    const f = frames[reqIdx];
    result.push({
      groupIndex: 0, type: "snip",
      srcAlias: f.decoded.srcAlias, destAlias: f.addrDestAlias,
      frameIndices: [reqIdx], complete: false,
      summary: "SNIP Request (no response in trace)",
      fields: {}, timing: {},
    });
  }

  // ── Step 3: Single-frame protocol pairs (PIP, Verify addressed, Traction) ─

  interface PairSpec {
    requestMti: number;
    replyMtis: number[];
    type: InteractionType;
    summarize: (req: ParsedFrame, rep?: ParsedFrame) => string;
    buildFields: (req: ParsedFrame, rep?: ParsedFrame) => Interaction["fields"];
  }

  const PAIR_SPECS: PairSpec[] = [
    {
      requestMti: MTI_PIP_REQUEST,
      replyMtis: [MTI_PIP_REPLY],
      type: "pip",
      summarize: (req, rep) => rep
        ? `PIP from ${req.addrDestAlias?.toString(16).toUpperCase().padStart(3, "0") ?? "?"}`
        : "PIP Request (no reply)",
      buildFields: (_req, rep): Interaction["fields"] => {
        if (!rep) return {};
        const payload = rep.addressedPayload ?? [];
        if (payload.length >= 3) {
          const flags = ((payload[0] << 16) | (payload[1] << 8) | payload[2]) >>> 0;
          return { flagsHex: `0x${flags.toString(16).toUpperCase().padStart(6, "0")}` };
        }
        return {};
      },
    },
    {
      requestMti: MTI_VERIFY_ADDR,
      replyMtis: [MTI_VERIFIED_NODE, MTI_VERIFIED_SIMPLE],
      type: "verify",
      summarize: (req, rep) => rep
        ? `Verified Node ID ${rep.jmriNodeIds[0] ?? ""}`
        : "Verify Node ID Addressed (no reply)",
      buildFields: (_req, rep): Interaction["fields"] => rep && rep.jmriNodeIds[0] ? { nodeId: rep.jmriNodeIds[0] } : {},
    },
    {
      requestMti: MTI_TRACTION_REQUEST,
      replyMtis: [MTI_TRACTION_REPLY],
      type: "traction",
      summarize: (_req, rep) => rep ? "Traction Control complete" : "Traction Control (no reply)",
      buildFields: () => ({}),
    },
  ];

  for (const spec of PAIR_SPECS) {
    const pending = new Map<string, number>(); // `${type}-${srcAlias}-${destAlias}` → frameIdx

    for (let i = 0; i < frames.length; i++) {
      const f = frames[i];
      if (consumed.has(i) || dgFrameSet.has(i)) continue;

      if (f.decoded.mtiValue === spec.requestMti && f.addrDestAlias !== undefined) {
        consumed.add(i);
        pending.set(`${spec.type}-${f.decoded.srcAlias}-${f.addrDestAlias}`, i);
        continue;
      }

      if (spec.replyMtis.includes(f.decoded.mtiValue) && f.addrDestAlias !== undefined) {
        const key = `${spec.type}-${f.addrDestAlias}-${f.decoded.srcAlias}`;
        const reqIdx = pending.get(key);
        if (reqIdx !== undefined) {
          pending.delete(key);
          consumed.add(i);
          const req = frames[reqIdx];
          result.push({
            groupIndex: 0, type: spec.type,
            srcAlias: req.decoded.srcAlias, destAlias: req.addrDestAlias,
            frameIndices: [reqIdx, i], complete: true,
            summary: spec.summarize(req, f),
            fields: spec.buildFields(req, f),
            timing: { requestToReplyMs: diffMs(req.timestamp, f.timestamp) },
          });
        } else {
          // Orphaned reply
          consumed.add(i);
          result.push({
            groupIndex: 0, type: spec.type,
            srcAlias: f.decoded.srcAlias, destAlias: f.addrDestAlias,
            frameIndices: [i], complete: false,
            summary: `${spec.type.toUpperCase()} Reply (no matching request)`,
            fields: spec.buildFields(frames[0], f),
            timing: {},
          });
        }
      }
    }

    // Emit orphaned requests
    for (const [, reqIdx] of pending) {
      const f = frames[reqIdx];
      result.push({
        groupIndex: 0, type: spec.type,
        srcAlias: f.decoded.srcAlias, destAlias: f.addrDestAlias,
        frameIndices: [reqIdx], complete: false,
        summary: spec.summarize(f, undefined),
        fields: spec.buildFields(f, undefined),
        timing: {},
      });
    }
  }

  // ── Step 4: Global Verify ──────────────────────────────────────────────────
  {
    const pendingGlobal: number[] = [];
    for (let i = 0; i < frames.length; i++) {
      const f = frames[i];
      if (consumed.has(i) || dgFrameSet.has(i)) continue;
      if (f.decoded.mtiValue !== MTI_VERIFY_GLOBAL) continue;
      consumed.add(i);
      pendingGlobal.push(i);
    }
    for (const reqIdx of pendingGlobal) {
      const reqTs = frames[reqIdx].timestamp;
      const frameIndices = [reqIdx];
      const nodes: string[] = [];
      for (let i = reqIdx + 1; i < frames.length; i++) {
        const f = frames[i];
        if (consumed.has(i) || dgFrameSet.has(i)) continue;
        const mti = f.decoded.mtiValue;
        if (mti !== MTI_VERIFIED_NODE && mti !== MTI_VERIFIED_SIMPLE) continue;
        if (diffMs(reqTs, f.timestamp) > 500) break;
        consumed.add(i);
        frameIndices.push(i);
        if (f.jmriNodeIds[0]) nodes.push(f.jmriNodeIds[0]);
      }
      result.push({
        groupIndex: 0, type: "verify",
        srcAlias: frames[reqIdx].decoded.srcAlias, destAlias: undefined,
        frameIndices, complete: true,
        summary: `Verify Node ID Global → ${nodes.length} response(s)`,
        fields: nodes.length > 0 ? { respondingNodes: nodes } : {},
        timing: {},
      });
    }
  }

  // ── Step 5: Identify Events + identified responses ─────────────────────────
  {
    const IDENTIFY_MTIS = new Set([MTI_IDENTIFY_EVENTS_GLOBAL, MTI_IDENTIFY_EVENTS_ADDR]);
    for (let i = 0; i < frames.length; i++) {
      const f = frames[i];
      if (consumed.has(i) || dgFrameSet.has(i)) continue;
      if (!IDENTIFY_MTIS.has(f.decoded.mtiValue)) continue;
      consumed.add(i);
      const frameIndices = [i];
      let count = 0;
      for (let j = i + 1; j < frames.length; j++) {
        const nf = frames[j];
        if (consumed.has(j) || dgFrameSet.has(j)) continue;
        if (!IDENTIFIED_MTIS.has(nf.decoded.mtiValue)) break;
        if (diffMs(f.timestamp, nf.timestamp) > 2000) break;
        consumed.add(j); frameIndices.push(j); count++;
      }
      result.push({
        groupIndex: 0, type: "event-identification",
        srcAlias: f.decoded.srcAlias, destAlias: f.addrDestAlias,
        frameIndices, complete: true,
        summary: `Identify Events → ${count} identified responses`,
        fields: { responseCount: count },
        timing: {},
      });
    }
  }

  // ── Step 6: Alias negotiation ─────────────────────────────────────────────
  {
    const cidByAlias = new Map<number, number[]>();
    for (let i = 0; i < frames.length; i++) {
      const f = frames[i];
      if (consumed.has(i) || dgFrameSet.has(i)) continue;
      if (f.decoded.isCidFrame) {
        consumed.add(i);
        const alias = f.decoded.srcAlias;
        if (!cidByAlias.has(alias)) cidByAlias.set(alias, []);
        cidByAlias.get(alias)!.push(i);
        continue;
      }
      const mti = f.decoded.mtiValue;
      if (mti === MTI_RID || mti === MTI_AMD || mti === MTI_AME || mti === MTI_AMR) {
        consumed.add(i);
        const alias = f.decoded.srcAlias;
        const cidFrames = cidByAlias.get(alias) ?? [];
        cidByAlias.delete(alias);
        result.push({
          groupIndex: 0, type: "alias-negotiation",
          srcAlias: alias, destAlias: undefined,
          frameIndices: [...cidFrames, i], complete: true,
          summary: `Alias negotiation 0x${alias.toString(16).toUpperCase().padStart(3, "0")}`,
          fields: { alias: `0x${alias.toString(16).toUpperCase().padStart(3, "0")}` },
          timing: {},
        });
      }
    }
    // Partial CID sequences (no RID in trace window)
    for (const [alias, cidFrames] of cidByAlias) {
      result.push({
        groupIndex: 0, type: "alias-negotiation",
        srcAlias: alias, destAlias: undefined,
        frameIndices: cidFrames, complete: false,
        summary: `Alias negotiation 0x${alias.toString(16).toUpperCase().padStart(3, "0")} (incomplete)`,
        fields: { alias: `0x${alias.toString(16).toUpperCase().padStart(3, "0")}` },
        timing: {},
      });
    }
  }

  // ── Step 7: Initialization Complete ───────────────────────────────────────
  for (let i = 0; i < frames.length; i++) {
    const f = frames[i];
    if (consumed.has(i) || dgFrameSet.has(i)) continue;
    if (f.decoded.mtiValue !== MTI_INIT_COMPLETE && f.decoded.mtiValue !== MTI_INIT_SIMPLE) continue;
    consumed.add(i);
    const nodeId = f.jmriNodeIds[0] ?? null;
    result.push({
      groupIndex: 0, type: "other",
      srcAlias: f.decoded.srcAlias, frameIndices: [i], complete: true,
      summary: `Initialization Complete${nodeId ? ` for ${nodeId}` : ""}`,
      fields: nodeId ? { nodeId } : {},
      timing: {},
    });
  }

  // ── Step 8: Standalone events (PCER, Learn Event) ─────────────────────────
  for (let i = 0; i < frames.length; i++) {
    const f = frames[i];
    if (consumed.has(i) || dgFrameSet.has(i)) continue;
    if (f.decoded.mtiValue !== MTI_PCER && f.decoded.mtiValue !== MTI_LEARN_EVENT) continue;
    consumed.add(i);
    const eventId = eventIdFromBytes(f.rawBytes);
    const label = f.decoded.mtiValue === MTI_PCER ? "PCER" : "Learn Event";
    const summary = `${label} ${eventId}${f.eventName ? ` (${f.eventName})` : ""}`;
    result.push({
      groupIndex: 0, type: "event",
      srcAlias: f.decoded.srcAlias, frameIndices: [i], complete: true,
      summary,
      fields: { eventId, ...(f.eventName ? { eventName: f.eventName } : {}) },
      timing: {},
    });
  }

  // ── Step 9: Remaining Producer/Consumer Identified frames ─────────────────
  for (let i = 0; i < frames.length; i++) {
    const f = frames[i];
    if (consumed.has(i) || dgFrameSet.has(i)) continue;
    if (!IDENTIFIED_MTIS.has(f.decoded.mtiValue)) continue;
    consumed.add(i);
    const eventId = eventIdFromBytes(f.rawBytes);
    const mtiName = f.decoded.mtiInfo?.name ?? "Identified";
    result.push({
      groupIndex: 0, type: "event",
      srcAlias: f.decoded.srcAlias, frameIndices: [i], complete: true,
      summary: `${mtiName} ${eventId}`,
      fields: { eventId, mtiName },
      timing: {},
    });
  }

  // ── Step 10: Orphaned Datagram ACKs ───────────────────────────────────────
  for (let i = 0; i < frames.length; i++) {
    const f = frames[i];
    if (consumed.has(i) || dgFrameSet.has(i)) continue;
    const mti = f.decoded.mtiValue;
    if (mti !== MTI_DG_RECEIVED_OK && mti !== MTI_DG_REJECTED) continue;
    consumed.add(i);
    const ok = mti === MTI_DG_RECEIVED_OK ? "OK" : "Rejected";
    result.push({
      groupIndex: 0, type: "datagram-ack",
      srcAlias: f.decoded.srcAlias, destAlias: f.addrDestAlias,
      frameIndices: [i], complete: true,
      summary: `Datagram Received ${ok}`,
      fields: { result: ok },
      timing: {},
    });
  }

  // ── Step 11: Unmatched datagrams ──────────────────────────────────────────
  for (let di = 0; di < datagrams.length; di++) {
    if (consumedDg.has(di)) continue;
    const dg = datagrams[di];
    consumedDg.add(di);
    for (const fi of dg.frameIndices) consumed.add(fi);
    const proto = dg.bytes[0] === 0x20 ? "Memory Config"
      : dg.bytes[0] === 0xDE || dg.bytes[0] === 0xE4 ? "SNIP"
      : dg.bytes[0] === 0x84 || dg.bytes[0] === 0x82 ? "PIP"
      : dg.bytes[0] === 0x30 ? "Traction"
      : "Unknown";
    const bytePreview = dg.bytes.slice(0, 8).map(toHex2).join(" ") + (dg.bytes.length > 8 ? "…" : "");
    result.push({
      groupIndex: 0, type: "datagram",
      srcAlias: dg.srcAlias, destAlias: dg.destAlias,
      frameIndices: [...dg.frameIndices],
      complete: dg.complete,
      summary: `${proto} datagram (${dg.bytes.length} bytes)`,
      fields: { protocol: proto, bytes: bytePreview },
      timing: {},
    });
  }

  // ── Step 12: Remaining frames ─────────────────────────────────────────────
  for (let i = 0; i < frames.length; i++) {
    if (consumed.has(i)) continue;
    const f = frames[i];
    consumed.add(i);
    const mtiName = f.decoded.mtiInfo?.name ?? `Unknown(0x${f.decoded.mtiValue.toString(16).toUpperCase()})`;
    result.push({
      groupIndex: 0, type: "other",
      srcAlias: f.decoded.srcAlias,
      destAlias: f.decoded.destAlias ?? f.addrDestAlias,
      frameIndices: [i], complete: true,
      summary: mtiName,
      fields: {},
      timing: {},
    });
  }

  // ── Step 13: Compute gapToNextMs ─────────────────────────────────────────
  // For each interaction, find the next interaction of the same type from the same src.
  const byKey = new Map<string, Interaction[]>();
  for (const g of result) {
    const key = `${g.type}-${g.srcAlias}`;
    if (!byKey.has(key)) byKey.set(key, []);
    byKey.get(key)!.push(g);
  }
  for (const group of byKey.values()) {
    group.sort((a, b) => (a.frameIndices[0] ?? 0) - (b.frameIndices[0] ?? 0));
    for (let i = 0; i < group.length - 1; i++) {
      const curr = group[i];
      const next = group[i + 1];
      const lastFi  = curr.frameIndices[curr.frameIndices.length - 1];
      const firstFi = next.frameIndices[0];
      if (lastFi !== undefined && firstFi !== undefined) {
        const lastTs  = frames[lastFi]?.timestamp;
        const firstTs = frames[firstFi]?.timestamp;
        if (lastTs && firstTs) curr.timing.gapToNextMs = diffMs(lastTs, firstTs);
      }
    }
  }

  // ── Step 14: Assign stable groupIndex ────────────────────────────────────
  for (let i = 0; i < result.length; i++) result[i].groupIndex = i;

  return result;
}
