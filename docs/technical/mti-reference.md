# MTI Quick Reference

*Message Type Indicator (MTI) values for LCC/OpenLCB protocol. For detailed protocol information, see [protocol-reference.md](protocol-reference.md).*

**Source:** TN-9.7.3 (Message Network Standard)

## General Messages (0x04xx - 0x06xx)

| MTI | Name | Direction | Purpose |
|-----|------|-----------|---------|
| `0x0490` | Verify Node ID (Global) | Broadcast | Discover all nodes on network |
| `0x0488` | Verify Node ID (Addressed) | To specific node | Check if node is online |
| `0x0170` | Verified Node ID | Response | Node responds with its 6-byte ID |
| `0x0171` | Verified Node ID (Simple) | Response | Simplified response format |
| `0x04A8` | Protocol Support Inquiry | Query | Ask what protocols node supports |
| `0x0668` | Protocol Support Reply | Response | List of supported protocols |

## Producer/Consumer Identified (0x05xx)

### Producer Responses

| MTI | Name | Purpose |
|-----|------|---------|
| `0x0544` | Producer Identified Valid | Event is configured and node produces it |
| `0x0545` | Producer Identified Invalid | Event slot exists but not configured |
| `0x0547` | Producer Identified Unknown | Cannot determine event state |

### Consumer Responses

| MTI | Name | Purpose |
|-----|------|---------|
| `0x04C4` | Consumer Identified Valid | Event is configured and node consumes it |
| `0x04C5` | Consumer Identified Invalid | Event slot exists but not configured |
| `0x04C7` | Consumer Identified Unknown | Cannot determine event state |

## Event Messages (0x09xx)

| MTI | Name | Purpose |
|-----|------|---------|
| `0x0997` | Identify Events (Global) | Query all events network-wide |
| `0x0968` | Identify Events (Addressed) | Query events from specific node |
| `0x095B4` | Producer/Consumer Event Report (PCER) | Node reports an event occurred |
| `0x0594` | Identify Producer | Ask "who produces this event?" |
| `0x04A4` | Identify Consumer | Ask "who consumes this event?" |

## Datagram Protocol (0x19xxx - 0x1Axxx)

### Datagram Send (0x1Axxx)

| MTI | Name | Purpose |
|-----|------|---------|
| `0x1A28` | Datagram (Complete) | Single-frame datagram |
| `0x1A48` | Datagram (First) | First frame of multi-frame datagram |
| `0x1A68` | Datagram (Middle) | Middle frame of multi-frame datagram |
| `0x1A88` | Datagram (Final) | Final frame of multi-frame datagram |

### Datagram Responses (0x19xxx)

| MTI | Name | Purpose |
|-----|------|---------|
| `0x19A28` | Datagram ACK OK | Successfully received datagram |
| `0x19A48` | Datagram NAK | Rejected or error receiving datagram |
| `0x19A68` | Datagram ACK Pending | Received but processing (rare) |

## Specific Datagram Types

### SNIP (Simple Node Information Protocol)

| MTI | Name | Purpose |
|-----|------|---------|
| `0x19DE8` | SNIP Request | Request node information |
| `0x19A08` | SNIP Response | Contains SNIP data (via datagram) |

### Memory Configuration

| MTI | Name | Purpose |
|-----|------|---------|
| `0x20A8` | Memory Config Read | Read from node memory |
| `0x20A1` | Memory Config Read Reply | Response with data |
| `0x20AA` | Memory Config Write | Write to node memory |
| `0x20AB` | Memory Config Write Reply | Confirmation |

### Configuration Description Information (CDI)

| MTI | Name | Purpose |
|-----|------|---------|
| `0x20A8` | Read CDI | Read CDI XML (memory space 0xFF) |
| `0x20A1` | CDI Data | Response with CDI fragment |

## Stream Protocol (0x1Fxxx, 0x18xxx)

*Currently not used in Bowties*

| MTI | Name | Purpose |
|-----|------|---------|
| `0x1F888` | Stream Init Request | Initiate stream connection |
| `0x1F868` | Stream Init Reply | Accept/reject stream |
| `0x1F8A8` | Stream Data Send | Stream data chunk |
| `0x18A88` | Stream Data Proceed | Flow control |
| `0x18A68` | Stream Data Complete | Stream finished |

## Initialization/Alias (0x10xxx)

*Low-level protocol initialization, typically handled by lcc-rs library*

| MTI | Name | Purpose |
|-----|------|---------|
| `0x10700` | Initialization Complete | Node alias allocation complete |
| `0x10701-10707` | AMD (Alias Mapping Definition) | Claim alias frames |

## MTI Bit Structure

**MTI Encoding (simplified):**

```
15  14  13  12  11  10   9   8   7   6   5   4   3   2   1   0
 │   │   │   │   │   │   │   │   │   │   │   │   │   │   │   │
 └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
     Priority        Message Type          Addressing

Priority: 0-3 (0=highest)
Message Type: Core function
Addressing: Simple/Addressed/Stream, etc.
```

**Common Patterns:**

- `0x04xx`: General protocol messages
- `0x05xx`: Event-related responses
- `0x09xx`: Event queries and reports
- `0x19xxx`: Datagram responses
- `0x1Axxx`: Datagram send

## Frame Examples

**Verify Node ID (Global):**
```
:X1949059BN;
  └─ MTI: 0x0490 (with source alias 0x59B embedded)
  └─ Data: None
```

**Verified Node ID Response:**
```
:X1917059BN050201021234;
  └─ MTI: 0x0170
  └─ Data: 05.02.01.02.12.34 (6-byte Node ID)
```

**SNIP Request:**
```
:X19DE859BN123;
  └─ MTI: 0x19DE8
  └─ Data: None (or optional flags)
  └─ Destination: Alias 0x123
```

**Producer/Consumer Event Report:**
```
:X195B459BN0502010200000003;
  └─ MTI: 0x095B4 (PCER)
  └─ Data: 05.02.01.02.00.00.00.03 (8-byte Event ID)
```

## MTI Usage in Bowties

**Currently Implemented:**
- ✅ `0x0490` - Node discovery (Verify Node ID Global)
- ✅ `0x0170` - Node responses (Verified Node ID)
- ✅ `0x19DE8` - SNIP requests
- ✅ `0x1A28/48/68/88` - SNIP datagram reception
- ✅ `0x19A28` - Datagram ACK
- ✅ `0x0968` - Identify Events (Addressed) — sent per node during bowtie build (Feature 006)
- ✅ `0x0544/0545/0547` - Producer Identified (Valid/Invalid/Unknown) — collected by bowtie build
- ✅ `0x04C4/04C5/04C7` - Consumer Identified (Valid/Invalid/Unknown) — collected by bowtie build
- ✅ `0x20A8/20A1` - Memory configuration read (CDI and config values)
- ✅ `0x095B4` - Event monitoring (PCER) — Protocol Monitor view
- ✅ `0x0997` - Identify Events (Global) — event discovery feature complete; addressed variant (`0x0968`) used in practice

**Planned:**
- ⏳ `0x20AA` - Memory configuration write

**Not Planned:**
- ❌ Stream protocol (out of scope)
- ❌ Firmware update (out of scope)
- ❌ Clock synchronization (out of scope)

---

*For protocol details and usage examples, see [protocol-reference.md](protocol-reference.md)*  
*For implementation, see [lcc-rs-api.md](lcc-rs-api.md)*
