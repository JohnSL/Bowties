# Profile YAML Schema Deltas — Block Indicator Facility (Spec 018)

This document specifies the YAML schema changes required in the two profile files that ship with Bowties. The intent is to keep diffs surgical and preserve all existing user-visible behaviour from spec 015.

---

## File 1: `app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml`

### Per-board change — applies to **BOD4**, **BOD4-CP**, **BOD-8**, **BOD-8-SM**

#### Before (excerpt — BOD-8 example)

```yaml
- daughterboardId: "BOD-8"
  displayName: "BOD-8"
  kind: "input"
  validityRules:
    - targetPath: "Port I/O/Line/Input Function"
      lineOrdinals: [1, 2, 3, 4, 5, 6, 7, 8]
      constraintType: allowValues
      allowedValues: [2]
      allowedValueLabels: ["Active Lo"]
      explanation: "BOD-8 detector lines are active-low."
    # ... more validityRules
  metadata:
    notes: "8-channel occupancy detector"
    channelInputs:
      - channelType: "block-occupancy"
        inputs: [1, 2, 3, 4, 5, 6, 7, 8]
        eventMapping:
          occupied: { producerLeafIndex: 0 }
          clear:    { producerLeafIndex: 1 }
```

#### After

```yaml
- daughterboardId: "BOD-8"
  displayName: "BOD-8"
  kind: "input"
  metadata:
    notes: "8-channel occupancy detector"
    channelInputs:
      - channelType: "block-occupancy"        # kept transitionally for Slice 2 readers; removed in Slice 6
        styleId: "bod-block-detector-input"   # NEW — explicit style binding
        role: "block-occupancy"               # NEW — explicit role binding (must match style's declared role)
        inputs: [1, 2, 3, 4, 5, 6, 7, 8]
        eventMapping:
          occupied: { producerLeafIndex: 0 }
          clear:    { producerLeafIndex: 1 }
        constraints:                          # NEW — moved verbatim from the old top-level validityRules
          - targetPath: "Port I/O/Line/Input Function"
            lineOrdinals: [1, 2, 3, 4, 5, 6, 7, 8]
            constraintType: allowValues
            allowedValues: [2]
            allowedValueLabels: ["Active Lo"]
            explanation: "BOD-8 detector lines are active-low."
          # ... rest of the moved rules
  # NOTE: the top-level `validityRules` block is removed. The constraints now travel with the style
  # that owns the channel claiming the pin.
```

#### Semantics

- The constraint contract is now declared **on the style** (`channelInputs[].constraints`), not on the daughter-board entry. User-visible behaviour is identical (FR-028): selecting a BOD-family board still applies the same restrictions to the same fields, because the auto-created channels carry the style whose constraints get applied.
- `channelType` is kept in this slice for backward parsing during the transition; the explicit `styleId` and `role` are the post-018 source of truth. Slice 6 removes `channelType`.
- The lookup that the existing relevance/validity renderer performs changes from "find rules under the selected daughter-board entry" to "find rules under the style of each channel claiming this pin." Backend resolution code in `bowties-core` is updated accordingly (single seam change).

---

## File 2: `app/src-tauri/profiles/RR-CirKits_Inc._Signal-LCC.profile.yaml`

### New top-level section: `subsystemStyles`

The Signal LCC profile gains a new top-level section declaring the user-creatable styles available on each subsystem and their constraint contracts.

#### Before (excerpt)

```yaml
schemaVersion: "2.0"
nodeType:
  manufacturer: "RR-CirKits, Inc."
  model: "Signal-LCC"
eventRoles:
  - groupPath: "Direct Lamp Control/Lamp"
    role: Consumer
  # ... other eventRoles
relevanceRules:
  - id: "lamp-config-when-used-by-mast"
    affectedTarget: "Direct Lamp Control/Lamp"
    allOf:
      - field: "Lamp Selection"
        irrelevantWhen: ["Used by Mast"]
    explanation: "..."
  # ... other rules
```

#### After (additions only; nothing removed)

```yaml
schemaVersion: "2.0"
nodeType:
  manufacturer: "RR-CirKits, Inc."
  model: "Signal-LCC"
eventRoles:
  - groupPath: "Direct Lamp Control/Lamp"
    role: Consumer
  # ... unchanged
relevanceRules:
  # ... unchanged
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
            explanation: "Direct lamp control requires the lamp row to not be claimed by a mast."
```

#### Semantics

- `subsystemStyles` is the **declared catalog** of user-creatable styles per subsystem (R1, FR-012).
- When the user invokes *Add channel* on a slot whose required role is `lamp-indicator`, the AddChannelPicker enumerates rows under `Direct Lamp Control/Lamp` across all connected Signal LCC nodes and filters by the style's `constraints` block (R9).
- When a user-owned channel claims a row, the constraint contract is applied to that row's CDI fields through the existing relevance/validity renderer (R6). The user sees `Lamp Selection` locked to its style-required value while unmanaged fields stay editable.
- The `disallowValues` constraint type is a new addition to the existing constraint vocabulary (which already supports `allowValues` and `hideSection`). The renderer interprets `disallowValues` as "show all values except those in `disallowedValues`."

---

## Backend parser changes (summary)

`bowties-core` profile loader changes are minimal and surgical:

1. Read the new `subsystemStyles` top-level key on profile load; store in the in-memory profile representation alongside existing `eventRoles` and `relevanceRules`.
2. Read the new `styleId`, `role`, and `constraints` fields inside `channelInputs`; store on the per-board metadata.
3. Constraint resolution path: when computing which CDI fields are managed (read-only, restricted, or hidden) for a given pin, the source switches from "daughter-board entry's `validityRules`" to "active styles whose channels claim this pin." Result is identical when the same constraint set is moved; differs only when new styles join (e.g., `single-led-direct-lamp` on a lamp row).
4. New constraint type `disallowValues` joins the existing `allowValues` / `hideSection` switch in the renderer.

No schema-version bump required for either profile file in this slice (parsers tolerate the new optional keys; absence of `subsystemStyles` means "this subsystem has no user-creatable styles"). The eventual cleanup in Slice 6 removes the transitional `channelType` field, which can occur within the existing `schemaVersion: "2.0"` umbrella because it is a delete-only change for a field no live consumer reads.
