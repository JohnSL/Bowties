* LCC Traffic Monitor:
  * When we're getting text, show the actual text along side the bytes
  * Same for other data types
  * Have a check box that will show the byte data with the parsed results.
  * Show names for the message types
* Empty-`element_path` role classifications leak into `bowties.yaml` on save
  * Root cause: `bowties-core/src/bowtie/catalog.rs::extract_catalog_role_classifications` emits a `{nodeKey}:{element_path.join("/")}` key for every Producer/Consumer entry regardless of whether `element_path` is empty. `card.ambiguous_entries` in `catalog.rs` can produce entries with `element_path: vec![]` (protocol-only Ambiguous fallback at L355 / L381 / L416 etc., taken when the slot walk finds no matching slot on the node). If such an entry is later user-classified as Producer/Consumer, the extract emits a key shaped `"0201570002D9:"` — the shape observed in `temp/Test 4/bowties.yaml` on 2026-07-03. On reopen, `merge_layout_metadata` tries to match this key against real ambiguous entries and either mis-matches (any empty-path entry on that node gets reclassified) or silently accumulates as dead metadata.
  * Approach: skip extraction when `entry.element_path.is_empty()`, and (defence-in-depth) skip persisting classifications with empty paths at the YAML boundary. Consider whether the underlying protocol-only-Ambiguous entries should be emitted with empty `element_path` at all, or whether the catalog builder should drop them (they have no actionable slot to remove/rewrite).
  * Prerequisite: none.
  * Follow-up:
    1. Add the empty-path guard in `extract_catalog_role_classifications`.
    2. Migration for existing corrupted YAML: `layout_capture` load path drops role_classifications with empty element_path (log via `load_warnings`).
    3. Regression test on the extract function plus a load-side test that a `bowties.yaml` containing `"0201570002D9:"` opens cleanly without producing spurious reclassifications.
* SPROG USB-LCC CDI read timeouts (Issue #14): RESOLVED. Root cause was insufficient
  post-ACK pacing — Bowties sent the next datagram request before the gateway finished
  forwarding the ACK on CAN. Fixed by introducing `datagram_reader.rs` (unified exchange
  with configurable `post_ack_delay_ms` defaulting to 10ms), increasing the read timeout
  from 2000ms → 3000ms, and capping resend retries at 3. Tunable via `tuning.toml` in the
  app data directory.
  * **2026-07-18 root-cause correction (spec 019 S10):** the SPROG USB-LCC **v1.4** CDI
    failure that later reopened this area was NOT a pacing problem — post-ACK pacing was only
    a symptom mask. The true root cause was a serial `\r\n` framing bug (Bowties appended
    CR/LF after every `;`-terminated GridConnect frame; JMRI sends none, and SPROG v1.4's FTDI
    buffer handling can't tolerate the extra bytes under CDI load). Fixed in `gridconnect_serial.rs`;
    `post_ack_delay_ms` now defaults to 0 (S8) and pacing is retained only as a `tuning.toml`
    escape hatch. See `temp/SESSION-HANDOFF-2026-07-18.md` and ADR-0018 §2026-07-18 extension.
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
* Backend `LayoutState` deep module — implement [specs/proposals/backend-layout-state.md](proposals/backend-layout-state.md)
  * Root cause (architectural): the backend has no single owner for the three-layer in-memory layout model (saved / drafts / live-derived). Persistent node data is scattered across `node_proxy` fields, `node_registry.saved_trees`, and `AppState.offline_bowtie_data`. The save flow walks per-node proxies for snapshots, so any node whose proxy doesn't currently hold CDI (the normal state after every reconnect) is silently dropped from the layout file on save — physically deleting its `nodes/<key>.yaml` and `cdi/<key>.xml`. Confirmed at byte level via the `[BUG-INVEST]` instrumentation on 2026-06-28 (see `temp/Test 3` vs `temp/Test 3 - Copy`).
  * Decision: do the architectural fix, not a symptom patch. Introduce `LayoutState` in `bowties-core/src/layout/` as the single owner; shrink `NodeProxy` to a pure bus-IO actor. See proposal for full design, three-slice migration plan, and ADR implications (new ADR; supersedes parts of ADR-0009; extends ADR-0005 + ADR-0011).
  * Progress (2026-06-28):
    * Slice 1 landed: `bowties_core::layout::state::LayoutState` introduced, `AppState.layout_state` parallel-populated in `open_layout_directory`, cleared in `close_layout`. No callers switched yet. Slice-1 unit tests cover `from_loaded` indexing, `snapshot_for_save` round-trip, and captured-vs-saved CDI precedence.
    * Slice 2 landed: save path now resolves CDI XML length from `LayoutState` (saved or captured layer) when the proxy lacks it, eliminating the `fingerprint == "missing"` data-loss path. `record_captured` is wired at both CDI-download success seams (`cdi.rs`). Slice-2 behavior pins (`r1_every_persisted_node_resolves_cdi_xml_after_open`, `r2_captured_cdi_resolves_for_unsaved_node`, plus capture-layer fingerprint contracts) live in `bowties-core/src/layout/`.
    * Slice 3a landed (2026-06-28): duplicate caches deleted (`LiveNodeProxy::cdi_data` / `cdi_parsed`, `AppState::OfflineBowtieData`). ADR-0015 published.
  * Progress (2026-07-03) — draft-layer activation:
    * `DraftLayer` materialised: `pending_facilities: Option<FacilitiesDocument>` + `pending_channels: Option<ChannelsDocument>` (bowtie-metadata drafts stay frontend-only until a backend read needs them here). `LayoutState::sync_drafts(deltas)` / `clear_drafts()` / `effective_facilities()` / `effective_channels()` added. See ADR-0015 §"2026-07-03 extension".
    * `sync_layout_drafts` / `clear_layout_drafts` IPCs added (`app/src-tauri/src/commands/layout_drafts.rs`). Frontend orchestrator `facilityOrchestrator.{composeBowtiesIfWired, tearDownFacilityBowties}` calls `syncLayoutDrafts` before every `composeFacilityBowties` IPC. `compose_facility_bowties` reads through the effective view — closes the "no bowties compose when facility becomes Wired" S6 bug.
    * `save_layout_directory` refreshes `LayoutState.saved` from the just-written documents and calls `clear_drafts()` inline (otherwise post-save reads through the effective view would base their merge on a stale saved layer).
  * Progress (2026-07-04) — discovered-roles layer; catalog side-channel elimination; config_values deletion:
    * `discovered_roles: BTreeMap<String, RoleClassification>` added to `LayoutState`. Catalog rebuild sites (`build_bowtie_catalog_command`, CDI completion) call `record_discovered_roles()` after extracting resolved classifications from the live catalog. Save flow reads from `LayoutState::discovered_roles()` with `or_insert_with` (user-explicit classifications win). Cleared after save.
    * The entire catalog side-channel merge (`merge_catalog_bowties_into`) was removed from `save_layout_directory`. Bowtie metadata is now exclusively delta-backed. This structurally eliminates the "stale catalog resurrects delta-deleted bowties" bug class.
    * `LiveNodeProxy::config_values` and `SynthesizedNodeProxy::config_values` deleted. The catalog builder now reads EventId values from the authoritative config tree via `collect_event_id_leaves()` (same pattern the offline branch already used). All `GetConfigValues` / `SetConfigValues` / `MergeConfigValues` proxy message variants and their sync sites in `cdi.rs` were removed. This eliminates the "phantom catalog entries after bus writes" bug class caused by `commit_leaf_value` updating the tree but not the stale `config_values` HashMap.
  * Next steps:
    * Route `list_facilities` / `list_channels` through `LayoutState` (see separate backlog item above).
    * Future: add a `live` layer to `LayoutState` for bus-read-back values (drift detection, paging fields like LT-50 macros, status values like track current/voltage). The `discovered_roles` layer establishes the pattern for accumulating protocol-discovered data with clear provenance.
  * Follow-up:
    1. Validate slices 1+2 with manual end-to-end re-run of the R1 and R2 scenarios (open 5-node layout → connect → save no edits → reopen; connect to bus with Tower-LCC, read configs, save). Once verified, remove the `[BUG-INVEST]` instrumentation in [app/src-tauri/src/commands/layout_capture.rs](app/src-tauri/src/commands/layout_capture.rs), [app/src/routes/+page.svelte](app/src/routes/+page.svelte), and [app/src/lib/orchestration/configReadOrchestrator.ts](app/src/lib/orchestration/configReadOrchestrator.ts) (`git grep '\[BUG-INVEST\]'` to find them). The bowties-core slice-2 behavior pins are the durable replacement.
    2. After `snip` / `pip_flags` are assessed, decide whether to move them to LayoutState or keep as bus-IO-only on the proxy. Current assessment: YAGNI — no bug class prevented. Revisit only if a concurrency bug or save-flow concern surfaces.
    3. R2's deeper question (why Tower-LCC's CDI doesn't end up in proxy state after a successful read) is no longer a data-loss issue after slice 2; reassess whether it still matters for live-state correctness.
    4. R3 (no-CDI node still offers "Read Configuration") did not reproduce in the 2026-06-28 session; capture flags on the pre-018 worktree if it surfaces again, then triage separately.
* Route `list_facilities` / `list_channels` IPC reads through `LayoutState.effective_facilities()` / `effective_channels()`
  * Root cause: the frontend baseline for both stores hydrates from IPC commands that read the raw on-disk YAML documents (`bowties_core::layout::read_facilities` / `read_channels`) — they do NOT go through the referentially-clean `LayoutState.saved` / `effective_*` view that every other backend reader (compose, catalog rebuild, sync) now uses. Result: the frontend baseline can diverge from `LayoutState.saved` immediately after open, which surfaced on 2026-07-03 as the "orphan facility slot binding" bug (facility referenced a channel absent from `channels.yaml`; frontend baseline retained the ghost, backend composer did not). The load-time repair in `facilityCascadeOrchestrator.reconcileDanglingChannelRefsOnLoad()` masks the symptom by staging a `detachChannelFromSlot` draft on open, but the underlying asymmetry remains.
  * Approach: route both IPC commands through `AppState.layout_state.read().await.effective_*()` (or, if the frontend baseline should mirror `saved` rather than `effective`, then `saved.*` — decide during design). Once done, revisit whether `reconcileDanglingChannelRefsOnLoad` is still needed: if the frontend always sees the normalised view, the drafts it stages become a defence-in-depth pass that finds nothing.
  * Prerequisite: none — `LayoutState.saved` is already referentially clean (ADR-0002 §2026-07-03 extension).
  * Follow-up:
    1. Route `list_facilities` and `list_channels` through the effective view.
    2. Decide whether the load-time repair pass stays as defence-in-depth or is removed once the seam is symmetric.
    3. Add a behavior test that exercises the "orphan binding on disk" scenario end-to-end.
* `BowtieCatalogPanel.confirmDeleteBowtie` should also reset composed leaves (extend the 2026-07-03 teardown consolidation to user-driven bowtie deletes)
  * Root cause: `resetComposedLeavesForFacility` (2026-07-03) now ensures every teardown call fully reverses composition by resetting consumer leaves via composer-forward or a metadata-driven fallback. The user-facing "Delete this bowtie" action in `BowtieCatalogPanel.svelte` still calls `bowtieMetadataStore.deleteBowtie(hex)` only — it removes the annotation row but leaves the consumer leaves holding the composed event id, so a subsequent CDI-scan catalog rebuild re-produces the card as an auto-discovered orphan. Same class of "incomplete inverse" the teardown consolidation fixed, at a different callsite.
  * Approach: extract a companion primitive `resetLeavesForEventId(eventIdHex)` that shares the tree-scan-by-value logic from the fallback path. Have both `BowtieCatalogPanel.confirmDeleteBowtie` and (as a defence-in-depth pass) `resetComposedLeavesForFacility`'s fallback call it. The panel's confirm dialog can then honestly promise a durable delete.
  * Prerequisite: 2026-07-03 teardown consolidation (landed). Should be done alongside the `list_facilities` routing item because both change the frontend/backend seam symmetry story.
  * Follow-up:
    1. Extract the primitive.
    2. Wire it into `BowtieCatalogPanel.confirmDeleteBowtie`.
    3. Add a regression test: delete a facility-composed bowtie via the panel; save; reopen; assert no orphan card.
* `bowtieCatalogStore` / `layoutStore.bowties` merge — investigate the upstream trigger that produced two catalog cards for one event id
  * Status (2026-07-03): the UI-crashing surface is closed. `buildEffectiveBowtiePreview` now gates the catalog phase on `seenEventIds.has(cardKey)` symmetrically with the other three merge phases (first-wins), so a duplicate `BowtieCard` from any upstream contributor can no longer surface as a Svelte `each_key_duplicate` crash in `BowtieCatalogPanel`. ADR-0010 addendum 2026-07-03 codifies the symmetric-dedup rule; regression `bowties.svelte.test.ts::dedupes duplicate catalog cards with the same event id (first wins)`.
  * Root cause still open: the specific upstream trigger that produced two `BowtieCard`s with the same `event_id_hex` in the 2026-07-03 session (reported alongside the orphan-binding bug — one card with two slots both classified as "Unknown role", another with three slots correctly typed as producer/consumer) has not been reproduced. Hex-format normalisation is closed by ADR-0010's 2026-07-03 `EventIdKey` extension. Remaining suspects: an ambiguous-classification path in the backend catalog builder ([bowties-core/src/bowtie/catalog.rs](bowties-core/src/bowtie/catalog.rs) — `build_bowtie_catalog` + `merge_layout_metadata`); or a race between compose + HMR + `cdi-read-complete`. (Note: the entire catalog side-channel merge was eliminated 2026-07-04 — `merge_catalog_bowties_into` removed; bowtie metadata is now exclusively delta-backed. This eliminates the catalog write-back as a duplicate-card suspect.)
  * Approach: instrument the backend to log when `catalog.bowties` post-`merge_layout_metadata` has any duplicate `event_id_hex`; capture a reproducer trace from the same layout the user was working with (facility B4, single input binding, then attempt add-output-then-save). Once reproduced, fix at the specific source.
  * Prerequisite: none. Not urgent — the crash-shield holds the UI. Prioritise if a user reports a case where the surviving card is materially wrong (e.g. the classification card wins over the well-typed one).
  * Follow-up:
    1. Add a backend diagnostic log at catalog emit time asserting `event_id_hex` uniqueness across `catalog.bowties`.
    2. Reproduce with a real layout and capture the offending card pair.
    3. Fix at the true source and remove the diagnostic once solid.
* Profile-annotation timing race on live proxy trees — close at source
  * Root cause: [app/src-tauri/src/commands/cdi.rs](app/src-tauri/src/commands/cdi.rs) around L2599 rebuilds a live proxy's tree via `build_node_config_tree` (no profile annotation) after a config-value read and stores it back on the proxy; the profile-annotation pass runs later, when the *last* node in the batch completes. Any backend read of the tree between those two events (Spec 018 / S6 facility bowtie composition, and any future reader) sees `event_role: None` on every EventId leaf.
  * Confirmed at runtime (2026-07-03): with a Tower-LCC + Signal-LCC pair connected live, adding a lamp channel to a facility triggers compose. Tower-LCC tree shows `profile_applied=true` with 232 Producer + 232 Consumer + 2 None EventId leaves. Signal-LCC tree shows `profile_applied=false` with all 594 EventId leaves at `event_role: None`. Under the specific Lamp row prefix, both EventId leaves were `None` — confirming the annotation gap is real and not a partial-annotation issue.
  * Interim mitigation (2026-07-03): `bowties-core::facility_bowties::role_matches` is permissive — treats `None` / `Ambiguous` as matching any expected role — so consumer-leaf resolution under a single-role prefix (lamp row, track-circuit row) works against transiently-unannotated trees. `channel_events::collect_from_children` stays strict because it operates on mixed-role prefixes (connector line).
  * Approach: annotate every tree the moment it lands on the proxy (either in `build_node_config_tree`'s wrapper site or in `set_config_tree`), so the strict-role invariant holds unconditionally. Once fixed, revisit the permissive fallback in `facility_bowties::role_matches` (may keep as defence-in-depth or revert to strict).
* Channels-panel "Used by" cell — multi-binding overflow ergonomics
  * Root cause: Spec 018 / S3 renders the **Used by** cell as a `; `-separated list of `{facility} / {slot}` pairs to handle multi-binding scenarios (e.g. ABS, where one block-occupancy channel feeds the home signal plus distant and rear-protect signals on adjacent blocks). The format is correct grammatically, but a row with three or more bindings will overflow the cell and force horizontal scroll on the table.
  * Approach: when a binding list exceeds N entries (or measured width), collapse to `Block 5 / Block (input); +2 more` with a hover tooltip listing all entries; allow click-to-expand if the user wants the full list inline. Decide N empirically once multi-binding ships.
  * Prerequisite: Spec 018 / S4 (landed) lights up the column for real (single binding); a future ABS-related feature surfaces the first multi-binding case.
  * Follow-up:
    1. Decide the overflow threshold (count + width).
    2. Implement the collapse + tooltip + click-to-expand affordance in `ChannelRow.svelte`.
    3. Test against the first real multi-binding scenario.
* Style-driven Lamp Selection auto-lock — deferred from Spec 018 / S5 (D5)
  * Root cause: Spec 018 / S5 (HITL D5) ships the `single-led-direct-lamp` style and the Add-channel atomic flow, but **defers** automatic enforcement of `Lamp Selection` on claimed Direct Lamp Control rows. Today the user must set `Lamp Selection` to the correct pin manually in the Config tab after adding a lamp channel; spec FR-027 / FR-028 / FR-029 (auto-lock the field to the style-required value via the existing validity / relevance mechanism) are explicitly relaxed.
  * Approach: land the `compute_active_styles(layout)` extract in `bowties-core` (the (A) destination from the S5 pre-impl D5 analysis): "what styles are currently active on what CDI surfaces?" becomes a single function over the layout + channels + connector selections, replacing the daughter-board-only style activation today. `collect_validity_rules` becomes a thin projection over `compute_active_styles`. This satisfies both this item and the constraint-filter item below in one step.
  * Prerequisite: a real driver for engine-driven style activation appears (e.g., multi-LED aspect styles where Lamp Selection must be locked across several rows simultaneously). Until then the manual step is the contract.
  * See also the "Profile-declared user-configuration prerequisites" item below — that captures the orthogonal allocation-choice case (which pin drives this lamp?), which this exclusion-constraint item does not cover.
  * Follow-up:
    1. Land `compute_active_styles(layout)` and refactor `collect_validity_rules` to consume it.
    2. Declare `constraints` on the `single-led-direct-lamp` style in `RR-CirKits_Inc._Signal-LCC.profile.yaml` so the engine auto-locks `Lamp Selection` to the style-required value when a row is claimed by a lamp-indicator channel.
    3. Remove the manual-step note from `docs/user/` and the release notes; add a regression test that an active lamp-indicator channel locks the claimed row's `Lamp Selection` field.
* Constraint-filtered eligibility in `eligibleLampRowsForStyle` — deferred from Spec 018 / S5 (D5 knock-on)
  * Root cause: Spec 018 / S5 ships `effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp')` as **unclaimed-only** filtering. The original FR-030 requirement to also exclude rows whose `Lamp Selection == "Used by Mast"` is deferred together with D5 — without engine-driven style activation, the only source of truth for "this row is being used by a mast" lives in the user's manual configuration, which the orchestrator does not introspect.
  * Approach: once `compute_active_styles(layout)` lands (item above), the eligibility derivation can ask "does this row's current Lamp Selection value disqualify it under the lamp-indicator style's constraints?" and exclude it from the picker.
  * Prerequisite: the style auto-lock item above.
  * Follow-up:
    1. Add constraint-driven filtering to `eligibleLampRowsForStyle` once the auto-lock item is closed.
    2. Update the AddChannelPicker empty-state copy to distinguish "no unclaimed rows" from "no constraint-compatible rows".
* Profile-declared user-configuration prerequisites for consumer channels — deferred from Spec 018 / S5 (2026-06-30 quickchange)
  * Root cause: Some consumer styles require the user to make a hardware-allocation choice Bowties cannot infer — e.g. `single-led-direct-lamp` needs the user to set `Lamp Selection` to a specific pin from a shared pool on the Signal-LCC. This is orthogonal to the exclusion-constraint case handled by the "Style-driven Lamp Selection auto-lock" item above: that item locks a field *away from* a bad value; this item guides the user *toward* an allocation Bowties has no way to pre-decide. The 2026-06-30 quickchange removed the temporary in-picker deferral note (`AddChannelPicker.svelte`) that over-fitted the current slice (hard-coded `Lamp Selection`, `Direct Lamp Control`, and `Signal-LCC node` — none of which generalise to future consumer styles); discoverability now leans on `docs/user/` and release notes, so the profile-language gap here is more visible.
  * Approach: needs design. Options to weigh, in rough order of increasing structural depth:
    (A) **Declarative prerequisite list per style** in profile YAML — `postAdditionUserActions: [{ fieldPath, requirement, helpText }]`. Bowties surfaces incomplete items as tooltips on the Channels-panel consumer-channel row and as a blocker on the FacilitySlot filled-state; the AddChannelPicker note (if any) becomes data-driven.
    (B) **In-picker allocation sub-step** — after picking the lamp row, prompt for the allocation field in the same modal and write it on confirm. Picker becomes multi-step and needs profile guidance about which follow-on field(s) to solicit.
    (C) **Post-add "complete configuration" affordance** — newly-created consumer channels start in an `incomplete` sub-state with a "Configure…" affordance that deep-links into the Config tab at the right field.
    (D) **Do nothing structural** — keep manual notes but relocate them per-style; accept that the profile can't express prerequisites for future consumer styles.
  * Open design questions:
    1. Prerequisite vs. exclusion-constraint as one concept or two — reconcile with the existing Style Constraint Contract seam before implementing.
    2. Does an unsatisfied prerequisite block the facility from becoming `Wired`? (Interacts with Spec 018 / S6.) *(2026-07-01 empirical answer: S6 shipped with `facilityStatus` = pure slot fullness — an unsatisfied prerequisite does NOT block Wired today. If a future design chooses (A) or (B) above, this bar will need to rise.)*
    3. Does the concept extend to pin-uniqueness (two lamp-indicator channels shouldn't claim the same physical pin — a resource-pool allocation problem the current constraint engine can't express)?
  * Prerequisite: none technically; independent of the "Style-driven Lamp Selection auto-lock" item, though they'll likely land close together.
  * Follow-up:
    1. Write a short design note or spec covering the profile-language extension. Decide between (A)/(B)/(C)/(D) with an ADR extension to ADR-0013 if the answer is (A)/(B)/(C).
    2. Reconcile with the Style Constraint Contract seam.
    3. When designed, restore an appropriate user-facing affordance (picker, row tooltip, and/or slot state) driven by the chosen mechanism.
* Retire per-callsite `flushDraftToBackend` — Commit 2 of the ADR-0012 2026-07-03 draft-mirror activation
  * Root cause: Commit 1 built `configDraftMirrorOrchestrator` as the sole reactive owner of the config-draft → backend IPC path, but the two legacy callsites (`TreeLeafRow.svelte` leaf-row commit; `BowtieCatalogPanel.svelte` `handleNewConnection` + `handleClearConnection`) still call `flushDraftToBackend` directly. The mirror already emits the same `setModifiedValue` for those same edits, so the legacy calls are now redundant work rather than the source of correctness. Leaving them in place indefinitely keeps a second, silent contributor to the seam alive — the exact "someone forgot to flush" pattern the mirror exists to prevent from re-emerging under a different name.
  * Approach: after Commit 1 has soaked (no user reports of missed writes, no test flakes tied to the mirror), delete the three `flushDraftToBackend` invocations, delete the exported function from `configDraftOrchestrator.ts`, delete the `flushDraftToBackend` describe block in `configDraftOrchestrator.test.ts`, and delete the `flushDraftToBackend` mocks from `TreeLeafRow.test.ts`, `TreeLeafRow.offline.test.ts`, and `BowtieCatalogPanel.test.ts`. Shape 2 (`applyEdit` calls the mirror), Shape 3 (keep per-callsite flushes), and Shape 4 (fold mirror into `configDraftOrchestrator`) were considered and rejected on 2026-07-03 — see ADR-0012 §"2026-07-03 extension: connected-mode draft-to-backend mirror".
  * Prerequisite: Commit 1 landed (this session). No user-facing behaviour change expected.
  * Follow-up:
    1. Delete the three call sites and the exported function.
    2. Delete the associated tests and mocks.
    3. Confirm the full app test suite still passes.
* Batched `set_modified_values` IPC + emission coalesce for the Config Draft Backend Mirror
  * Root cause: `configDraftMirrorOrchestrator` (ADR-0012 2026-07-03 extension) emits one `setModifiedValue` IPC per new/changed draft. Facility composition writes 2 consumer leaves per bowtie in the same reactive tick, so a facility with N bowties emits 2N IPCs. Fine for current layout sizes but wasteful.
  * Approach: introduce a batched `set_modified_values(nodeId, entries[])` IPC (backend accumulates into `NodeProxy.modified_value` in one lock cycle); have the mirror coalesce all pending emissions from a single reactive tick into per-node batches before invoking IPC. MUST replace the mirror's current per-draft emission, not run in parallel — the single-owner property of the seam is what makes the "someone forgot to flush" bug class impossible. Defer until profiling shows a real cost.
  * Prerequisite: Commit 2 above (retire `flushDraftToBackend`) — do not add a batching seam while a legacy per-draft flush still exists.
  * Follow-up:
    1. Measure the IPC cost on a realistic layout (Tower-LCC + Signal-LCC, several facilities).
    2. If material, add the batched IPC + coalesce.
    3. Update ADR-0012 §"2026-07-03 extension" with the batching contract.
* "Flush pending drafts on connect" for the Config Draft Backend Mirror
  * Root cause: `configDraftMirrorOrchestrator` reads `layoutStore.isConnected` inside its effect body, NOT as a reactive dependency, so a user who edits while offline then connects does not automatically flush the backlog to the backend. This is intentional (the mirror is not a re-emit engine — its diff advances even when offline so reconnects do not fire spurious IPCs), but it does mean an offline-edit → connect → connected-save sequence relies on `stageDraftsForOfflineSave` at offline-save time. If we later want the mirror to also observe connect transitions and flush pending offline drafts to the backend on connect (so the user can save without switching to offline mode first), that is a separate deliberate feature.
  * Approach: needs design. Options include (A) a one-shot `flushOnConnect()` hook that walks `configChangesStore.draftEntries()` and emits `setModifiedValue` for each non-placeholder key, wired to a `layoutStore.isConnected` transition observer; (B) an explicit user affordance ("Push offline edits to bus") that keeps the timing under user control; (C) leaving the current offline-vs-online contract intact and treating the workflow as "close and reopen in the appropriate mode".
  * Prerequisite: none; independent of Commit 2 and the batching item above.
  * Follow-up:
    1. Decide whether the feature is worth building (real user report? Or a theoretical smoothness win?).
    2. If yes, pick option A/B/C and add an ADR-0012 extension covering the connect-transition contract.
