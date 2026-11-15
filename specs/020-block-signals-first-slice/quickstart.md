# Automatic Block Signals Quickstart and Acceptance Walkthrough

**Status**: Planned workflow; not a claim that the feature exists today.

## Before implementation acceptance

Before finalizing hardware-dependent design, run the small [single-board S4r/S3r probe](manual-track-circuit-test.md), then [repeat across two boards](manual-two-board-test.md) with one receiver-restart check. Use factory-reset Signal-LCC test slots and copy generated IDs between named fields; leave Tower-LCC detectors untouched. The broad acceptance progression below comes later, not before design. Perceived blended yellow is the modeler's choice, not a gate; no flashing proxy is needed.

## Modeler walkthrough

1. Open/save an existing layout and connect compatible Signal-LCC nodes using the normal Bowties connection workflow.
2. Make sure named blocks already have usable detection and suitable outputs/resources are available. Detector creation and arbitrary lamp/display types are outside this flow.
3. Create a named **Automatic Block Signals**:
   - **Blocks**: enter one or more existing blocks in travel order; choose eastward/westward diagram orientation.
   - **Signals**: name the stable entrance signal for each block.
   - **Display**: use the supported two-driver RG searchlight: red=Stop, red+green=Approach, green=Clear. Choose the color-quality/output-resource tradeoff yourself; no yellow-approval step is required.
   - **Bindings**: assign each signal's host and separate red/green outputs.
   - **Confirm**: review protected blocks, expected behavior, real outputs, changes, claims/releases and every affected node.
4. Resolve all blocking issues. Save a draft separately from applying it. No hardware change occurs just because the wizard closes or the layout saves.
5. Apply through the existing sync lifecycle, then read back. Treat pending, partial and unverified results as such.
6. Change occupancy and inspect physical appearances plus actual published/received restrictions. Preview expectations are not live observations.
7. Reopen the saved/read-back Line. Rename it and change only orientation; block order, IDs, bindings and hardware must remain unchanged.
8. Remove only after reviewing owned resources and references; shared blocks/detection remain.

## Acceptance progression

- **One block**: occupied Stop; clear Clear. No downstream protection is implied.
- **Append a second block**: retain the original signal identity/name/compatible outputs. Clear upstream + downstream Stop -> Approach; downstream Approach/Clear -> Clear.
- **Three blocks, two nodes**: all clear -> Clear/Clear/Clear; final occupied -> Clear/Approach/Stop; middle occupied -> Approach/Stop/terminal state. During acceptance, exercise all 8 occupancy combinations with own occupied -> Stop, clear plus next Stop -> Approach, otherwise Clear.
- **Two-board variations**: compare same-board and cross-board reception; swap upstream/downstream hosts; then place S1/S2/S3 on A/B/A to test both cross-board hops. Record startup, single-board restart, Bowties disconnection and node input-loss/recovery separately.
- **Maintenance**: prepend/reorder/rebind/remove a nonfinal block; review old/new host changes, retain unaffected identities and release only proven ownership. Last-block removal is Line removal.
- **Failures**: conflicting outputs, capacity shortage, missing detection, unresolved addresses, unsupported hardware, stale review, uncertain external references and unavailable observations block unsafe application.
- **Recovery**: interrupt after one node and within a later node; identify actual written, unattempted and unknown patches, read back and review a safe new recovery proposal.
- **Parity**: run equivalent headless scenarios through the same proposal and lifecycle service; compare normalized identities, links, patches, resource diffs and validation issues.

Repeat physical acceptance for both diagram orientations without reversing travel order. Require 100% rule/output/restriction table agreement and truthful failure status, not a judgment of yellow hue. For the review usability check, at least 4 of 5 unfamiliar modelers must identify protected block, expected indication and wiring without raw configuration.

## Developer starting point

Read [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md) and [contracts](contracts/tauri-ipc.md). Continue with design/slicing before implementation. Use the existing testing strategy and owner-level TDD; no dependencies, production behavior or hardware support were added by this planning run.
