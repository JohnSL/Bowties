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

// ─── Types ──────────────────────────────────────────────────────────────────

export interface DecodedPayload {
  protocol: string;
  command: string;
  fields: Record<string, string | number | boolean>;
  summary: string;
  confidence: Confidence;
}

// ─── Memory Configuration Protocol (0x20) ───────────────────────────────────

/*
 * Command byte encoding (byte 1):
 *   High nibble selects operation class:
 *     0x40 = Read family
 *     0x50 = Write family
 *     0x60 = Operations (not common in traces)
 *     0x80 = Node info / address space
 *     0xA8 = Unfreeze; 0xAA = Freeze
 *     0xDE = Update Complete
 *
 *   Low 2 bits for Read/Write encode address space when using the "top 3":
 *     00 = space given in byte 6
 *     01 = space 0xFD
 *     10 = space 0xFE
 *     11 = space 0xFF
 *
 *   Addressed bit (bit 3): 0 = no dest bytes stripped yet; for datagrams the
 *   dest was already in the header, so bytes start from protocol byte.
 */

const ADDRESS_SPACE_SUFFIX: Record<number, number> = { 0x01: 0xFD, 0x02: 0xFE, 0x03: 0xFF };

const MEM_CFG_COMMANDS: Record<number, string> = {
  0x40: "Read",
  0x41: "Read Reply (OK)",
  0x42: "Read Under Mask",
  0x43: "Read Under Mask Reply (OK)",
  0x44: "Write",
  0x45: "Write Under Mask",
  0x46: "Write Stream",
  0x4A: "Read Reply (Fail)",
  0x4B: "Read Under Mask Reply (Fail)",
  0x50: "Write Reply (OK)",
  0x51: "Write Reply (Fail)",
  0x52: "Write Stream Reply (OK)",
  0x53: "Write Stream Reply (Fail)",
  0x84: "Get Config Options",
  0x85: "Get Config Options Reply",
  0x86: "Get Address Space Info",
  0x87: "Get Address Space Info Reply",
  0x88: "Lock / Reserve",
  0x89: "Lock Reply",
  0x8A: "Get Unique ID",
  0x8B: "Get Unique ID Reply",
  0xA8: "Unfreeze / Reboot",
  0xAA: "Freeze / Reset",
  0xDE: "Update Complete",
};

function decodeMemoryConfig(bytes: number[]): DecodedPayload {
  if (bytes.length < 2) {
    return {
      protocol: "Memory Configuration",
      command: "?",
      fields: { rawHex: toHex(bytes) },
      summary: "Truncated memory config datagram",
      confidence: "partial",
    };
  }

  const cmd = bytes[1];
  // Mask off the low 2 bits to get the base command; low bits encode space
  const baseCmd = cmd & 0xFC;
  const spaceSuffix = cmd & 0x03;
  const cmdName = MEM_CFG_COMMANDS[cmd] ?? MEM_CFG_COMMANDS[baseCmd] ?? `Unknown (0x${cmd.toString(16).toUpperCase()})`;

  const isReadFamily  = (cmd & 0xF0) === 0x40;
  const isWriteFamily = (cmd & 0xF0) === 0x50;
  const isReadReply   = (cmd & 0xFE) === 0x40 || cmd === 0x41;
  const isWriteCmd    = (cmd & 0xFC) === 0x44;

  const fields: Record<string, string | number | boolean> = { command: `0x${cmd.toString(16).toUpperCase().padStart(2,"0")} (${cmdName})` };
  let summary = cmdName;

  if ((isReadFamily || isWriteFamily) && bytes.length >= 6) {
    // Bytes 2-5: 32-bit address, big-endian
    const address = ((bytes[2] << 24) | (bytes[3] << 16) | (bytes[4] << 8) | bytes[5]) >>> 0;
    fields.address = `0x${address.toString(16).toUpperCase().padStart(8, "0")}`;

    // Address space
    let addrSpace: number;
    let payloadStart: number;
    if (spaceSuffix !== 0) {
      addrSpace = ADDRESS_SPACE_SUFFIX[spaceSuffix]!;
      payloadStart = 6;
    } else if (bytes.length >= 7) {
      addrSpace = bytes[6];
      payloadStart = 7;
    } else {
      addrSpace = 0;
      payloadStart = 6;
    }
    fields.addressSpace = `0x${addrSpace.toString(16).toUpperCase().padStart(2, "0")}`;

    // Data / count
    const data = bytes.slice(payloadStart);
    if (isReadReply || isWriteCmd) {
      if (data.length > 0) {
        fields.dataBytes = toHex(data);
        fields.byteCount = data.length;
      }
    } else if ((cmd & 0xFC) === 0x40 && bytes.length > payloadStart) {
      // Read command includes a count byte
      fields.count = bytes[payloadStart];
    }

    const spaceName = addressSpaceName(addrSpace);
    summary = `${cmdName} addr=0x${address.toString(16).toUpperCase()} space=${spaceName}`;
    if (fields.dataBytes) summary += ` data=[${fields.dataBytes}]`;
  } else if ((cmd & 0xFC) === 0x50 && bytes.length >= 2) {
    // Write reply: may contain error code
    if (bytes.length >= 4) {
      const errCode = (bytes[2] << 8) | bytes[3];
      fields.errorCode = `0x${errCode.toString(16).toUpperCase().padStart(4, "0")}`;
      summary = `${cmdName} error=0x${errCode.toString(16).toUpperCase()}`;
    }
  }

  return {
    protocol: "Memory Configuration",
    command: cmdName,
    fields,
    summary,
    confidence: "full",
  };
}

function addressSpaceName(space: number): string {
  switch (space) {
    case 0xFF: return "0xFF (CDI)";
    case 0xFE: return "0xFE (All Memory)";
    case 0xFD: return "0xFD (Configuration)";
    case 0xFC: return "0xFC (Accel Scratch Pad)";
    case 0xFB: return "0xFB (Train Search)";
    default:   return `0x${space.toString(16).toUpperCase().padStart(2, "0")}`;
  }
}

// ─── SNIP (Simple Node Identification) ──────────────────────────────────────

function decodeSnip(bytes: number[]): DecodedPayload {
  const isRequest = bytes[0] === 0xDE;
  if (isRequest) {
    return {
      protocol: "SNIP",
      command: "Request",
      fields: {},
      summary: "SNIP Request (get node name/description)",
      confidence: "full",
    };
  }

  // SNIP Reply (0xE4): null-terminated strings
  const strings = extractNullStrings(bytes.slice(1));
  const labels = ["Manufacturer", "Model", "Hardware Version", "Software Version", "Node Name", "Node Description"];
  const fields: Record<string, string | number | boolean> = {};
  strings.forEach((s, i) => { fields[labels[i] ?? `str${i}`] = s; });

  return {
    protocol: "SNIP",
    command: "Reply",
    fields,
    summary: `SNIP Reply: ${strings[4] ?? strings[0] ?? "(no name)"}`,
    confidence: "full",
  };
}

function extractNullStrings(bytes: number[]): string[] {
  const strings: string[] = [];
  let current = "";
  for (const b of bytes) {
    if (b === 0) {
      strings.push(current);
      current = "";
    } else {
      current += String.fromCharCode(b);
    }
  }
  if (current) strings.push(current);
  return strings;
}

// ─── PIP (Protocol Identification Protocol) ─────────────────────────────────

const PIP_BITS: [number, string][] = [
  [0x800000, "Simple Protocol"],
  [0x400000, "Datagram Protocol"],
  [0x200000, "Stream Protocol"],
  [0x100000, "Memory Configuration"],
  [0x080000, "Reservation"],
  [0x040000, "Producer/Consumer Event Transport"],
  [0x020000, "Identification"],
  [0x010000, "Teaching/Learning"],
  [0x008000, "Remote Button"],
  [0x004000, "Abbreviated Default CDI"],
  [0x002000, "Display Protocol"],
  [0x001000, "Simple Node Information"],
  [0x000800, "Configuration Description Information"],
  [0x000400, "Traction Control"],
  [0x000200, "Function Description Information"],
  [0x000100, "DCC Command Station"],
  [0x000080, "Simple Train Node"],
  [0x000040, "Function Configuration"],
  [0x000020, "Firmware Upgrade"],
];

function decodePip(bytes: number[]): DecodedPayload {
  const isRequest = bytes[0] === 0x84;
  if (isRequest) {
    return {
      protocol: "PIP",
      command: "Request",
      fields: {},
      summary: "PIP Request (protocol identification inquiry)",
      confidence: "full",
    };
  }

  // Reply: 8 bytes of protocol flags after the 0x82 byte
  if (bytes.length < 4) {
    return { protocol: "PIP", command: "Reply", fields: { rawHex: toHex(bytes) }, summary: "PIP Reply (truncated)", confidence: "partial" };
  }

  const flags = ((bytes[1] << 16) | (bytes[2] << 8) | bytes[3]) >>> 0;
  const supported = PIP_BITS.filter(([bit]) => (flags & bit) !== 0).map(([, name]) => name);

  return {
    protocol: "PIP",
    command: "Reply",
    fields: { flags: `0x${flags.toString(16).toUpperCase().padStart(6, "0")}`, supported: supported.join(", ") },
    summary: `PIP Reply: [${supported.join(", ")}]`,
    confidence: "full",
  };
}

// ─── Traction Control (0x30 family) ─────────────────────────────────────────

const TRACTION_CMDS: Record<number, string> = {
  0x00: "Set Speed/Direction",
  0x01: "Set Function",
  0x10: "Emergency Stop",
  0x20: "Query Speed",
  0x21: "Query Function",
  0x30: "Controller Config (Assign)",
  0x31: "Controller Config (Release)",
  0x34: "Controller Config (Query)",
  0x40: "Consist (Attach)",
  0x41: "Consist (Detach)",
  0x42: "Consist (Query)",
  0x48: "Manage Reserve",
  0x49: "Manage Reserve Reply",
};

function decodeTraction(bytes: number[]): DecodedPayload {
  const cmd = bytes[1];
  const cmdName = TRACTION_CMDS[cmd] ?? `Unknown (0x${cmd.toString(16).toUpperCase()})`;
  const fields: Record<string, string | number | boolean> = {
    command: `0x${cmd.toString(16).toUpperCase().padStart(2, "0")} (${cmdName})`,
  };

  if (cmd === 0x00 && bytes.length >= 4) {
    // Speed/Direction: 16-bit IEEE 754 half-float; positive = forward
    const raw16 = (bytes[2] << 8) | bytes[3];
    const sign = (raw16 & 0x8000) !== 0 ? "Reverse" : "Forward";
    fields.direction = sign;
    fields.speedRaw = `0x${raw16.toString(16).toUpperCase().padStart(4, "0")}`;
  } else if (cmd === 0x01 && bytes.length >= 5) {
    const fnNum = (bytes[2] << 8) | bytes[3];
    fields.functionNumber = fnNum;
    fields.value = bytes[4];
  }

  return {
    protocol: "Traction Control",
    command: cmdName,
    fields,
    summary: `Traction ${cmdName}`,
    confidence: "full",
  };
}

// ─── Public entry point ──────────────────────────────────────────────────────

/**
 * Decode a datagram payload given as an array of bytes.
 * The first byte identifies the protocol.
 */
export function decodeDatagramPayload(bytes: number[]): DecodedPayload {
  if (bytes.length === 0) {
    return { protocol: "Empty", command: "", fields: {}, summary: "Empty datagram", confidence: "full" };
  }

  const proto = bytes[0];

  switch (proto) {
    case 0x20: return decodeMemoryConfig(bytes);
    case 0xDE: return decodeSnip(bytes);   // SNIP request
    case 0xE4: return decodeSnip(bytes);   // SNIP reply
    case 0x84: return decodePip(bytes);    // PIP request
    case 0x82: return decodePip(bytes);    // PIP reply
    case 0x30: return decodeTraction(bytes);
    default:
      return {
        protocol: `Unknown (0x${proto.toString(16).toUpperCase().padStart(2, "0")})`,
        command: "",
        fields: { rawHex: toHex(bytes) },
        summary: `Unknown datagram protocol 0x${proto.toString(16).toUpperCase()}, ${bytes.length} bytes`,
        confidence: "partial",
      };
  }
}

// ─── Utility ─────────────────────────────────────────────────────────────────

function toHex(bytes: number[]): string {
  return bytes.map(b => b.toString(16).toUpperCase().padStart(2, "0")).join(" ");
}
