# Quickstart: Offline Layout Editing

**Feature Branch**: `010-offline-layout-editing`

## Goal

Capture a live LCC layout once, open and edit it later without bus access, then sync selected offline changes back to the bus.

## Prerequisites

- Bowties app built and runnable (`npm run tauri dev` from `app/`).
- At least one reachable LCC network for capture/sync validation.
- Existing node discovery and read flows functioning.

## Workflow 1: Capture Layout Online

1. Connect to the target LCC bus.
2. Discover nodes and run configuration reads (node-by-node or read-all).
3. Open Save Layout and choose a new directory.
4. Confirm generated files:
   - `manifest.yaml`
   - `nodes/<NODE_ID>.yaml` per discovered node
   - `bowties.yaml`
   - `offline-changes.yaml`
5. Verify partial-read nodes show `captureStatus: partial` and missing details.

Expected result:
- Layout directory is readable YAML with deterministic ordering.
- Bowtie metadata and role classifications persist.

## Workflow 2: Open and Browse Offline

1. Disconnect LCC bus.
2. Open the captured layout directory.
3. Verify persistent banner shows `Offline - Captured <date/time>`.
4. Browse configuration and Bowties tabs.
5. For partial captures, verify fields render `(Not captured)` and are not editable.

Expected result:
- No bus connection required.
- Captured values and metadata visible exactly from disk state.

## Workflow 3: Edit Offline and Save

1. Change one configuration field and one bowtie name/tag.
2. Confirm both changes show offline-change indicators.
3. Save layout directory.
4. Close and reopen the layout.

Expected result:
- Pending offline changes are restored after reopen.
- Baseline and planned values remain separate in `offline-changes.yaml`.

## Workflow 4: Connect and Sync

1. With a layout containing pending changes, connect to a bus.
2. During SNIP-only stage, verify preliminary `matching in progress` state.
3. After value reads complete, review Sync Panel categories:
   - Conflicts (must resolve per row)
   - Clean (bulk pre-selected)
   - Already applied (count only)
4. If match is `uncertain` or `likely different`, choose mode:
   - `target layout bus`, or
   - `bench/other bus`
5. Resolve conflicts and apply selected rows.

Expected result:
- No automatic writes occur without explicit apply.
- Successful rows clear; failed rows remain pending with reason.
- Read-only reply rows are cleared and reset to latest bus value.

## Workflow 5: Staged Node Preparation

1. Open layout offline and add a staged node not present in original capture.
2. Save and verify staged node has own `nodes/<NODE_ID>.yaml`.
3. Connect on bench bus and read configuration for staged node.
4. Make changes, save, and later connect to target bus.
5. Confirm staged node rows appear in Sync Panel and apply like normal rows.

Expected result:
- Staged nodes are first-class persisted snapshots.
- Missing staged nodes on target bus remain pending, non-blocking.

## Validation Checklist

- Capture time for <=10 nodes fits SC-001 target.
- Offline open to first node render fits SC-002 target.
- Sync session accounts for 100% pending rows (SC-003).
- Git diff for one-node change mostly touches one node file (SC-007).

## Developer Test Commands

```bash
cd app
npm test

cd src-tauri
cargo test
```
