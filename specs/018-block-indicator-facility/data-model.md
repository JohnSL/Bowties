# Data Model — Block Indicator Facility (Spec 018)

Entity shapes and relationships for the channel-and-facility model. All entities below have a **persistent** form (YAML on disk under the layout folder), a **wire** form (JSON across the Tauri IPC boundary), and an **in-memory** form (Rust struct / TS interface). This document specifies the persistent and wire forms; in-memory forms follow trivially via `serde` / TS interface mirroring.

## Entity overview

```text
┌─────────────────────────┐
│ BehaviorTemplate (code) │  ◀── R2: hardcoded module in bowties-core
│  - templateId           │
│  - displayName          │
│  - slots[]              │  ◀── declares slot label, producer/consumer, required role
│  - mapping[]            │  ◀── (producer state → consumer command), e.g. occupied→lit, clear→unlit
└─────────────┬───────────┘
              │ templateId
              ▼
┌─────────────────────────┐        ┌─────────────────────────────────┐
│ Facility                │ slots  │ Channel                          │
│  - facilityId (UUIDv4)  │◀──────▶│  - channelId (UUIDv4)            │
│  - templateId           │ N : 1  │  - name                          │
│  - name                 │ via    │  - role          (FK to Role)    │
│  - slotBindings:        │ slot   │  - style         (FK to Style)   │
│      label → channelId? │ entry  │  - ownership                     │
└─────────────┬───────────┘        │  - binding (discriminated)       │
              │                    │      kind=connectorInput | lampRow│
              │ owns                └──────────────┬──────────────────┘
              ▼                                    │
┌─────────────────────────┐                        │ claims
│ Bowtie (existing)       │                        ▼
│  - bowtieId             │              ┌──────────────────────────┐
│  - createdByFacility    │ (new field)  │ Pin (no persisted form;  │
│  - …existing fields…    │              │  addressed via binding)   │
└─────────────────────────┘              └──────────────────────────┘

┌─────────────────────────┐
│ Role (code)             │   ◀── R1: in code (Rust enum + TS literal union)
│  - roleId               │
│  - stateVocabulary      │   ◀── e.g. block-occupancy: unknown | occupied | clear
└─────────────────────────┘

┌─────────────────────────┐
│ Style (profile YAML)    │   ◀── R1: declared in profile YAML per subsystem
│  - styleId              │
│  - role                 │   ◀── FK to Role
│  - subsystem            │
│  - userCreatable: bool  │
│  - claimsPins: spec     │
│  - eventLeaves: mapping │   ◀── role-state → CDI leaf index (producer or consumer side)
│  - constraints[]        │   ◀── { targetPath, constraintType, allowedValues, … }
└─────────────────────────┘
```

---

## Channel (extended; replaces pre-018 `InformationChannel`)

**Persistent form** (entry in `channels.yaml`):

```yaml
- channelId: "0c3e5cfa-1d54-4e3e-9c12-7c6c8b5b6f01"   # UUIDv4, stable
  name: "Block 5"                                       # user-renameable; default generated per spec 015
  role: "block-occupancy"                               # one of: block-occupancy | lamp-indicator
  style: "bod-block-detector-input"                     # one of: bod-block-detector-input | single-led-direct-lamp
  ownership: "hardware-owned"                           # hardware-owned | user-owned
  binding:                                              # discriminated by `kind`
    kind: "connectorInput"
    nodeKey: "02.01.57.00.00.12"
    connector: "port-a"
    input: 3                                            # 1-based

# Lamp-indicator channel example:
- channelId: "9a82bb15-4b0c-4f54-8a36-c1f7a4d3e2b0"
  name: "Block 5 Indicator"
  role: "lamp-indicator"
  style: "single-led-direct-lamp"
  ownership: "user-owned"
  binding:
    kind: "lampRow"
    nodeKey: "02.01.57.00.00.4f"
    rowOrdinal: 7                                       # 1-based
```

**Wire form** (TS interface mirrored in `app/src/lib/types/channels.ts`):

```ts
export type ChannelRole = 'block-occupancy' | 'lamp-indicator';
export type ChannelStyle = 'bod-block-detector-input' | 'single-led-direct-lamp';
export type ChannelOwnership = 'hardware-owned' | 'user-owned';

export type ChannelBinding =
  | { kind: 'connectorInput'; nodeKey: string; connector: string; input: number }
  | { kind: 'lampRow'; nodeKey: string; rowOrdinal: number };

export interface Channel {
  channelId: string;        // UUID v4
  name: string;
  role: ChannelRole;
  style: ChannelStyle;
  ownership: ChannelOwnership;
  binding: ChannelBinding;
}
```

**Validation rules**:
- `channelId` MUST be UUID v4. Stable across rename. Globally unique within a layout.
- `role` MUST be a registered role (R1).
- `style` MUST be a registered style whose declared role equals `channel.role` (R1, FR-002).
- `binding.kind` MUST match the style's declared binding shape (e.g., `single-led-direct-lamp` requires `kind === 'lampRow'`).
- `ownership === 'hardware-owned'` ⟹ the channel was created by a hardware-config event (BOD daughter-board selection). Its lifetime is tied to that selection (FR-006, FR-007).
- `ownership === 'user-owned'` ⟹ the channel was created by an `Add channel` action on a facility slot. Its lifetime is tied to its single binding slot (FR-006).
- A channel MAY have at most one binding to a facility slot at any point in time (FR-004). This is enforced at the facility-store / orchestrator layer, not on the channel record itself.

**State transitions** (lifecycle):

```text
Hardware-owned:
   [board selected] ──auto-create──▶ Exists ──[user rename]──▶ Exists  (rename does not change ownership)
                                       │
                                       └──[board cleared/changed]──▶ Deleted (cascade frees slot bindings + Wired facilities)

User-owned:
   [Add channel on slot] ──atomic──▶ Exists + bound to slot
                                       │
                                       ├──[Remove from slot]──▶ Deleted (no orphan unbound state in this slice)
                                       └──[Rebind to other channel]──▶ Deleted (if no longer bound)
```

---

## Facility

**Persistent form** (entry in `facilities.yaml`):

```yaml
schemaVersion: "1.0"
facilities:
  - facilityId: "5e8d4b22-3f10-4a4b-bf30-9d1c2e6f3a45"  # UUIDv4
    templateId: "block-indicator"
    name: "Block 5"
    slotBindings:
      input: "0c3e5cfa-1d54-4e3e-9c12-7c6c8b5b6f01"      # channelId, nullable
      output: null                                        # empty slot is a first-class state
```

**Wire form**:

```ts
export interface Facility {
  facilityId: string;                            // UUID v4
  templateId: string;                            // 'block-indicator' (only value in this slice)
  name: string;
  slotBindings: Record<string, string | null>;   // slot label → channelId | null
}

export type FacilityStatus = 'Incomplete' | 'Wired';
```

**Validation rules**:
- `facilityId` MUST be UUID v4, stable across rename, globally unique within a layout.
- `templateId` MUST refer to a registered behavior template (FR-014).
- `slotBindings` keys MUST exactly match the template's declared slot labels (no extras, no missing).
- Each non-null `slotBindings[label]` MUST refer to an existing channel whose `role` matches the template's slot declaration for `label` (FR-002, FR-003).
- A `channelId` MAY appear in at most one slot across all facilities in the layout (FR-004).

**Derived state — `facilityStatus(facility)`** (pure helper, R10):

```ts
function facilityStatus(facility: Facility): FacilityStatus {
  return Object.values(facility.slotBindings).every(v => v !== null)
    ? 'Wired'
    : 'Incomplete';
}
```

**State transitions**:

```text
Created with empty slots
        │
        ├──[slot becomes filled]──▶ Re-derive status
        │                              │
        │                              └──[was last empty slot]──▶ Wired
        │                                                            │
        │                                                            └──orchestrator creates underlying bowtie(s)
        │                                                                (existing bowtie creation mechanism)
        │
        └──[slot becomes empty]──▶ Re-derive status
                                        │
                                        └──[was Wired]──▶ Incomplete
                                                            │
                                                            └──orchestrator frees facility's bowtie(s)
                                                                (existing slot-detach pipeline)
```

---

## BehaviorTemplate (code-only entity)

**In-code form** (`bowties-core/src/behavior_templates/mod.rs`):

```rust
pub struct BehaviorTemplate {
    pub template_id: &'static str,        // "block-indicator"
    pub display_name: &'static str,       // "Block Indicator"
    pub slots: &'static [SlotDefinition], // ordered
    pub mapping: &'static [StateMapping], // producer state → consumer command
}

pub struct SlotDefinition {
    pub label: &'static str,              // "input" | "output"
    pub kind: SlotKind,                   // Producer | Consumer
    pub required_role: &'static str,      // "block-occupancy" | "lamp-indicator"
}

pub enum SlotKind { Producer, Consumer }

pub struct StateMapping {
    pub producer_state: &'static str,     // "occupied" | "clear"
    pub consumer_command: &'static str,   // "lit" | "unlit"
}

pub const BLOCK_INDICATOR: BehaviorTemplate = BehaviorTemplate {
    template_id: "block-indicator",
    display_name: "Block Indicator",
    slots: &[
        SlotDefinition { label: "input", kind: SlotKind::Producer, required_role: "block-occupancy" },
        SlotDefinition { label: "output", kind: SlotKind::Consumer, required_role: "lamp-indicator" },
    ],
    mapping: &[
        StateMapping { producer_state: "occupied", consumer_command: "lit" },
        StateMapping { producer_state: "clear",    consumer_command: "unlit" },
    ],
};

pub fn registered_templates() -> &'static [BehaviorTemplate] { &[BLOCK_INDICATOR] }
```

**Wire form** (JSON returned by `list_behavior_templates`):

```ts
export interface BehaviorTemplate {
  templateId: string;
  displayName: string;
  slots: Array<{ label: string; kind: 'producer' | 'consumer'; requiredRole: ChannelRole }>;
  mapping: Array<{ producerState: string; consumerCommand: string }>;
}
```

---

## Role (code-only entity)

**In-code form** (Rust enum mirrored as TS literal union):

```rust
pub enum ChannelRole {
    BlockOccupancy,   // states: Unknown, Occupied, Clear
    LampIndicator,    // states: Unknown, Lit, Unlit
}

pub enum BlockOccupancyState { Unknown, Occupied, Clear }
pub enum LampIndicatorState  { Unknown, Lit, Unlit }
```

```ts
export type ChannelRole = 'block-occupancy' | 'lamp-indicator';
export type BlockOccupancyState = 'unknown' | 'occupied' | 'clear';
export type LampIndicatorState  = 'unknown' | 'lit' | 'unlit';
```

**Notes**:
- `unknown` is a first-class state of every role per spec (no observation yet).
- State names encode real-world intent (`occupied`/`clear`, `lit`/`unlit`), never electrical abstractions (`on`/`off`, `true`/`false`).

---

## Style (profile-YAML entity)

**Persistent form** — declared in profile YAML under the relevant subsystem:

```yaml
# In RR-CirKits.shared-daughterboards.yaml under BOD-8 metadata:
metadata:
  channelInputs:
    - channelType: "block-occupancy"       # kept for transitional compatibility in Slice 2; retired in Slice 6
      styleId: "bod-block-detector-input"  # NEW — explicit style binding
      role: "block-occupancy"              # NEW — explicit role binding (matches styleId.role)
      inputs: [1, 2, 3, 4, 5, 6, 7, 8]
      eventMapping:
        occupied: { producerLeafIndex: 0 }
        clear:    { producerLeafIndex: 1 }
      constraints:                         # NEW — migrated from daughterboard validityRules
        - targetPath: "Port I/O/Line/Input Function"
          constraintType: "allowValues"
          allowedValues: [2]
          allowedValueLabels: ["Active Lo"]
          explanation: "BOD-8 detector lines use active-low occupancy outputs."
        # ... (other rules moved verbatim from the old validityRules block)

# In RR-CirKits_Inc._Signal-LCC.profile.yaml under Direct Lamp Control declaration:
subsystemStyles:                            # NEW top-level key
  - subsystem: "Direct Lamp Control"
    styles:
      - styleId: "single-led-direct-lamp"
        role: "lamp-indicator"
        userCreatable: true
        claimsPins:
          kind: "lampRow"
          count: 1
        eventMapping:
          lit:   { consumerLeafPath: "Lamp On" }
          unlit: { consumerLeafPath: "Lamp Off" }
        constraints:
          - targetPath: "Direct Lamp Control/Lamp/Lamp Selection"
            constraintType: "disallowValues"
            disallowedValues: ["Used by Mast"]
            explanation: "Direct lamp control requires the row to not be claimed by a mast."
```

**Wire form**: backend resolves the subsystem style catalog at config-read time and exposes per-row style availability through the existing CDI tree annotation pipeline; no separate IPC for style enumeration is required. The constraint application path is the existing relevance/validity renderer (R6).

**Validation rules**:
- Every `styleId` MUST be unique across all profiles.
- A style's `role` MUST be a registered role.
- A `single-led-direct-lamp` style's `eventMapping` MUST reference consumer leaves declared on the same row via the existing `eventRoles` declaration.

---

## Pin (logical entity; not directly persisted)

A pin is the addressable target of a channel's `binding`. It has **no separate persistent form** — it is implicitly defined by the combination of `nodeKey` + subsystem + ordinal (connector input, lamp row). The only place where pins surface as data is inside `channel.binding`. Future styles claiming multiple pins per channel extend `binding` to carry a list (see *Spec extensibility* below).

---

## Bowtie (existing; gains one new field)

The existing `Bowtie` persistence in `bowties.yaml` gains exactly one new optional field:

```yaml
- bowtieId: "..."
  createdByFacility: "5e8d4b22-3f10-4a4b-bf30-9d1c2e6f3a45"   # NEW — UUIDv4 or absent
  # ... all existing fields unchanged
```

**Validation rules**:
- `createdByFacility` is OPTIONAL. Absent ⟹ user-created bowtie (the existing case).
- When set, MUST refer to an existing `facilityId`. Cascade: deleting the facility removes these bowties via the existing slot-detach pipeline (R5).

---

## Relationships and invariants summary

| Invariant | Enforced where |
|---|---|
| `channel.style.role == channel.role` | `validateChannel()` in `bowties-core/src/channels` |
| `channel.binding.kind` matches the style's claim shape | `validateChannel()` |
| `facility.slotBindings[label].role == template.slots[label].requiredRole` | `validateFacility()` |
| Each `channelId` appears in at most one `slotBindings[label]` across all facilities | `facilities.svelte.ts` store (assertion on bind) + backend on persist |
| `createdByFacility` references an existing `facilityId` | `validateLayout()` cross-file check |
| Hardware-owned channels exist iff their backing hardware-config selection exists | `connectorSelectionOrchestrator` step 4 + cascade on board clear |
| User-owned channels exist iff they are bound to exactly one slot | `facilityOrchestrator` (atomic Add-channel and Remove-from-slot) |
| `facilityStatus()` is derived, never stored | `effectiveLayoutStore` merge (R10) |

---

## Spec extensibility (not implemented; design intent)

Already shaped to absorb future work without schema rework:

- **Multi-pin styles** — `binding.kind === 'lampRow'` becomes `binding.kind === 'lampRows'` with `rowOrdinals: number[]`; no other entity changes.
- **Multiple styles per role** — `style` enum widens; `Channel.style` and slot-binding logic unchanged because slots match by role.
- **YAML-defined templates** — `BehaviorTemplate` Rust struct becomes deserialisable from YAML; the wire form is unchanged.
- **Placeholder-node-backed channels** — `binding.nodeKey` accepts placeholder NodeKeys (already a unified type per ADR-0008); no entity change.
- **Channel fan-out (ref-counted user-owned channels)** — `facility.slotBindings` becomes many-to-many in the channel→facilities direction; channel deletion gated on bind-count rather than on first slot detach.
