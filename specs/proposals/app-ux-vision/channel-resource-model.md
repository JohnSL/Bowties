# Proposal: Channel Resource Model — Generalized Hardware Reference

**Status:** Draft proposal — captures the architectural pivot from Tower-LCC-shaped channel hardware references to a purpose-typed resource model. Companion to the channel-related sections of the UX vision.
**Origin:** Architecture discussion, June 2026. Triggered by the realization that the current `HardwareReference { connector, input }` shape (shipped in spec 015) cannot represent Signal LCC lines, LED drivers, or signal masts, and that the connector slug was leaking into channel display labels.
**Related:**
- [App UX Vision](./app-ux-vision.md) — workspaces, channel types, multi-pin channels
- [Behavior Templates & Information Channels](./behavior-templates-proposal.md) — facility-level templates that consume channels
- [App UX Vision Mockups](./app-ux-vision-mockups.html) — concrete UI illustrations referenced throughout

---

## Problem

The information-channel data model shipped in spec 015 encodes a channel's backing hardware as `HardwareReference { nodeKey, connector, input }`. That shape was modeled on the Tower-LCC's two-level addressing (named connectors that accept daughter boards, each daughter board exposing numbered inputs). It works for Tower-LCC with BOD-family daughter boards and it does not work for anything else:

- **Signal LCC** has 8 general-purpose I/O lines directly on the node plus 16 LED drivers and N firmware-defined signal masts. None of that fits a `(connector, input)` shape.
- **Standalone detector boards** (e.g., a future BOD-8 as its own LCC node) expose inputs directly, with no connector layer.
- **Output channels** — a single LED indicator on Signal LCC, or a 3-color signal aspect — are not "inputs" and don't fit the slug.
- **Display labels** today leak the storage form: `ChannelCard.svelte` renders `channel.hardwareRef.connector` as the literal slug `connector-a`.

The deeper problem is that the channel is currently tied to **how it's addressed on one specific board family**, rather than to **what it is**. That couples layer-3 (the layout-level channel concept) to layer-1 (the board's CDI shape), and there's no clean place to put hardware-shape diversity.

The generalization that emerged from the discussion: separate the layers, and let the channel see only as much as it needs.

---

## Three-Layer Model

```
LAYER 3 — Channel (in channels.yaml)
  { id, name, channelType, resourceRef: { nodeKey, resourceId } }
  Binds to one resource of the channel-type's required resource type.
  Knows: resource types and instance IDs. Nothing about fields, signatures, or hardware shape.

           ▲ binds-by-type

LAYER 2 — Resource (system-typed; provenance varies)
  System-shipped resource type catalog (purpose-typed):
    occupancy, signal-aspect-3-color, led-output, button-input,
    turnout-position, mast, ...
  Each resource type declares:
    - A behavior contract (what the channel can rely on)
    - One or more signatures, each = (field-roles, active-state rules)
  Resource instance:
    - Picks one signature from its type, binds it to specific CDI fields on a node
    - Provenance: profile-pre-declared | profile-slot-template | user-mapped
    - "Active" when bound by a channel → its signature's active-state rules apply

           ▲ backed-by

LAYER 1 — CDI fields (raw, board-specific)
  The addressable surface of the firmware. No semantic interpretation.
```

Each layer has clear ownership, and adds meaning without changing what's below.

### Why purpose-typed resources, not hardware-shape-typed

Earlier sketches of this model used hardware-shape categories at layer 2 (`io-line`, `led-driver`, `mast`). That was a leak: the channel layer ended up needing to know "I'm a block-occupancy channel that can bind to io-line resources configured as inputs." Hardware diversity bled upward.

Purpose-typed resources push the hardware diversity down. A channel says "I need an `occupancy` resource"; the resource type itself encodes the field requirements and active-state values. The channel never knows whether the occupancy resource is backed by a Tower-LCC BOD-4 input, a Signal LCC I/O line configured as a detector, or a standalone BOD-8's direct input.

### Why "signatures" within a type

A single resource type can have more than one valid implementation. Concrete example: `signal-aspect-3-color` (red/green/yellow) can be implemented as:

- **3-LED-discrete signature** — three CDI fields, one per color, on=that color.
- **2-LED-mixed signature** — two CDI fields (red, green); red alone=Red, green alone=Green, both=Yellow.
- **Mast-driven signature** — a single firmware mast resource that hides LED management entirely (Signal LCC's Mast section).

All three satisfy the same behavior contract: "display Red, Green, or Yellow on command." A channel of type `signal-aspect-3-color` binds to any of them and doesn't care which.

Recording the signature on the resource instance lets the constraint engine pick the right active-state rules. The channel layer is permanently insulated from the difference.

---

## Resource Creation Paths

A layer-2 resource instance is produced through one of three paths. All three produce the same record shape (`{ id, resourceType, signatureId, fieldBindings, label }`) and feed the same downstream machinery (display, constraint enforcement, event resolution).

| Path | Who authors what | When this applies |
|---|---|---|
| **Pre-instantiated** | Profile ships fully-bound resources for fixed-function hardware | BOD-4 in Tower-LCC Connector A produces 4 occupancy resources at known field paths |
| **Slot template** | Profile declares "from this slot, you can construct any of these resource types"; user picks which to instantiate per slot | Signal LCC I/O lines, where each line can become an occupancy / button-input / led-output resource |
| **User-mapped** | User picks a resource type from the system catalog and binds its field roles to CDI fields by hand | DIY or unprofiled boards; firmware-author-supplied profile not available |

The user-mapped path is what makes the channel model viable on boards without profiles. It does *not* introduce new resource types — those stay in a closed system catalog. The user authors only the field signature for one specific resource instance.

This also means the supply chain is uniform: a high-quality shipped profile, a user touch-up to a shipped profile, and a fully user-authored DIY resource all produce indistinguishable layer-2 records. The UI for resource creation can be a single flow with different amounts pre-filled.

---

## Constraints: Where They Live and How They Activate

The constraint rules live in the **resource type's signature**, not on the channel and not on a cross-product of channel-type × resource-kind.

- A signature declares **field roles** and, for each role, the **active-state rules** (what value(s) are valid, what's locked, what's freely editable).
- A resource instance is **passive** until a channel binds it. While unbound, its fields are just CDI fields with no extra constraint.
- A channel binding **activates** the resource. Its active-state rules apply to the bound fields and the constraint engine enforces them.

This separation is why the channel can stay passive metadata. The channel is the activation trigger; the rule book is the resource type's signature; the constraint engine is the runtime layer.

### Two layers of constraint within a signature

For convenience, signature rules fall into two natural tiers, and the editor surfaces them differently:

| Layer | Example for `led-output` signature on a generic I/O pin | Editor presentation |
|---|---|---|
| **Shape / mode** — which CDI field determines "what this resource is right now" | `Pin Function = Output` (locked when this resource is active) | Primary managed field; presented first |
| **Leaf rules** — values under the established shape | `Output Function = Steady Active Hi` (locked); brightness, fade = unmanaged | Secondary managed fields + unmanaged fields below |

The shape constraint matters because it determines which other CDI fields are even relevant under the resource's prefix. The relevance-rule machinery the profile system already uses for daughter-board selection extends naturally to this case.

### Override path

The only path to override a managed field is **Raw CDI** (the existing escape hatch). There is no per-channel override flag in `channels.yaml`. Drift detection — already in the vision — flags managed fields that are out of range and offers a one-click repair.

Rationale: a per-channel override would add stored state, sync complexity, and a third constraint mechanism without enabling anything Raw CDI doesn't already enable.

---

## Editing Through a Channel

The Wiring workspace's per-channel detail view is a **profile-curated lens onto a CDI subtree**. The channel doesn't own values; it offers a structured presentation of the CDI fields its bound resource references.

### Single-resource channel (most common)

Three zones in the channel detail, top to bottom:

1. **Identity.** Name, channel type, bound resource (label + resource type).
2. **Unmanaged settings.** The freely-editable fields under the resource's signature, presented as ordinary inputs. Mockup 1's "Wiring Settings" panel is exactly this.
3. **Managed settings.** Collapsed by default; expandable for inspection. Shows which fields the active signature locked, with the rationale and a pointer to Raw CDI for override. Mockup 1's "▸ Managed by channel type" panel is this.

### Multi-resource channel (e.g., aspect from raw LEDs)

When a channel's resource consumes multiple CDI field groups — like a 3-LED-discrete `signal-aspect-3-color` resource — the detail view adds a **per-resource section** between identity and unmanaged settings. Each constituent has its own sub-row with unmanaged fields scoped to that constituent (per-LED brightness, fade, effect). Channel-level unmanaged fields (anything that applies to the resource as a whole, e.g., a lamp-fade group setting) sit above the per-resource section.

This isn't a separate mockup; the structure follows from the resource's signature declaring per-role scope (channel-level vs per-constituent).

### Unbound resources and free slots

Not every resource is bound to a channel, and not every slot has a resource. The Wiring workspace presents them all in the same per-node table:

| State | Presentation | Action |
|---|---|---|
| Channel bound to resource | Channel row with name, type, facility, live state (Mockup 1 rows 1–4) | Edit inline; click "▾ details" for the lens view |
| Resource on a slot, no channel | "Available — `<resource type>`" row | "Create channel ↗" link |
| Slot with no resource | "Unconfigured — General I/O" row (Mockup 1 rows 5–8) | "Assign type →" link into the slot-template picker (Mockup 2) |

The "Assign type →" flow shown in Mockup 1 (pins 5–8) and Mockup 2 already corresponds to the slot-template creation path described above — choosing a channel/resource type for a slot creates the resource and immediately binds a channel.

---

## Channel Persistence

`channels.yaml` records only what's needed to identify and re-bind the channel:

```yaml
schemaVersion: '2.0'
channels:
  - id: 2b8dc48f-a9b0-45d6-b394-39f11d55de2c
    name: "Eagle Creek — East Approach"
    channelType: block-occupancy
    resourceRef:
      nodeKey: 0201570002D9
      resourceId: ca-input-1
```

That's the entire shape. No copies of CDI values, no field-binding details, no override state, no constraint cache. The CDI tree is the truth; the resource catalog (profile or user) is the binding; the channel is the identity.

Resources also need persistence — see "Open questions" below for where they live.

---

## Implementation: What Changes

This is a draft proposal; the items below describe the work, not a slice plan.

| Area | Change |
|---|---|
| **Layer-2 schema** | New `Resource` type catalog (system catalog) and `ResourceInstance` record (per-node). Resource type carries contract + signature list. Instance carries type, signature id, and field bindings. |
| **Profile schema (`.profile.yaml`)** | Replace the current `channelInputs` block with two equivalent constructs: (a) pre-instantiated resource declarations, (b) slot-template declarations. The existing `eventMapping` data folds into the signature's field roles. |
| **`channels.yaml` schema** | Bump to `schemaVersion: '2.0'`. Replace `hardwareRef: { nodeKey, slotId, inputOrdinal }` with `resourceRef: { nodeKey, resourceId }`. One-shot migration for v1.0 files (deterministic mapping for Tower-LCC entries: `(connector-a, 1)` → `ca-input-1`, etc.). |
| **`resolve_channel_event_ids`** | Becomes a resource lookup: find resource → read its signature's producer-event field roles → return event IDs. No more connector/input arithmetic at the call site. |
| **`ChannelCard.svelte`** | Renders the profile-supplied `resource.label` instead of `hardwareRef.connector`. The original display-leak that triggered this discussion goes away. |
| **Constraint engine** | New: given a node's resource bindings and channels, compute the set of active signature rules and feed them to ConfigEditor's relevance-rule machinery. Existing relevance rules handle the actual UI filtering. |
| **Slot-template UI** | Mockup 2's "channel type picker" is reframed as "resource type picker" — the user is choosing which resource to construct from the slot. The channel is auto-created and bound. (Same UX flow; clearer underlying semantics.) |
| **User-mapped resource UI** | New "Define resource" flow on unprofiled or under-profiled boards. Pick a resource type, see its signatures, pick a signature, bind its field roles to CDI fields. Bounded UI: the system catalog drives what's possible. |

---

## Migration

The current `channels.yaml` (`schemaVersion: '1.0'`) contains only Tower-LCC channels with `(connector-a/b, inputN)` references. The mapping to the new shape is deterministic:

- `slotId: connector-a, inputOrdinal: N` → `resourceId: ca-input-N`
- `slotId: connector-b, inputOrdinal: N` → `resourceId: cb-input-N`
- `channelType: block-occupancy` is unchanged.
- File-level `schemaVersion` bumps to `2.0`; loader handles both versions.

The Tower-LCC profile must, in the same release, declare the corresponding `ca-input-N` / `cb-input-N` resources (pre-instantiated based on the connector's daughter board selection) so that migrated channel references resolve.

No data loss; no user action required.

---

## Mockup References

The existing mockups already illustrate most of the model. Cross-references for the proposal:

| Mockup | What it illustrates that this proposal preserves or refines |
|---|---|
| **Mockup 1** — App Shell, Wiring view | The per-channel detail expansion (Wiring Settings + Managed by channel type) **is** the channel-as-lens view described here. Pins 5–8's "Assign type →" rows correspond to the slot-template creation path. Sidebar "Raw CDI" remains the escape hatch. |
| **Mockup 2** — Channel Type Application (per-pin) | The 3-Aspect Signal Head picker is, in this model, "construct a `signal-aspect-3-color` resource from this slot template, picking the 3-LED-discrete signature." The "Pins 4–6 will be claimed" line is the signature's field-role binding. The constraint confirmation banner is the active-state rules applying on bind. |
| **Mockup 3** — Facility Comprehension | Unchanged. The facility view already references channels by name; the resource layer below is hidden as intended. |
| **Mockup 4** — Template Apply | Unchanged. Pending requirements (e.g., "no named signal channel") correspond to channels whose required resource type isn't yet present on the layout. The "Create new channel inline" option triggers the slot-template flow. |
| **Mockup 5** — Channel Inventory | Unchanged. The hardware reference column reads "`<node> <resource label>`" (e.g., "Tower-3 — Connector A — Input 1" or "Signal LCC #1 — Mast 2") — the label is now profile-supplied per resource, not built from slot slugs. |
| **Mockup 8** — Pin Documentation | Unchanged. The printable form maps resource instances to channels, with labels from the profile. Boards without connectors (Signal LCC) render naturally — sections are per-resource-group, not per-connector. |

No new mockups are required by this proposal. The model is what the mockups have been describing; this writeup names it.

---

## Implications for the UX Vision

Two clarifications and one substantive shift relative to the current vision proposals:

1. **Resource layer is named explicitly.** `app-ux-vision.md` and `behavior-templates-proposal.md` currently describe "channels," "pins," "connectors," and "channel types" without naming the resource layer that sits between them. Both proposals get a small clarifying paragraph pointing at this one.
2. **Boards without profiles are no longer second-class.** The current vision line *"Boards without profiles fall back to the raw CDI view"* softens to: *"Boards without profiles can still expose channels via user-authored resource mappings (technical but bounded); Raw CDI remains the ultimate escape."*
3. **Channel hardware reference shape changes** (`hardwareRef` → `resourceRef`). The vision doesn't pin down this shape, so no vision text is invalidated — but the example labels in `app-ux-vision.md` ("Pin 3 on Connector A of Tower-3") need to become resource-label-shaped to read accurately across board families.

---

## Open Questions

1. **Where do user-authored resource definitions live?** Three candidates:
   - In the layout file (`channels.yaml` or sibling) — layout-scoped; lost when changing layouts.
   - In a per-node overlay (`node-resources.yaml` keyed by node ID) — survives layout changes.
   - In a user-extensible profile catalog — survives layouts and node IDs; can be re-used across nodes of the same model.

   Probably the second or third. Decision can wait until the user-mapping flow is designed.

2. **Partial resource definitions.** Can a resource be created from an incomplete field signature (e.g., user maps the producer-event roles but not the mode-determining role on a board where mode isn't separately configurable)? Likely yes when the signature itself permits it; needs a per-signature "required vs optional" markup.

3. **Composite or facility-level resources.** A facility (e.g., a signal mast with multiple aspect channels) currently composes from multiple channels. Could "facility = composite resource" be a clean way to model dispatcher-controlled signal interlocking, or is the facility/channel split the right boundary? Probably the latter (facilities are a Railroad-workspace concept that span nodes); this should stay separate from resource modeling.

4. **Slot-template feasibility constraints.** A slot might support multiple resource types (e.g., occupancy OR led-output) but not simultaneously. The profile already implicitly says "one resource per slot." Worth making explicit in the schema (`exclusive: true` on slot templates) so the constraint engine can prevent invalid combinations.

5. **Resource → channel-type compatibility direction.** Today's framing is "channel-type X requires resource type Y." Should resources also advertise which channel-types they accept? Both directions are useful in UI (template apply filters channels by required type; channel creation filters resources by compatible type). Probably store on the channel-type and derive the reverse, but worth confirming.

6. **Re-application of a slot template.** Changing what a slot does (occupancy → led-output) is delete-and-recreate, same pattern as today's BOD-8 → BOD-4 transition. The existing user-confirmation flow ("channels will be removed") generalizes; no new UX work needed beyond message text.

---

## Non-Goals

- **Signal aspect rule programming.** Aspect rule tables (which physical lamps light for which aspects on a Signal LCC mast) stay in the existing CDI editor (or a Mast-specific sub-editor). They're not a property of the resource model — they're authored values within the resource's unmanaged surface.
- **Behavior templates that need cross-resource logic.** Covered by `behavior-templates-proposal.md`; this proposal only deals with the data and constraint layer, not with facilities or template DSL.
- **JMRI bridge bean shape.** Covered by `jmri-bridge-proposal.md`; the bridge maps channels to JMRI beans regardless of the underlying resource shape.
- **Multi-node resources.** A resource lives on one node. Cross-node behavior is composed at the facility level.

---

## Suggested GitHub Issue (for approval before filing)

**Title:** Channel resource model — generalize HardwareReference to a purpose-typed resource layer

**Labels:** `kind/idea`, `area/profiles`, `area/channels`, `area/ux`

**Body:** (use this proposal or a condensed form; finalize at issue-filing time)
