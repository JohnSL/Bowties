---
name: trace-analysis
description: >
  Analyse LCC/OpenLCB message traces captured from JMRI's LCC Monitor window.
  Decodes CAN frame headers, identifies message types, resolves node aliases,
  reassembles datagrams, and decodes memory config / SNIP / PIP / traction
  payloads. Handles partial traces (JMRI's ~500-line buffer) gracefully.
  Keywords: LCC, OpenLCB, CAN, trace, JMRI monitor, datagram, memory config,
  SNIP, PIP, event, alias, decode, analyse.
---

# LCC Trace Analysis Skill

## When to invoke this skill

Load this skill whenever the user:
- Pastes or references a JMRI CAN Monitor trace (lines matching `HH:MM:SS.mmm: [[…] …]  R|S: …`)
- Asks "what is node X doing?", "what is being read/written?", "why is node Y not responding?"
- Wants to understand LCC message sequences, event producers/consumers, or datagram contents

## Step 0 — Acquire the trace

The user may supply the trace in one of three ways:

| How they give it | What to do |
|---|---|
| Pasted inline in chat | Call `load_trace(trace)` with the text |
| "I've copied it from JMRI" | Call `read_clipboard()` — loads and caches the trace |
| Saved to a file | Call `read_trace_file(path)` — loads and caches the trace |

If no trace has been provided and the user asks an analysis question, ask:
> "Please paste the trace from JMRI, or say 'I've copied it' and I'll read your clipboard."

## Step 1 — Orient with summarize_session

Always call `summarize_session()` first (the trace is already cached from Step 0). This gives:
- A confirmed or inferred alias→NodeID map
- Message counts per node — quick way to spot who is most active
- Whether the trace starts at boot (`hasBootFrames`) or is a mid-session fragment

**Critical for partial traces**: if `hasBootFrames: false`, the node map was built
from JMRI's decoded text column alone. This is normally reliable, but note it to the user:
> "The trace doesn't include node boot — node identities are inferred from JMRI labels."

## Step 2 — Route to the right tool

Use the node map from Step 1 and the question type to select the right tool.
For most analysis questions, use the **three-tier progressive discovery pattern**:
1. `list_groups(type, nodeId)` — server-side filtered summary rows with pre-computed timing
2. If you need raw frame bytes or JMRI decoded text: `get_frames(frameIndices)` — drill-down

### Tool routing table

| Question | Primary tool | Notes |
|---|---|---|
| "What is node X reading or writing?" | `list_groups(type:"memory-config", nodeId:"…")` | Requires ≥1 filter; returns timing |
| "How fast does node X respond?" | `list_groups(type:"memory-config", nodeId:"…")` | Check `timing.requestToAckMs`, `ackToReplyMs` |
| "Show me SNIP/PIP/Verify exchanges" | `list_groups(type:"snip"/"pip"/"verify")` | Each row includes request+reply frames |
| "What events does node X produce?" | `list_groups(type:"event", nodeId:"…")` | Covers PCER and Learn Event |
| "Show me all interactions for node X" | `list_groups(nodeId:"…")` | All types for one node |
| "Show me all memory-config operations" | `list_groups(type:"memory-config")` | All nodes |
| "What is alias 0x646?" | `summarize_session()` → nodes map | Already done in Step 1 |
| "Show me what node X did in order" | `timeline(nodeId)` | Individual frames, not grouped |
| "What does this byte string mean?" | `decode_datagram(bytesHex)` | Context-free, no trace needed |
| "What is this specific line?" | `decode_frame(line)` or `parse_line(line)` | Context-free, no trace needed |
| "Show me the raw bytes for these frames" | `get_frames(frameIndices)` | Drill-down after list_groups |

### Progressive discovery pattern

**Start broad, then narrow:**
```
summarize_session()                          # orientation — groupCounts shows what's present
  → list_groups(type:"memory-config")        # all memory config, all nodes
  → list_groups(type:"memory-config", nodeId:"09.00.99.05.01.C0")  # one node only
  → get_frames([12, 13, 14, 15])             # drill into specific interaction frames
```

**Important**: `list_groups` requires at least one filter (`type` or `nodeId`).
Calling it with no filters returns an error. Use `summarize_session()` first to see
the `groupCounts` and understand what's in the trace before filtering.

### Timing analysis (Bowties vs JMRI latency)

`list_groups` pre-computes four timing gaps for `memory-config` interactions:
- `requestToAckMs`: request datagram last frame → Datagram Received OK from responder  
  *(transport ACK — how fast does the node acknowledge receipt?)*
- `ackToReplyMs`: ACK → reply datagram first frame  
  *(processing time — how fast does the node start sending the reply?)*
- `replyToAckMs`: reply last frame → Datagram Received OK from requester  
  *(reverse transport ACK — how fast does the client acknowledge the reply?)*
- `gapToNextMs`: last frame of this interaction → first frame of next same-type same-src  
  *(inter-request gap — how fast does the client send the next request?)*

To compare Bowties vs JMRI response latencies, call `list_groups(type:"memory-config")`
and compare `requestToAckMs` and `ackToReplyMs` across interactions from each client.

### "What does this byte string mean?"
→ `decode_datagram(bytesHex)`
- Context-free: paste any hex string (e.g. `20 41 00 00 02 64 08`)
- Returns protocol name, command, address, space, data

### "What is this specific line?"
→ `decode_frame(line)` or `parse_line(line)`
- `decode_frame` gives a one-line human description
- `parse_line` gives the structured fields (use when you need the raw numbers)

### "Which events does node X produce/consume?"
→ `list_groups(type:"event", nodeId:"…")` for PCER/Learn Event  
→ `list_groups(type:"event-identification", nodeId:"…")` for Identify Events exchanges

## Partial trace guidance

JMRI's monitor buffer holds approximately 500 lines. Traces are often fragments
from the middle of a session. Keep in mind:

- **Alias map**: always check `nodes[alias].confidence` in the summary. `inferred`
  means JMRI labelled the frames but no boot frame was captured — usually correct.
- **Datagrams**: `complete: false` on a `list_groups` row means either the First or
  Final datagram frame was outside the window. The available bytes are still decoded.
- **Partial interaction**: `complete: false` on any row means a boundary frame is missing.
  The `frameIndices` still point to whatever was captured for that interaction.
- **Don't say "can't determine"** unless the relevant frames are simply absent. Work
  with what is in the trace and note limitations explicitly.

## Answering specific question types

### Memory read/write diagnosis

1. `list_groups(type:"memory-config", nodeId:"<node>")` to list operations for a node
2. Each result row includes: `summary`, `fields.command`, `fields.address`, `fields.addressSpace`, `fields.dataBytes`, and pre-computed `timing`
3. Call `get_frames(frameIndices)` on any row to see the raw CAN bytes
4. Look for `fields.errorCode` in the reply — this indicates the write was rejected
5. Compare `timing.ackToReplyMs` values across operations to spot slow reads

### Node startup sequence

1. `timeline(nodeId)` for the node
2. Look for: CID 4-7 frames → RID → Initialization Complete → Verify Node ID Global
   → Verified Node ID → Identify Events → SNIP exchange
3. Or call `list_groups(nodeId:"…")` to see all interaction types in one view
4. If only a subset appears, note where the trace window starts

### Event dispatch

1. `list_groups(type:"event", nodeId:"…")` for PCER events from a node
2. Note EventID, sender, and timing
3. `list_groups(type:"event-identification")` for Identify Events / Identified responses

### Error diagnosis

Look for these in `list_groups` or `timeline`:
- `complete: false` on a memory-config row — reply datagram missing or truncated
- `fields.errorCode` present — node rejected a read/write with an error
- `type:"datagram-ack"` rows with `summary: "Datagram Received Rejected"` — transport rejection
- `type:"other"` rows with summary `Optional Interaction Rejected` or `Terminate Due to Error`
- `Optional Interaction Rejected` — a node declined an interaction; payload has error code
- `Terminate Due to Error` — fatal error; payload has error code
- `Datagram Rejected` — the destination node rejected a datagram; check the error flags
- `Write Reply (Fail)` / `Read Reply Fail` in a memory config row — operation not accepted

## JMRI monitor recommended settings

For best trace quality, enable in the "LCC Monitor" window:
- ✓ Show timestamps
- ✓ Show raw data
- ✓ Show Name for Node *(adds  Src: / Dest: lines — parsed automatically)*
- ✓ Event Name *(appends the event name on PCER lines)*
- ✗ Event Uses *(not parsed; leave off)*

## Output conventions

When presenting analysis results to the user:
- Use NodeIDs (dotted hex e.g. `09.00.99.05.01.C0`) rather than raw aliases
- Present memory addresses as `0x00000264` (8-digit hex)
- Present address spaces by name when known: `0xFD (Configuration)`, `0xFF (CDI)`
- Note `confidence` only when it is `partial` or `inferred` — omit it when `full`
- For partial datagrams, say "incomplete datagram (First/Final frame not in trace window)"
  rather than just showing the bytes
