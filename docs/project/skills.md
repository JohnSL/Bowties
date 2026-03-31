# Skills

This document describes the Copilot skills available in the Bowties project and how to use them.

Skills are `.github/skills/<name>/SKILL.md` files that Copilot loads automatically when you ask a question that matches the skill's domain. They give the AI structured workflows, tool-call sequences, and domain knowledge that make answers significantly more accurate — especially for protocol-specific tasks like LCC trace analysis.

## Available skills

| Skill | When it activates |
|---|---|
| [trace-analysis](#trace-analysis) | You paste or reference an LCC/JMRI CAN Monitor trace |

---

## trace-analysis

Analyse LCC/OpenLCB message traces captured from JMRI's LCC Monitor window.

### What it does

The skill uses a local MCP server (`tools/trace-mcp`) to decode raw CAN frame headers, map node aliases to NodeIDs, group all frames into typed logical interactions, pre-compute timing gaps, and decode payloads for these protocols:

| Protocol | What gets decoded |
|---|---|
| Memory Configuration | Read/write address, address space, data bytes, request/reply timing |
| SNIP | Node manufacturer, model, name, description |
| PIP | Supported protocol list |
| Traction | Speed, direction, function number |
| Event exchange | PCER, Producer/Consumer Identified, range events |
| Alias negotiation | CID/RID sequence, alias assignment |

It handles **partial traces** (JMRI's monitor holds only ~500 lines): even mid-session fragments can be analysed because JMRI's own decoded text column provides node identity information when boot frames are absent.

### Setup

**1. Install dependencies**

```bash
cd tools/trace-mcp
npm install
npm run build
```

**2. Register the MCP server in VS Code**

Add to your `.vscode/mcp.json` (create if it doesn't exist):

```json
{
  "servers": {
    "lcc-trace": {
      "type": "stdio",
      "command": "node",
      "args": ["${workspaceFolder}/tools/trace-mcp/dist/index.js"]
    }
  }
}
```

After saving, the LCC trace tools appear automatically in Copilot Chat's tool list.

**3. Configure JMRI**

In the JMRI LCC Monitor window, enable:

| Setting | Why |
|---|---|
| ✓ Show timestamps | Needed for timeline ordering |
| ✓ Show raw data | Primary source for all decoding |
| ✓ Show Name for Node | Adds human names below addressed frames |
| ✓ Event Name | Appends event name on PCER lines |
| ✗ Event Uses | Not needed — leave off |

### Getting a trace into Copilot

Three ways to provide the trace:

**Paste directly** — Select All in the JMRI monitor (Ctrl+A), copy, paste into the chat alongside your question.

**From clipboard** — Copy from JMRI, then ask your question without pasting. Copilot will call `read_clipboard()` automatically.

**From a file** — Use JMRI's "Save to file" button, then mention the path in your question (e.g. "analyse the trace at C:\traces\session.log").

### Example questions

```
What are the nodes in this trace?
```
→ Calls `summarize_session()` — lists every node with its NodeID and alias, plus `groupCounts` showing how many of each interaction type (memory-config, snip, pip, events, …) are present.

---

```
What is node 09.00.99.05.01.C0 reading from the configuration space?
```
→ Calls `list_groups(type:"memory-config", nodeId:"09.00.99.05.01.C0")` and shows each operation with address, space, data bytes, and pre-computed timing (request→ACK, ACK→reply, reply→ACK).

---

```
Why is Bowties slower than JMRI when reading configuration?
```
→ Calls `list_groups(type:"memory-config")` and compares `timing.requestToAckMs` and `timing.ackToReplyMs` across operations from each client.

---

```
Show me the SNIP exchanges in this trace.
```
→ Calls `list_groups(type:"snip")` — each row covers the full request+multi-frame-response with `requestToReplyMs` timing and the node name extracted from the response.

---

```
What does this datagram mean: 20 41 00 00 02 64 08
```
→ Calls `decode_datagram("20 41 00 00 02 64 08")` — context-free, no trace needed.
→ Answer: Memory Config / Read space=0xFF, address 0x00000264, 8 bytes requested.

---

```
Show me the startup sequence for the node with alias 0x825.
```
→ Calls `timeline(nodeId:"0x825")` or `list_groups(nodeId:"0x825")` and explains the CID/RID/Init/Verify/Identify sequence.

---

```
Why is the write to node X failing?
```
→ Calls `list_groups(type:"memory-config", nodeId:"…")`, looks for rows with `fields.errorCode` or `complete: false`, and reports the error.

### CLI usage

The tool also includes a command-line interface for quick inspection without Copilot:

```bash
# From the tools/trace-mcp directory after npm run build:

node dist/cli.js path/to/trace.log --summary
node dist/cli.js path/to/trace.log --memory-ops --space 0xFD
node dist/cli.js path/to/trace.log --timeline --node 09.00.99.05.01.C0
node dist/cli.js --clipboard --summary
```

### MCP tools reference

| Tool | Input | What it returns |
|---|---|---|
| `read_trace_file` | file path | loads trace from disk, caches it |
| `read_clipboard` | — | loads trace from clipboard, caches it |
| `load_trace` | trace text | loads pasted trace text, caches it |
| `parse_line` | one trace line | structured fields (header, MTI, aliases, bytes) |
| `decode_frame` | one trace line | human-readable one-line description |
| `summarize_session` | — | node map, message counts, interaction `groupCounts`, boot detection |
| `list_groups` | `type?`, `nodeId?` (≥1 required) | typed interaction rows with pre-computed timing and `frameIndices` |
| `get_frames` | `frameIndices[]` | raw frame details for drill-down after `list_groups` |
| `decode_datagram` | hex bytes string | protocol, command, decoded fields (context-free) |
| `timeline` | optional `nodeId` | per-node chronological frame list |

**Progressive discovery pattern**: `summarize_session` → `list_groups(type, nodeId)` → `get_frames(frameIndices)`
