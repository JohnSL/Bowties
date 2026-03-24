# Layout File Format

Layout files store the user's bowtie diagram — event ID assignments, names, tags, and any manual role-classification overrides. They are the primary save artifact of Bowties.

## File extension

`.bowties.yaml`

The open-file dialog also accepts bare `.yaml` / `.yml`, but **Save As** always defaults to `.bowties.yaml`.

## Serialization

YAML, written by the Rust [`serde_yaml_ng`](https://crates.io/crates/serde_yaml_ng) crate. Field names use **camelCase** throughout (enforced by `#[serde(rename_all = "camelCase")]` on all Rust types).

Saves use an atomic write strategy: content is written to a `.yaml.tmp` sibling file, flushed, then renamed over the target path to prevent corruption on crash or power loss.

---

## Top-level structure

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

## `schemaVersion`

Always `"1.0"`. The Rust loader rejects any other value with an error before returning the layout to the frontend.

---

## Bowties mapping

Each key in `bowties` identifies a bowtie. Two key formats are valid:

### Dotted-hex event ID key

Exactly 8 two-hex-digit groups separated by `.`:

```
02.01.57.00.02.D9.02.F6
```

Used for bowties that have been wired to a real LCC event ID (either read from a node or assigned by the user).

### Planning placeholder key

`planning-<digits>`, where `<digits>` is a numeric timestamp:

```
planning-1774216908886
```

Used for bowties that exist in the diagram but are not yet associated with a real event ID. The timestamp is the Unix millisecond epoch at creation time.

### Bowtie value (`BowtieMetadata`)

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

## Role classifications

The `roleClassifications` mapping stores manual overrides for event slots that the automatic Producer/Consumer classifier could not resolve unambiguously. It is empty (`{}`) for most layouts.

### Key format

`"{nodeId}:{elementPath}"` where `elementPath` is the slash-joined path through the CDI tree to the EventID field:

```
05.02.01.02.03.00:Port I/O/Line #1/Event Produced
```

### Value (`RoleClassification`)

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

## Complete example

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

## Recent-layout sidecar

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

## Source references

| Purpose | File |
|---------|------|
| TypeScript types | [app/src/lib/types/bowtie.ts](../../app/src/lib/types/bowtie.ts) |
| Frontend layout store | [app/src/lib/stores/layout.svelte.ts](../../app/src/lib/stores/layout.svelte.ts) |
| Frontend bowtie metadata store | [app/src/lib/stores/bowtieMetadata.svelte.ts](../../app/src/lib/stores/bowtieMetadata.svelte.ts) |
| Rust layout types | [app/src-tauri/src/layout/types.rs](../../app/src-tauri/src/layout/types.rs) |
| Rust file I/O (load/save) | [app/src-tauri/src/layout/io.rs](../../app/src-tauri/src/layout/io.rs) |
| Tauri commands | [app/src-tauri/src/commands/bowties.rs](../../app/src-tauri/src/commands/bowties.rs) |
