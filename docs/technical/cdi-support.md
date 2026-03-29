# CDI Support Coverage

This document tracks which parts of the OpenLCB CDI (Configuration Description Information)
specification (S-9.7.4.1) are supported by the Bowties app, and which are not yet implemented.

---

## Supported

### Document structure

| Element | Attributes / children | Notes |
|---------|----------------------|-------|
| `<cdi>` | root | — |
| `<identification>` | manufacturer, model, hardware_version, software_version | Parsed; used for CDI file-cache key and node-list label |
| `<acdi>` | *(presence flag)* | Recorded; see [ACDI fetch gap](#acdi-address-space-fetch) |
| `<segment>` | space, origin, name, description | Multiple segments per node are supported |

### Groups

| Feature | Detail |
|---------|--------|
| Nesting | Unlimited depth |
| Replication | `replication` attribute; N instances with auto-computed offsets |
| Instance labels | `<repname>` template prefix + instance number |
| `GroupHints` — hideable | Group can be collapsed/expanded by the user |
| `GroupHints` — hiddenByDefault | Initial collapsed state when hideable |
| `GroupHints` — readOnly | All fields in the group rendered as read-only |
| Empty-group filtering | Footnote 4: groups with no renderable children are dropped |

### Leaf elements — parse + tree build + read + write

#### `<int>`

| Feature | Detail |
|---------|--------|
| Sizes | 1, 2, 4, 8 bytes (big-endian two's-complement signed) |
| Constraints | min, max, default (pre-populated before first read) |
| Map | Value↔label enum; rendered as select dropdown |
| Reserved-value handling | When current value is absent from map, a disabled "(Reserved: N)" option is shown |
| Hint — slider | `immediate`, `tickSpacing`, `showValue`; rendered as `<input type="range">` |
| Hint — radiobutton | Rendered as a radio-button group when map is also present |

#### `<string>`

| Feature | Detail |
|---------|--------|
| Size | N bytes (N−1 usable characters + null terminator) |
| Default | Pre-populated in tree when specified |
| Read / Write | Datagram read and write |

#### `<eventid>`

| Feature | Detail |
|---------|--------|
| Size | Always 8 bytes |
| Read / Write | Datagram read and write; dotted-hex input with validation |
| Event-role display | Producer / Consumer tags sourced from node profile |
| Bowtie linking | Event IDs can be linked to Bowtie diagrams |

#### `<float>`

| Feature | Detail |
|---------|--------|
| Size 2 | IEEE 754 half-precision (f16) — encode and decode |
| Size 4 | IEEE 754 single-precision (f32) — encode and decode |
| Size 8 | IEEE 754 double-precision (f64) — encode and decode |
| Constraints | min, max, default (pre-populated before first read) |
| Read / Write | Datagram read and write |

#### `<action>`

| Feature | Detail |
|---------|--------|
| Sizes | 1, 2, 4, 8 bytes |
| `value` | Payload written to node on trigger |
| `buttonText` | Custom label shown on the trigger button; defaults to "Trigger" |
| `dialogText` | When present, a `window.confirm` dialog is shown before triggering |
| Trigger | `trigger_action` datagram write |

#### `<blob>`

| Feature | Detail |
|---------|--------|
| Parse | Size and offset parsed; cursor arithmetic is correct for elements that follow |
| Metadata display | Name, description, address, and size shown in the element detail card |
| Read / Write | **Not implemented** — see [Blob I/O gap](#blob-interactive-io) |

### Config I/O

| Operation | Detail |
|-----------|--------|
| Read single value | `read_config_value` Tauri command — datagram read |
| Write single value | `write_config_value` Tauri command — datagram write |
| Trigger action | `trigger_action` Tauri command — datagram write |
| Bulk read | `read_plan` / `start_bulk_read` — sequential datagram reads across all segments in one pass |
| Default pre-population | CDI-specified defaults shown in the tree before the first read |

---

## Not yet supported

### Blob interactive I/O

`<blob>` nodes are structurally tracked (size + address are correct) and shown in the
element detail panel, but their memory contents are never read from the node or written
back. There is no UI control for viewing or editing raw binary data.

**Impact:** Nodes that use blob elements for firmware parameters or certificates will show
those fields as display-only placeholders with no value.

---

### `<bit>` element

The CDI specification and the Java reference implementation (`BitRep`) define a `<bit>`
element for single-bit boolean flags. The Rust parser silently ignores any XML tags it
does not recognise (forward-compatibility behaviour), so `<bit>` elements are dropped
without error.

**Impact:** Any node CDI that uses `<bit>` will silently lose those fields from the
configuration tree.

---

### `<link>` element

Each CDI item may include a `<link>` child containing a URL pointing to external
documentation for that field. The parser silently ignores `<link>` tags; no URL is
extracted or rendered as a help link in the UI.

**Impact:** Help-link annotations present in CDI XML are not surfaced to users.

---

### `<identification>` — `<map>` child

The spec allows a `<map>` inside `<identification>` for locale-sensitive translations of
the manufacturer or model name strings. Only plain string values are stored;
the translation map is not parsed.

**Impact:** Negligible in practice — no known real-world CDI uses this feature.

---

### `<identification>` version display in config editor

hardware_version and software_version are parsed from `<identification>` but are not
displayed anywhere in the configuration editor. Manufacturer and model are shown only in
the node list (via SNIP) and as part of the CDI file-cache filename.

**Impact:** Users cannot see the firmware/hardware version from within the configuration
editor itself.

---

### ACDI address-space fetch

When `<acdi/>` is present in the CDI, the node exposes a compact name and description
in address space 0xFC. The parser notes the presence of the `<acdi>` flag but the app
never issues reads from address space 0xFC. Node name and description are sourced from
SNIP (Simple Node Information Protocol) instead.

**Impact:** On nodes where SNIP strings are empty but ACDI data is populated, the node
name will fall back to the raw node ID rather than the ACDI-provided name.

---

### Float and int hints on non-integer element types

Rendering hints (`<hints>`) are parsed for `<int>` and `<group>` only. If a future CDI
revision adds hints to `<float>`, `<string>`, or `<eventid>`, they will be silently
ignored.

---

## Summary table

| Feature | Status |
|---------|--------|
| `<identification>` parsed | ✅ |
| `<identification>` versions displayed in editor | ❌ |
| `<acdi>` flag recognised | ✅ |
| `<acdi>` address-space fetch | ❌ |
| Multiple `<segment>` | ✅ |
| `<group>` nesting + replication | ✅ |
| `<group>` hideable / readOnly hints | ✅ |
| `<int>` read / write | ✅ |
| `<int>` slider hint | ✅ |
| `<int>` radiobutton hint | ✅ |
| `<int>` reserved-value dropdown | ✅ |
| `<string>` read / write | ✅ |
| `<eventid>` read / write | ✅ |
| `<float>` f16 / f32 / f64 read / write | ✅ |
| `<action>` trigger | ✅ |
| `<blob>` cursor tracking | ✅ |
| `<blob>` read / write | ❌ |
| `<bit>` element | ❌ |
| `<link>` URL | ❌ |
| `<identification><map>` | ❌ |
