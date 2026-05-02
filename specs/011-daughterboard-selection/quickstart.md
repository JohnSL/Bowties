# Quickstart

## Goal

Validate the connector daughterboard selection workflow end to end on supported RR-CirKits carrier boards using reusable daughterboard profiles and saved per-node connector selections.

## Prerequisites

- Bowties workspace builds successfully in the current branch.
- At least one RR-CirKits Tower family profile and one RR-CirKits Signal family profile are available.
- Connector-slot metadata and reusable daughterboard definitions are present in authored profile YAML.
- A saved layout or offline layout context is available for persistence checks.

## Verification Flow

### 1. Run focused checks

From `app/`:

```powershell
npm run check
npm test
```

From `app/src-tauri/`:

```powershell
cargo test
```

### 2. Open a supported carrier board

1. Launch Bowties with a layout containing a supported RR-CirKits Tower or Signal LCC node.
2. Open the node configuration view.
3. Confirm the UI shows connector-slot controls with profile-authored labels and a `None installed` option where allowed.

### 3. Select daughterboards per slot

1. Select one compatible daughterboard for slot A and a different compatible daughterboard for slot B.
2. Confirm each slot selection is tracked independently.
3. Confirm affected lines/sections immediately narrow to valid options for their own slot.
4. Set one slot back to `None installed`.
5. Confirm the affected lines immediately return to the base carrier-board options unless that profile explicitly authors empty-slot behavior.

### 4. Trigger staged compatibility repairs

1. Start with a value that is valid for the first daughterboard.
2. Change the slot selection to a daughterboard that invalidates that value.
3. Confirm Bowties automatically stages compatible replacements or resets.
4. Confirm the staged changes are visible before apply and no newly invalid values can be chosen.

### 5. Save and reopen

1. Save the layout/project context.
2. Close and reopen the saved context.
3. Confirm connector selections restore for each node and the filtered config view reappears without re-entry.

### 6. Verify non-modular fallback behavior

1. Open a node with no connector-slot metadata.
2. Confirm no connector-selection UI appears.
3. Confirm existing configuration behavior is unchanged.

### 7. Verify reusable daughterboard behavior across carrier families

1. Open one in-scope Tower carrier and one in-scope Signal carrier.
2. Use a shared reusable daughterboard definition on both, with any necessary carrier-specific overrides.
3. Confirm profile reuse works without duplicating the shared daughterboard definition.

## Expected Outcome

- Connector slots are visible only for supported carrier boards.
- Per-node connector selections persist with saved layout/project context.
- Valid choices narrow by connector slot and selected daughterboard.
- Bowties stages compatible follow-up config edits automatically when a selection invalidates existing values.
- Unsupported nodes keep the pre-feature experience.