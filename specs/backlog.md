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
* Channel resource model — generalize HardwareReference (proposal: `specs/proposals/app-ux-vision/channel-resource-model.md`)
  * Root cause: the spec-015 `HardwareReference { nodeKey, slotId, inputOrdinal }` shape is Tower-LCC-specific and cannot represent Signal LCC lines, LED drivers, signal masts, or output channels generally. `ChannelCard.svelte` also renders the raw `connector-a` slug as a display label, leaking the storage form.
  * Approach: introduce a purpose-typed Resource layer (system catalog: `occupancy`, `signal-aspect-3-color`, `led-output`, `button-input`, `mast`, …) with one-or-more signatures per type. Channels reference resources by id; the channel never sees CDI fields. Resources come from one of three paths: profile-pre-instantiated, profile-slot-template, or user-mapped (for unprofiled boards). See the proposal for the full three-layer model.
  * Follow-up:
    1. Review and refine the proposal; convert open questions to decisions.
    2. Profile schema: declare a per-node resource catalog (pre-instantiated resources + slot templates) replacing the current `channelInputs` block. Fold the existing `eventMapping` into signature field roles.
    3. `channels.yaml` schema: bump to 2.0; replace `hardwareRef` with `resourceRef`; one-shot migration for v1.0 Tower-LCC entries (`(connector-a, N)` → `ca-input-N`, `(connector-b, N)` → `cb-input-N`).
    4. Backend: rewrite `resolve_channel_event_ids` as a resource lookup. Add a constraint engine that activates a resource's active-state rules when a channel binds it.
    5. Frontend: render the profile-supplied `resource.label` in `ChannelCard.svelte` (fixes the original display leak). Reframe the Mockup 2 picker as resource-type/signature picker. Add a user-mapped resource flow for unprofiled boards.
  * Note: the visible `connector-a` slug in the Railroad tab subtitle is deliberately deferred to this migration rather than patched separately. The display fix is one body-of-code change at the same call site as the data-model swap; doing it now would require a bridge helper with a `(connector, input)` signature we'd then back out. The Railroad tab is unreleased, so users aren't seeing the slug in any shipped build.
