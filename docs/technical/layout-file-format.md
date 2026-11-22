# Layout File Format

A layout is a **folder**, not a single file. This document describes the
folder, its version markers and the version policy. The sections after
"Legacy single-file format" describe the pre-folder `.bowties.yaml` file and
are kept for reference only.

## Layout folder

| File | Holds | Version marker |
|------|-------|----------------|
| `manifest.yaml` | Layout identity and the folder's schema version | `schemaVersion` (integer), checked on open |
| `bowties.yaml` | Bowtie names, tags and role classifications | — |
| `event-names.yaml` | User event names | — |
| `channels.yaml` | Information Channels | `schemaVersion` `"2.0"`, not checked on open |
| `facilities.yaml` | Facilities, slot bindings, and the Automatic Block Signals record (`blockSignals`) | `schemaVersion` `"2.0"`, not checked on open |
| `offline-changes.yaml` | Offline Changes waiting for the Sync Panel | — |
| `nodes/` | Per-node snapshots | — |
| `cdi/` | Cached CDI files | — |

File names are defined in `bowties-core/src/layout/io.rs`. Field names are
camelCase. Facilities without an Automatic Block Signals record omit the
`blockSignals` key.

## Version policy (ADR-0005, 2026-11-22 extension)

- `manifest.yaml` `schemaVersion` is `LAYOUT_SCHEMA_VERSION` (currently **5**)
  in `bowties-core/src/layout/manifest.rs`. `OLDEST_SUPPORTED_LAYOUT_SCHEMA_VERSION`
  is **4**.
- Opening a layout accepts any manifest version from the oldest supported one
  up to the current one. A newer version is refused with a message that the
  layout was made by a newer version of Bowties. Versions older than 4 are
  refused as unsupported.
- Bump the manifest version, and the affected document's `schemaVersion`,
  for any change an older Bowties would mishandle, including new fields:
  serde ignores unknown fields, so an older build would drop the new data on
  its next save.
- No migration code while Bowties is pre-1.0 unless a slice needs one. Older
  documents must still deserialize through serde defaults.
- Tests: `bowties-core/src/layout/manifest.rs` (unit) and
  `bowties-core/tests/block_signals_layout_roundtrip.rs` (version 4 opens,
  version 6 refused through the real open path, version 5 roundtrip).

| Manifest version | Change |
|------------------|--------|
| 5 | Facilities may carry an Automatic Block Signals record; `facilities.yaml` `schemaVersion` "2.0" (Spec 020 E1a) |
| 4 | Oldest version this build opens |

---

## Legacy single-file format

Historical: the pre-folder `.bowties.yaml` format. Current builds do not
write it.

### File extension

`.bowties.yaml`

The open-file dialog also accepts bare `.yaml` / `.yml`, but **Save As** always defaults to `.bowties.yaml`.

### Serialization

YAML, written by the Rust [`serde_yaml_ng`](https://crates.io/crates/serde_yaml_ng) crate. Field names use **camelCase** throughout (enforced by `#[serde(rename_all = "camelCase")]` on all Rust types).

Saves use an atomic write strategy: content is written to a `.yaml.tmp` sibling file, flushed, then renamed over the target path to prevent corruption on crash or power loss.

---

### Top-level structure

```yaml
schemaVersion: '1.0'
bowties:
  <event-id-or-placeholder>:
    name: <string>       # optional
    tags: []
roleClassifications:
  <node-id>:<element-path>:
    role: Producer       # or Consumer
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schemaVersion` | `string` | yes | Schema version; must be `"1.0"` |
| `bowties` | mapping | yes | One entry per bowtie; see [Bowties mapping](#bowties-mapping) |
| `roleClassifications` | mapping | yes | Manual Producer/Consumer overrides; see [Role classifications](#role-classifications) |

---

### `schemaVersion`

Always `"1.0"`. The Rust loader rejects any other value with an error before returning the layout to the frontend.

---

### Bowties mapping

Each key in `bowties` identifies a bowtie. Two key formats are valid:

#### Dotted-hex event ID key

Exactly 8 two-hex-digit groups separated by `.`:

```
02.01.57.00.02.D9.02.F6
```

Used for bowties that have been wired to a real LCC event ID (either read from a node or assigned by the user).

#### Planning placeholder key

`planning-<digits>`, where `<digits>` is a numeric timestamp:

```
planning-1774216908886
```

Used for bowties that exist in the diagram but are not yet associated with a real event ID. The timestamp is the Unix millisecond epoch at creation time.

#### Bowtie value (`BowtieMetadata`)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | no | User-assigned display name; omitted from YAML when not set |
| `tags` | `string[]` | yes | List of user-assigned tag strings; `[]` when empty |

```yaml
bowties:
  02.01.57.00.02.D9.02.F6:
    name: T1 Select Main
    tags:
      - turnouts
      - mainline
  02.01.57.00.02.D9.03.02:
    tags: []
  planning-1774216908886:
    name: B1 Occupied
    tags: []
```

---

### Role classifications

The `roleClassifications` mapping stores manual overrides for event slots that the automatic Producer/Consumer classifier could not resolve unambiguously. It is empty (`{}`) for most layouts.

#### Key format

`"{nodeId}:{elementPath}"` where `elementPath` is the slash-joined path through the CDI tree to the EventID field:

```
05.02.01.02.03.00:Port I/O/Line #1/Event Produced
```

#### Value (`RoleClassification`)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `role` | `"Producer"` \| `"Consumer"` | yes | The user's explicit role assignment for this slot |

```yaml
roleClassifications:
  05.02.01.02.03.00:Port I/O/Line #1/Event Produced:
    role: Producer
  05.02.01.02.03.00:Port I/O/Line #2/Event:
    role: Consumer
```

The loader rejects any `role` value other than `"Producer"` or `"Consumer"`.

---

### Complete example

```yaml
schemaVersion: '1.0'
bowties:
  02.01.57.00.02.D9.02.F6:
    name: T1 Select Main
    tags: []
  02.01.57.00.02.D9.03.02:
    name: T1 Select Siding
    tags: []
  planning-1774216908886:
    name: B1 Occupied
    tags: []
roleClassifications: {}
```

---

### Recent-layout sidecar

Bowties stores the path of the most recently opened layout in a separate JSON sidecar file at:

```
<app-data-dir>/recent-layout.json
```

This file is **not** part of the layout format itself — it is an application state file and is never shared with other users.

```json
{
  "path": "/home/user/layouts/mytrack.bowties.yaml",
  "lastOpened": "2026-03-23T18:00:00Z"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `path` | `string` | Absolute path to the last-opened layout file |
| `lastOpened` | `string` | ISO 8601 UTC timestamp of when it was last opened |

---

### Source references

| Purpose | File |
|---------|------|
| TypeScript types | [app/src/lib/types/bowtie.ts](../../app/src/lib/types/bowtie.ts) |
| Frontend layout store | [app/src/lib/stores/layout.svelte.ts](../../app/src/lib/stores/layout.svelte.ts) |
| Frontend bowtie metadata store | [app/src/lib/stores/bowtieMetadata.svelte.ts](../../app/src/lib/stores/bowtieMetadata.svelte.ts) |
| Rust layout types | [app/src-tauri/src/layout/types.rs](../../app/src-tauri/src/layout/types.rs) |
| Rust file I/O (load/save) | [app/src-tauri/src/layout/io.rs](../../app/src-tauri/src/layout/io.rs) |
| Tauri commands | [app/src-tauri/src/commands/bowties.rs](../../app/src-tauri/src/commands/bowties.rs) |
