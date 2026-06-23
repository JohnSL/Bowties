---
name: diagnostic-analysis
description: >
  Interpret Bowties diagnostic reports from users. Knows what each field means,
  what absence of data implies at each session phase, and what to request next.
  Use when a user provides a diagnostic JSON (from issue, email, or clipboard)
  or when triaging connection/CDI/sync issues. Keywords: diagnostic, timeout,
  CDI, serial, SPROG, connection, debug, frame, ring buffer.
---

# Diagnostic Report Analysis

## When to invoke this skill

- A user provides a Bowties diagnostic report JSON (in an issue comment, email, or pasted)
- You are triaging a connection, CDI download, or config-read failure
- You need to understand what happened during a session based on the diagnostic snapshot

## Step 0 — Acquire the report

| How they give it | What to do |
|---|---|
| Pasted inline / issue comment | Extract the JSON block |
| Attached file | Read file |
| Multiple reports | Analyze each separately, then compare |

## Step 1 — Call `analyze_diagnostic` MCP tool

Pass the full JSON string as the `report` parameter. The tool returns:
- **Session phase** (just-connected / discovery-complete / cdi-failed / normal)
- **Connection details** (adapter, baud, flow control, device preset)
- **Discovery summary** (nodes found, probe timing)
- **Error entries** (phase, type, detail)
- **Frame activity summary** (TX/RX counts, time range)
- **Anomalies** (mismatches, timing issues)
- **Suggested next steps**

If the MCP tool is unavailable, fall back to the manual reference tables in Step 2–5.

## Step 2 — Determine session phase (manual fallback)

| Condition | Phase | Implication |
|-----------|-------|-------------|
| `cdiDownloads: {}` + `errors: []` + `eventRoleExchange: null` | Just connected | Report captured too early — CDI never attempted |
| `cdiDownloads: {}` + `errors` has `phase: "cdi-download"` entries | CDI failed | Real failure — analyze error details |
| `cdiDownloads` has entries + `configReads: {}` | CDI done, config not started | User hasn't opened node config panel |
| `cdiDownloads` has entries + `configReads` has entries | Normal session | Everything working |
| `eventRoleExchange` is non-null but `cdiDownloads: {}` | Post-event-roles | Connected and synced but no CDI read yet |

**Key insight:** The diagnostic is a point-in-time snapshot. Empty fields mean "not happened yet" — not necessarily "failed."

## Step 3 — Interpret discovery

| Field | Meaning | Anomaly |
|-------|---------|---------|
| `probes[].nodesRespondedCount` | Nodes that replied to that probe | Always 0 = known tracking bug (non-blocking) |
| `discovery.nodes[]` | Nodes successfully identified | Empty despite frames in ring buffer = timing race (report captured too early) |
| `nodes[].msAfterConnect` | When VerifiedNodeID arrived (ms after connect) | >2000ms = slow network or contention |
| `nodes[].snipQueryDurationMs` | SNIP round-trip time | >500ms on serial = possible contention; 0 = SNIP answered from cache or pre-existing |

## Step 4 — Interpret errors

| `error_type` | `phase` | Meaning | Likely cause |
|--------------|---------|---------|--------------|
| `"timeout"` | `"cdi-download"` | No Memory Config reply within 5s×3 attempts | Node not responding to datagrams; CAN bus issue; alias stale |
| `"protocol-error"` | `"cdi-download"` | Datagram rejected or malformed reply | Node firmware issue; wrong address space |
| `"timeout"` | `"config-read"` | Config field read timed out | Same causes as CDI timeout |
| `"timeout"` | `"snip-query"` | SNIP reply never arrived | Node offline or doesn't support SNIP |

## Step 5 — Decode frame activity

Call `decode_diagnostic_frames` with the `recentFrameActivity` array and (optionally) the `discovery.nodes` array as `knownNodes`.

If the MCP tool is unavailable, decode manually using this reference:

### Key frame identification (by header bits 28-24, i.e. first 2 hex digits of header)
| Top nibble (hex) | Frame type | Meaning |
|------------------|-----------|---------|
| `19` | Standard MTI (global or addressed) | Most protocol messages |
| `1A` | DatagramOnly | Single-frame datagram (CDI read request fits here: 7 bytes) |
| `1B` | DatagramFirst | Multi-frame datagram start |
| `1C` | DatagramMiddle | Multi-frame datagram continuation |
| `1D` | DatagramFinal | Multi-frame datagram end |
| `10`–`17` | CAN control (CID/RID/AMD/AME) | Alias negotiation / node startup |

### Common MTI values (header >> 12, for non-datagram frames)
| MTI value | Name | What it means |
|-----------|------|---------------|
| `0x19490` | Verify Node ID Global | Discovery probe |
| `0x19170` | Verified Node ID | Node announcing its identity |
| `0x19828` | Protocol Support Inquiry | PIP query |
| `0x19668` | Protocol Support Reply | PIP answer |
| `0x19DE8` | SNIP Request | Simple Node Info query |
| `0x19A08` | SNIP Response | Simple Node Info reply (multi-frame addressed) |
| `0x19A28` | Datagram Received OK | ACK for a datagram |
| `0x19A48` | Datagram Rejected | NAK for a datagram |

### What to look for in the ring buffer
- **TX datagram frames present but no RX datagram reply** → Node not responding to memory config
- **Only RX frames, no TX** → Bowties never sent requests (captured too early or connection issue)
- **TX VerifyNodeGlobal (0x19490)** → Discovery probe was sent
- **RX VerifiedNode (0x19170)** → Nodes responded to discovery
- **SNIP reply frames (0x19A08) with multi-frame flags** → SNIP data flowing correctly
- **No datagram frames at all** → Either CDI never attempted, or ring buffer only captured later traffic

### Ring buffer caveats
- Fixed size (last ~50 frames) — earlier activity is lost
- `timestampMs` counts **DOWN** from report capture time (higher = older)
- If all frames share the same `timestampMs`, they arrived in a burst
- Absence of frame types doesn't prove they were never sent — only not in the buffer window

### Addressed message body encoding
For standard addressed messages (MTI category `addressed`):
- Data bytes [0:1] = destination alias (big-endian, top 4 bits of byte 0 are flags)
- Remaining bytes = payload
- Multi-frame flag in byte 0: `0x10` = first, `0x30` = middle, `0x20` = last, `0x00` = only

### Datagram header encoding
For datagram frames (top nibble 0x1A–0x1D):
- Header bits [23:12] = destination alias
- Header bits [11:0] = source alias
- ALL data bytes are payload (no dest in body)

## Step 6 — Formulate response

### If phase = "just connected" (too early)
Tell the user:
> "This diagnostic was captured immediately after connecting, before any CDI
> download was attempted. To diagnose the read failure, please:
> 1. Connect to the serial port
> 2. Wait for nodes to appear in the sidebar
> 3. Click a node to trigger CDI download
> 4. Wait for the error/timeout message to appear
> 5. THEN copy the diagnostic report (⋮ menu → Copy Diagnostic Report)"

### If phase = "CDI failed"
Analyze the error entries and frame buffer to determine:
- Were datagram request frames sent? (TX with header top nibble 0x1A–0x1D)
- Were DatagramReceivedOk (0x19A28) frames received? (means node ACK'd the request)
- Were any datagram reply frames received? (RX with matching aliases)
- Was DatagramReceivedOk sent but no reply datagram followed? (node processing timeout)

### If comparing serial vs TCP
Key differences:
- TCP: GridConnect over TCP socket (reliable byte stream, no flow control issues)
- Serial: GridConnect over USB-serial (depends on baud rate, flow control, adapter firmware)
- Same protocol code in Bowties for both — differences are at transport level

### If comparing multiple reports
Look for patterns:
- Same failure across different nodes → transport/bus issue, not node-specific
- Failure with one node but not others → node firmware or config issue
- Works via TCP hub but not serial → SPROG adapter or serial config issue

## Step 7 — Suggest next actions

Based on findings, suggest one of:
| Finding | Suggestion |
|---------|------------|
| Report too early | Capture post-failure diagnostic |
| CDI timeout, no datagrams in buffer | Capture with larger ring buffer or JMRI trace |
| CDI timeout, request sent, no reply | Test same node via TCP; check SPROG firmware |
| Serial works for SNIP but not CDI | Possible multi-frame datagram issue at adapter level |
| Everything works | No issue — inform user the session was healthy |
