# Quickstart: Configuration Modes & Placeholder Boards

This quickstart walks through the end-to-end validation case from the spec: add a TurnoutBoss placeholder, flip Left ↔ Right, observe the reshape, and confirm the configuration survives a restart. It also covers the Tower-LCC migration sanity check.

## Prerequisites

- Bowties built locally (`cd app && pnpm tauri dev` or the equivalent `cargo`-driven build) from this feature branch.
- No LCC hardware required.
- Bundled profiles present under `app/src-tauri/profiles/`:
  - `Mustangpeak-Engineering_TurnoutBoss.profile.yaml`
  - `Mustangpeak-Engineering_TurnoutBoss.cdi.xml`
  - `RR-CirKits_Tower-LCC.profile.yaml` (re-expressed under v2)
  - `RR-CirKits_Tower-LCC.cdi.xml`

## 1. Create or open a layout

1. Launch Bowties. The `LayoutPicker` appears.
2. Choose **New Layout**, pick a folder, name it `placeholder-demo`. The layout opens with no real nodes.

## 2. Add a TurnoutBoss placeholder

1. In the layout view, use **Add board**.
2. Pick **Mustangpeak Engineering — TurnoutBoss** from the bundled board models.
3. The guided-configuration screen opens with the TurnoutBoss CDI rendered offline. The placeholder gets a generated id of the form `placeholder:<uuidv4>`, visible in the placeholder details panel.

**Expected**: Every eventid leaf displays identically to a real board's EventId field — showing the event ID value (all zeros) and the producer/consumer role badge — but is disabled (not editable) and does not show the add-connection control.

## 3. Exercise the Left / Right Configuration Mode

1. Locate the selector field **Layout Configuration Setup / How this TurnoutBoss is used on your layout.**
2. Set it to **Left**.
3. Observe:
   - Detector 3 group is **relevant** (no "irrelevant" banner).
   - Occupancy eventid leaves show Producer / Consumer roles per the Left overlay.
4. Change the selector to **Right**.
5. Observe:
   - Detector 3 group becomes **irrelevant** (banner appears, populated from the rule's `explanation`).
   - The Occupancy eventid leaves flip Producer ↔ Consumer per the Right overlay.
   - No errors in the dev console or backend log.

## 4. Edit non-event fields

1. Edit a few enum / integer / string fields (e.g. **Detector Sensitivity**, the Node Description string).
2. Edit the **User Name** leaf in the Identification segment to `Yard Throat (left)`. The sidebar label switches from `"{manufacturer} {model}"` to `Yard Throat (left)` (HITL 2026-05-25 — placeholders have no separate display-name field; naming is the standard CDI User Name leaf, same surface a real node uses).

**Expected**: Every edit persists into the placeholder's snapshot `config` tree (visible in the saved `<companion>/nodes/placeholder_<uuid>.yaml`).

## 5. Persist + reopen

1. Save the layout (Ctrl+S / Cmd+S).
2. Inspect the layout directory on disk — a new `<companion>/nodes/placeholder_<uuid>.yaml` `NodeSnapshot` file holds the placeholder (no `placeholderBoards:` block; that data shape was removed in S8.5).
3. Quit Bowties.
4. Relaunch, open the same layout from the picker.

**Expected**: The placeholder is restored with its id, all field edits including the User Name leaf, and current Left/Right selection (FR-017, SC-004). The sidebar shows the User Name immediately.

## 6. Confirm placeholder isolation in binding flows

1. With the placeholder present, open any flow that lets you bind an event from one node to another (e.g. the bowtie / event-link UI).
2. Search for the placeholder's eventid leaves.

**Expected**: They never appear as a binding source or target (FR-015, SC-005). Attempting to set one programmatically returns `PlaceholderEventNotBindable`.

## 7. Delete the placeholder

1. From the placeholder details panel, choose **Delete board**.
2. Confirm the prompt.

**Expected**: The placeholder is removed; other layout entries are untouched (FR-017a).

## 8. Tower-LCC migration sanity check

1. With a real Tower-LCC node connected (or a Tower-LCC placeholder added the same way as step 2), install each supported daughterboard variant on each connector via the existing connector picker (now driven by a `structuralSlot` Configuration Mode under the hood).
2. For every variant, confirm:
   - The relevant Port I/O Line groups light up or dim out exactly as they did before this feature.
   - Event roles in those groups match the daughterboard intent.
   - Any structural validity errors that fired previously still fire.

**Expected**: Behavior is identical to the pre-migration baseline (FR-023, SC-003). The shipped Tower-LCC profile contains zero references to `connectorSlots`, `connectorConstraintVariants`, `daughterboardReferences`, or `carrierOverrides`.

## 9. Profile-author preview

1. Repeat steps 2–5 against any other bundled profile.
2. Use the placeholder workflow as your preview tool — no Explorer screen required (SC-006).

---

## Troubleshooting

| Symptom | Likely cause | Action |
|---|---|---|
| "unrecognized variant value" banner on a fresh placeholder | Bundled profile has been edited so an old stored value no longer matches a declared variant | Pick a declared variant from the warning's inline picker (FR-007). |
| Layout opens but a placeholder shows as **Unknown model** | `profileStem` references a board no longer bundled in this build | Restore the bundle or delete the placeholder (FR-022). |
| Tower-LCC behavior differs from before | Migration regression | Compare the new overlay output against the pre-migration test snapshot — see Tower-LCC parity tests in the build's `cargo test` output. |
