# Panel Facility IPC Contracts

**Status**: `panel_wiring_facts` and `compose_panel` implemented in S1 (app/src-tauri/src/panel/). `compose_panel_removal` and release are S4. Types are in [data-model.md](../data-model.md). Pins are keyed by `output` (1–16); a lamp's identity is its Direct Lamp row (decision D1).

## Reused boundaries

- `sync_layout_drafts` / `clear_layout_drafts`: draft sync before compose.
- `save_layout_directory`, plus Sync Panel apply: the single writer (ADR-0001).
- The configuration edit drafts, through `configEditor.applyEdit(editKey, value, { facilityId })`.
- `eventStateStore`: live lamp observations.

## `panel_wiring_facts(request) -> PanelWiringFacts`

Request: `{ nodeKeys?: NodeKey[] }`. Without `nodeKeys`, every Signal LCC board with Direct Lamp Control is returned. The frontend compares owner facility IDs itself.

Response:

```text
{
  boards: [{
    nodeKey, name,
    pins: [{
      output, label,                // 9, "#9 H3-G" from signal_lcc_output_label
      group,                        // "H3", for the grid's grouping
      owner: { kind: "free" }
           | { kind: "panel", facilityId, name, row }   // this or another panel's lamp row
           | { kind: "facility", facilityId, name }     // ABS mast
           | { kind: "outside", row | null, description } // Lamp Description (or "Mast N"), configured outside Bowties
    }]
  }],
  issues: [{ nodeKey, detail }]
}
```

Guarantees: read-only. Owners come from the effective layout (saved plus drafts), so pins other open edits staged show as taken.

## `compose_panel(request) -> PanelComposition`

Request:

```text
{
  facilityId,
  record: PanelRecord,
  lamps: [{ entryId, nodeKey, output, name }],   // the editor's wiring; saved rows are read from the layout (S4)
  release: [{ nodeKey, owner }]                  // S4, optional
}
```

Response: `{ edits: ResolvedEdit[] (same form as compose_block_signals: nodeKey, space, address, path, value), assignments: [{ entryId, nodeKey, row }], issues: [{ code, entryId?, nodeKey?, output?, detail }] }`. Resets of dropped rows and named releases join `edits` (S4).

Rules:

- For each lamp, it keeps the lamp's row or allocates one, and sets Lamp Selection, Lamp On, Lamp Off and Lamp Description on it (data-model "Compiled effect").
- Rows this facility claimed before but not in `lamps` are reset.
- Issue codes:
  - `pinOwnedElsewhere`
  - `pinOutsideNotReleased`
  - `blockEventsUnresolved` (occupancy events unknown)
  - `boardUnavailable`
  - `duplicatePin`
  - `noFreeLampRow`
  - `releaseInvalid` (a named release of a claimed row; S4)
  - `claimedSlotUnavailable` (a dropped row's board has no facts; S4)
- Blocking issues mean the caller stages nothing.
- No writes, no draft mutation, no lasting reservation.

## `compose_facility_removal(facilityId) -> FacilityRemoval` (S4)

Kind-agnostic (replaces the planned `compose_panel_removal` and `compose_block_signals { remove: true }`). Response: `{ edits, logicPlacement, issues, claims: [{ nodeKey, kind: mast | logic | trackCircuit | directLamp, index, lampSelection | null }] }`. Resets exactly `facility_claimed_slots`; `claims` drives the confirmation dialog. Same guarantees as `compose_panel`. `panel_wiring_facts` also accepts optional `release` and returns `releaseIssues`.

## Open result extension

`LayoutDirectoryReadData.migration?: { blockIndicators: number, panels: number, dropped: number }`. If it is present, the route shows the notice and marks the layout changed (FR-032).

## Frontend orchestration

`facilityOrchestrator`:

- `createPanelFacility(editorState)` and `updatePanelFacility(editorState)`:
  1. sync drafts;
  2. call `compose_panel`;
  3. refuse if there are issues;
  4. create, rebind, rename or delete lamp channels and attach or detach them on `lamps`;
  5. add the facility or `setPanelRecord` (plus `renameFacility`);
  6. stage the edits;
  7. roll back steps 4–6 on failure.
- `previewFacilityRemoval(facilityId)`: sync drafts, call `compose_facility_removal`, stage nothing; returns the claims.
- `removeFacility({ facilityId, leaveBoard, previewedClaims })`: the only removal router. Compose again, refuse on issues or a changed plan, stage the resets, delete the facility and its lamp channels.

## Contract tests

1. Wiring facts label 16 pins H1-G…H4-L and attribute every owner kind.
2. Compose with pins #9, #3, #14 produces edits on those rows only.
3. Clear and override "lit when" swap Lamp On and Lamp Off.
4. A pin owned elsewhere blocks compose, and nothing is staged.
5. Edit resets exactly the dropped pins; removal resets exactly the claimed pins; ABS sharing the blocks is unchanged.
6. Migration open result: counts are correct, the layout is marked changed, and no configuration edits are staged.
