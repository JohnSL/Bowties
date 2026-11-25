# Automatic Block Signals IPC and Service Contracts

**Status**: Proposed contract extensions. No HTTP endpoints or new apply engine.
**Types**: [data-model.md](../data-model.md).

## Implemented: feedback-release preview (S2R)

`preview_block_signals(request: BlockSignalsPreviewRequest) -> BlockSignalsPreview`

This is the only Automatic Block Signals command that exists today. It backs the Blocks and Aspects
pages of the feedback release. Everything below this section is still the proposed contract for
the slices that review, save and apply (S3 onward).

| Layer | Owner |
|---|---|
| Typed frontend call | `app/src/lib/api/signaling.ts` (`previewBlockSignals`) |
| Tauri command | `preview_block_signals` in `app/src-tauri/src/lib.rs`; delegates to `signaling::preview` |
| Fact gathering | `app/src-tauri/src/signaling/mod.rs`: reads the active effective layout and profiles, resolves detection facts per block |
| Policy | `bowties-core/src/signaling/` (`chain.rs` derives signal blocks, chains, next signals and rules) |

Request (camelCase on the wire; omitted optional fields take the defaults shown):

```text
{
  facilityId,                     // editor identity; signal IDs are derived from it
  facilityName = "",
  direction = eastward,           // the direction the block list is written in
  movements = oneDirection,       // or bothDirections
  blocks: [{ blockId, name, joinedToPrevious = false, turnouts = [{ turnoutId, name }] }],
  entranceEnd = { kind: noPlant } | { kind: plant, plantId },
  exitEnd = { kind: nothingBeyond } | { kind: treatAsStop } | { kind: plant, plantId },
  settings = { aspects: 3, turnoutNotLined: stop, approachLighting: alwaysLit },
  signalOverrides = [{ signalId, aspects, approachLighting }],   // unknown IDs ignored
  signalNames = { signalId: name },                             // missing names get a default
  bindings?                       // S2 hardware selection; kept in the contract, not sent by the UI
}
```

Response: the normalized blocks and ends, one `SignalPreview` per derived signal (both chains when
`bothDirections`), the always-incomplete `capability` (`stage`, `save` and `apply` are
`unavailable`), and `issues`. Each signal carries its governed direction, entrance block, protected
blocks and turnouts, effective settings with an `overridden` flag, its `next` signal or end, and its
rules, most restrictive first. `observation` is always `unavailable`. Live occupancy shown on Blocks
comes from the existing effective-layout observations, not from this command.

Guarantees: no draft, claim, configuration change or bus write. With no layout open the command
still answers from the request alone. Placeholder turnouts are request data, not layout objects, and
always report `turnoutPositionUnavailable`. The frontend keeps only the newest response and drops
responses that arrive after Cancel (`blockSignalsEditorOrchestrator`).

## Existing boundaries to reuse

- `sync_layout_drafts(deltas)` / `clear_layout_drafts()` remain authoritative layout-draft synchronization.
- `save_layout_directory` / `open_layout_directory` remain persistence boundaries.
- `buildSyncSession` / `apply_sync_changes` remain sync planning/application boundaries.
- Existing config read/readback and post-apply reconciliation remain node-observation boundaries.

The current generic `ApplySyncResult` is not sufficient evidence that a multi-node Line is verified. Extend its backend-owned results/recovery linkage; do not invent a frontend success flag.

## Proposed preview boundary

`preview_signal_line_change(request) -> LineChangeProposal`

Request:

```text
{
  operation: create | edit | remove,
  facilityId?: existing stable facility ID,
  input?: { name, diagramDirection, orderedBlockIds, signalsAndBindings },
  expectedLayoutGeneration,
  expectedDraftGeneration
}
```

- Create accepts new editor identities using the existing facility convention; backend validates identity uniqueness.
- Edit/remove require a known Line and current generation.
- The backend resolves detection, node/profile facts, real link addresses, ownership and supported recipe evidence. The caller cannot supply authoritative claims, free slots or hardware validation.
- RG validation covers technical output compatibility and the red/red-plus-green/green mapping, independent of semantic Track Speed. Perceived yellow quality is not an issue code, approval field or reason to block application.
- Return a complete proposal and scoped issues. Missing hardware facts block staging/application, but the editor can display the incomplete preview.
- No persistence, config modification, bus write or lasting resource reservation.

Errors: structured existing IPC error envelope with codes distinguishing invalid request, missing layout/Line, stale generation, unresolved node/configuration, unsupported hardware/recipe, conflict and reference/ownership uncertainty. Include node/signal/resource scope and an actionable explanation.

## Proposed staging boundary

`stage_signal_line_change({ proposalId, revision, expectedLayoutGeneration, expectedDraftGeneration }) -> StagedLineChange`

- Retrieve the exact backend-reviewed proposal; do not trust client-reconstructed patches.
- Revalidate snapshot generations and resource/reference checks.
- Stage extended facility deltas and node configuration changes coherently through the existing draft/offline-change owner. Both are staged or neither is.
- Return authoritative effective facilities, pending node changes and revised draft generation.
- No bus writes or committed hardware claims.

Discard uses the current draft lifecycle and must undo the whole staged change, including pending allocations/releases. Saving uses the existing save boundary and records a pending proposal, not an applied Line.

## Apply and readback extension

Application selects the proposal's generated changes in the existing sync session. Before the first write, ensure the reviewed config preimages still match actual state. A stale/conflicting snapshot is an error requiring fresh review.

Provide persistent backend operation evidence:

```text
{
  operationId, proposalId, revision,
  status: pending | applying | verified | partial | failed | unverified,
  nodes: [{
    nodeId,
    writeStatus: pending | writing | written | verified | failed | unknown,
    changes: [{ changeId, outcome, error?, observedValue? }],
    readback: pending | matches | diverged | unavailable,
    error?, recoveryAction?
  }]
}
```

Progress notifications are keyed by operation and node and describe actual write/readback progress; they are not the durable source of truth. Reopening queries backend-persisted evidence.

Success means all required node patches/releases match readback. Disconnect after a write acknowledgement but before readback is unverified, not fully applied. Mid-node failure retains earlier acknowledged writes. Nodes never visited are not attempted, not failed or updated.

For orientation-only edits, the hardware projection is identical: save metadata through layout persistence, generate zero node patches and require no hardware application.

## Recovery boundary

`preview_signal_line_recovery({ operationId, expectedLayoutGeneration }) -> LineChangeProposal`

Read each affected node through existing config read services, then compare actual values with stored intended values and ownership evidence. Produce a newly reviewable pending delta, or explicit blocked issues. Resume through the same staging/apply boundary. Never blindly replay writes, assume rollback, or free resources on uncertain nodes.

## Core/headless service contract

```text
build_line_proposal(effectiveLayout, edit, resolvedFacts, supportedRecipe)
  -> validated proposal | scoped validation issues
```

This pure service lives in `bowties-core`. Backend preview adapts real state to it. A headless scenario calls the same service and backend lifecycle adapter, including draft/save/apply/readback validation; it cannot bypass resource checks or turn expected values into observations.

Parity compares normalized facility edits, stable identities, downstream links, node patches, resource diffs and issue codes. Request-generated IDs and timestamps alone may be normalized away.

## Contract test obligations

1. Guided/headless equivalent input produces identical normalized proposals.
2. Missing evidence, stale revision, conflicts and uncertain destructive ownership never write hardware.
3. Stage/discard/save preserve coherent facilities and pending node patches.
4. Metadata-only orientation change produces zero writes.
5. Two-node and mid-node failure expose actual progress and persist through reopen.
6. Readback divergence/unavailability prevents verified status.
7. Removal preserves shared detectors and unrelated configuration.
8. RG Approach selects both outputs and retains Approach Track Speed without any subjective yellow-approval requirement.
