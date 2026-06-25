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

## Tool requirement

Always use the `lcc-trace` MCP server tools for frame decoding and diagnostic analysis:
- `analyze_diagnostic` — pass the full report JSON for structured interpretation
- `decode_diagnostic_frames` — pass `recentFrameActivity` for frame-by-frame decoding
- `decode_frame` — decode individual frames when investigating specific messages

Use `tool_search` to load these tools before calling them. Only fall back to the manual
reference tables below if a tool call returns an error (not merely because the tables exist).

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

Only fall back to the manual reference tables in Step 2–5 if the MCP tool returns an error.

## Step 2 — Determine session phase (manual fallback)

### UX context for CDI vs config reads

"Read Configuration" is the user-facing action (button or sidebar dot). Internally it is a two-phase workflow:
1. **CDI XML download** — fetches the node's configuration schema (tracked in `cdiDownloads`). Skipped when the CDI is already cached.
2. **Config value read** — reads actual field values from node memory using the CDI schema (tracked in `configReads`).

The user never explicitly triggers "CDI download" — it happens automatically as a prerequisite of "Read Configuration". Discovery only queries SNIP/PIP metadata; clicking a node in the sidebar only selects it.

### Sidebar node indicators

The sidebar shows different indicators depending on node state. These help interpret what the user describes:

| Node state | Sidebar indicator | Meaning |
|---|---|---|
| Discovered but not yet saved in layout | `[NEW]` amber pill badge | Node found on bus but not persisted in saved layout |
| Saved in layout, config not yet read (online) | Amber clickable dot | Click triggers Read Configuration for that node |
| Config values have been read | Clean (no badge) | Fully read |
| Unsaved in-memory edits | Amber `pending-edits` dot | User changed values but hasn't saved |
| Saved offline edits pending apply | Teal `pending-apply` dot | Offline edits ready to write to node |

**Key:** `[NEW]` suppresses the not-read dot. A freshly discovered node shows `[NEW]`, never the dot. The dot only appears for nodes already persisted in the saved layout whose config values haven't been read this session (e.g., after reopening a saved layout and reconnecting).

Neither indicator checks CDI cache status — CDI availability is invisible to the sidebar and handled internally when Read Configuration runs.

### Phase table

| Condition | Phase | Implication |
|-----------|-------|-------------|
| `cdiDownloads: {}` + `errors: []` + `eventRoleExchange: null` | Just connected | Report captured before user clicked Read Configuration |
| `cdiDownloads: {}` + `errors` has `phase: "cdi-download"` entries | CDI XML download failed | Real failure during CDI fetch — analyze error details |
| `cdiDownloads` has entries + `configReads: {}` | CDI XML downloaded, config not started | CDI fetch succeeded but config values not yet read (user may not have completed the Read Configuration flow, or the read phase failed) |
| `cdiDownloads` has entries + `configReads` has entries | Normal session | Full Read Configuration flow completed successfully |
| `eventRoleExchange` is non-null but `cdiDownloads: {}` | Post-event-roles | Connected and synced but user hasn't clicked Read Configuration yet |

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

Only if the MCP tool returns an error, decode manually using this reference:

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
> "This diagnostic was captured before any configuration read was attempted.
> To diagnose the failure, please:
> 1. Connect to the serial port
> 2. Wait for nodes to appear in the sidebar
> 3. Click **Read Configuration** — either the button shown when you select
>    a node, or the **Read Remaining** toolbar button for all unread nodes.
>    (If the node shows a `[NEW]` badge, select it first, then click the
>    Read Configuration button that appears.)
> 4. Wait for the error/timeout message to appear
> 5. THEN copy the diagnostic report (⋮ menu → Copy Diagnostic Report)"
>
> Note: "Read Configuration" first downloads the CDI XML (if not cached),
> then reads config values. The diagnostic tracks both phases separately.

### If phase = "CDI XML download failed"
Analyze the error entries and frame buffer to determine:
- Were datagram request frames sent? (TX with header top nibble 0x1A–0x1D)
- Were DatagramReceivedOk (0x19A28) frames received? (means node ACK'd the request)
- Were any datagram reply frames received? (RX with matching aliases)
- Was DatagramReceivedOk sent but no reply datagram followed? (node processing timeout)

### If phase = "CDI done, config not started"
The CDI XML was fetched successfully but config value reads haven't happened or failed:
- If `errors` has `phase: "config-read"` entries → config read failed; analyze those errors
- If `errors` is empty → user may not have completed the flow, or report was captured between phases

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
| Report too early | Capture post-failure diagnostic (after clicking Read Configuration and seeing the error) |
| CDI XML timeout, no datagrams in buffer | Capture with larger ring buffer or JMRI trace |
| CDI XML timeout, request sent, no reply | Test same node via TCP; check adapter firmware |
| Config read timeout (CDI XML succeeded) | Node responds to CDI reads but not config reads — possible address-space issue |
| Serial works for SNIP but not CDI/config | Possible multi-frame datagram issue at adapter level |
| Everything works | No issue — inform user the session was healthy |
