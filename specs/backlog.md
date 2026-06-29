* LCC Traffic Monitor:
  * When we're getting text, show the actual text along side the bytes
  * Same for other data types
  * Have a check box that will show the byte data with the parsed results.
  * Show names for the message types
* SPROG USB-LCC CDI read timeouts (Issue #14): RESOLVED. Root cause was insufficient
  post-ACK pacing — Bowties sent the next datagram request before the gateway finished
  forwarding the ACK on CAN. Fixed by introducing `datagram_reader.rs` (unified exchange
  with configurable `post_ack_delay_ms` defaulting to 10ms), increasing the read timeout
  from 2000ms → 3000ms, and capping resend retries at 3. Tunable via `tuning.toml` in the
  app data directory.
* MERG CAN ID configuration: JMRI exposes a CAN ID option (100–127, default 126) for MERG
  adapters as an advanced setting. Bowties doesn't expose this yet. Low priority — default 126
  works unless there's a conflict with another host on the same CAN bus.
* Cache Location: The current location on my computer is `C:\Users\john_\AppData\Roaming\com.john.app\cdi_cache`. But that does match what we have in the architecture.md, which calls for `com.bowtiesapp.bowties` to be the directory.
* Add app icon
* Dynamic SNIP & Config
  * If you modify SNIP information from LccPro, for example, the updates should appear right away
  * Same for if you save config from another app. The changes should appear immediately
* Cascade profile rules for ConfigEditor
  * Root cause: ConfigEditor starts as a pass-through (no cascade logic). When a controlling field
    like a daughter board selector changes, dependent fields may need corrective default writes.
    Today this is handled manually or not at all.
  * Fix approach: author cascade rules in `.profile.yaml` alongside existing relevance rules, using
    the same extraction pipeline. ConfigEditor reads these rules and applies synchronous cascade
    corrections within `applyEdit()`.
  * Prerequisite met: edit layer refactor (changes module + ConfigEditor) is complete.
* Release workflow publication polish
  * Root cause: the new skill-based `/release-publish` workflow now owns tag creation and release-notes generation, but the final GitHub draft-release publication step is still a manual paste-and-publish handoff.
  * Follow-up:
    1. Validate that the generated end-user markdown is consistently good enough to paste directly into the GitHub draft release without manual rewriting.
    2. If the manual publication step becomes a recurring pain point, decide later whether to add a verified GitHub CLI path without regressing the simpler manual workflow.
* Connector daughterboard Signal-LCC authoring evidence
  * Root cause: The current implementation ships Signal-LCC aux-port selection and persistence support, but the workspace still does not contain equivalent Signal-LCC CDI/manual path evidence for aux-port-governed sections, so those profiles intentionally leave `affectedPaths` empty.
  * Follow-up:
    1. Acquire concrete Signal-LCC CDI or manual path evidence for aux-port-governed sections and line modes.
    2. Author Signal-LCC affected paths and any carrier-specific overrides once those concrete paths are verified.
* Mixed-use BOD4-CP sampled/output half constraints
  * Root cause: Connector rules now support slot-relative `lineOrdinals`, so Bowties can constrain the detector half of BOD4/BOD4-CP accurately. The remaining gap is richer cross-field modeling for the BOD4-CP sampled/output half (local lines 5-8), where the manual allows multiple valid steady, pulse, and sample combinations depending on the attached device.
  * Follow-up:
    1. Capture concrete Tower-LCC-compatible mappings for the BOD4-CP local lines 5-8 output modes and corresponding sampled input modes.
    2. Extend repair/constraint authoring if needed so Bowties can express output-function and input-function combinations for the BOD4-CP shared lines without hiding valid steady-output use cases.
* JMRI Bridge integration (proposal stage)
  * Draft proposals exist (`specs/proposals/app-ux-vision/jmri-bridge-proposal.md`, `specs/proposals/app-ux-vision/behavior-templates-proposal.md`) exploring bidirectional sync between Bowties channels and JMRI objects (sensors, turnouts, signal masts) via a Jython bridge script.
  * Key design decisions still open: protocol-agnostic channel model (LCC + DCC/LocoNet via JMRI), LogixNG as alternative logic execution target, panel topology import for future layout editor, signal system metadata per channel.
  * No implementation work until proposals are reviewed and scoped.
* Channel hardware references as navigable hyperlinks (ADR-0003 display-reference rule)
  * Root cause: ADR-0003's 2026-06-25 extension establishes that any "node + path" reference in the UI must be a clickable hyperlink that navigates to the configuration field. The current `ChannelRow` hardware line shows resolved text but is not a link.
  * Follow-up:
    1. Design the navigation target: clicking a hardware ref on the Railroad tab should switch to Config tab, select the node, and focus the relevant field/connector.
    2. Implement as a `<button>` that dispatches a navigation action (likely via `configFocusStore` or similar routing mechanism).
    3. Add test coverage for navigation behavior (`ChannelRow.test.ts`).
* Placeholder nodes — generalise planning beyond facility scaffolding
  * Vision-doc reference: `specs/proposals/app-ux-vision/app-ux-vision.md` (Channel Roles, Styles, and Bindings; Placeholder Nodes).
  * Root cause: spec 018's planning capability stops at empty facility slots. The broader vision needs a way to declare "boards I plan to buy" and back channels with their pins/Logic-blocks before any real hardware connects, so the user can configure daughter boards, name channels, apply templates, and aggregate hardware needs (e.g., "you need 3 more LED outputs for this aspect style") without owning any of the boards yet.
  * Approach: extend the current read-only placeholder model into a writable one whose pins/Logic-blocks back channels exactly the way real-node pins do. Channels created against placeholders use the same role/style/binding shape as channels on real nodes; promoting a placeholder to a real node retargets the bindings (existing placeholder-reconciliation flow in the vision).
  * Follow-up:
    1. Lift placeholders to fully writable surfaces (daughter-board selection, channel creation, template application).
    2. Surface a hardware-requirements aggregate over current bindings to placeholder nodes ("buy 3 more LED outputs").
    3. Specify the promote/reconcile UX for binding migration when a real node arrives.
  * Note: the spec-015 `HardwareReference` migration (originally tracked under the now-folded "Channel resource model" backlog entry) is absorbed into spec 018's channel/role/style/binding rebuild and is no longer a separate backlog item. The Railroad-tab `connector-a` slug display fix lands as part of that rebuild.
* Channel/facility persistence atomicity — fold into the atomic save (ADR-0002 follow-up)
  * Root cause: channel CRUD (`createChannels` / `deleteChannels` / `renameChannel`) and `facilitiesStore.loadFacilities` run as separate IPC calls *after* the save orchestrator returns, violating ADR-0002's promise that the backend owns layout-file persistence as one unit. A partial failure in any of these post-save IPCs leaves the layout in a torn state with no rollback.
  * Approach: move channel/facility CRUD inside the backend's atomic save, behind a single IPC. The route stops doing `await deleteChannels()` / etc. after the orchestrator; channel + facility deltas flow into `save_layout_directory` alongside bowtie deltas; partial failure rolls back the whole save.
  * Natural follow-on to the `LayoutState` work ([ADR-0015](../product/architecture/adr/0015-backend-layout-state-single-owner.md)): `LayoutState` already owns channels and facilities in memory, so atomicity falls out of a future `LayoutState::save()` once that method's surface is filled in.
* Channels-panel "Used by" cell — multi-binding overflow ergonomics
  * Root cause: Spec 018 / S3 renders the **Used by** cell as a `; `-separated list of `{facility} / {slot}` pairs to handle multi-binding scenarios (e.g. ABS, where one block-occupancy channel feeds the home signal plus distant and rear-protect signals on adjacent blocks). The format is correct grammatically, but a row with three or more bindings will overflow the cell and force horizontal scroll on the table.
  * Approach: when a binding list exceeds N entries (or measured width), collapse to `Block 5 / Block (input); +2 more` with a hover tooltip listing all entries; allow click-to-expand if the user wants the full list inline. Decide N empirically once multi-binding ships.
  * Prerequisite: Spec 018 / S4 (landed) lights up the column for real (single binding); a future ABS-related feature surfaces the first multi-binding case.
  * Follow-up:
    1. Decide the overflow threshold (count + width).
    2. Implement the collapse + tooltip + click-to-expand affordance in `ChannelRow.svelte`.
    3. Test against the first real multi-binding scenario.
