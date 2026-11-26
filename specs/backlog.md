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
* Placeholder nodes — generalise planning beyond facility scaffolding
  * Vision-doc reference: `specs/proposals/app-ux-vision/app-ux-vision.md` (Channel Roles, Styles, and Bindings; Placeholder Nodes).
  * Root cause: spec 018's planning capability stops at empty facility slots. The broader vision needs a way to declare "boards I plan to buy" and back channels with their pins/Logic-blocks before any real hardware connects, so the user can configure daughter boards, name channels, apply templates, and aggregate hardware needs (e.g., "you need 3 more LED outputs for this aspect style") without owning any of the boards yet.
  * Approach: extend the current read-only placeholder model into a writable one whose pins/Logic-blocks back channels exactly the way real-node pins do. Channels created against placeholders use the same role/style/binding shape as channels on real nodes; promoting a placeholder to a real node retargets the bindings (existing placeholder-reconciliation flow in the vision).
  * Follow-up:
    1. Lift placeholders to fully writable surfaces (daughter-board selection, channel creation, template application).
    2. Surface a hardware-requirements aggregate over current bindings to placeholder nodes ("buy 3 more LED outputs").
    3. Specify the promote/reconcile UX for binding migration when a real node arrives.
  * Note: the spec-015 `HardwareReference` migration (originally tracked under the now-folded "Channel resource model" backlog entry) is absorbed into spec 018's channel/role/style/binding rebuild and is no longer a separate backlog item. The Railroad-tab `connector-a` slug display fix lands as part of that rebuild.
* Backend `LayoutState` deep module — complete the remaining work from the [historical backend layout-state proposal](archive/backend-layout-state.md)
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
    * Future: add a `live` layer to `LayoutState` for bus-read-back values (drift detection, paging fields like LT-50 macros, status values like track current/voltage). The `discovered_roles` layer establishes the pattern for accumulating protocol-discovered data with clear provenance.
  * Follow-up:
    1. Validate slices 1+2 with manual end-to-end re-run of the R1 and R2 scenarios (open 5-node layout → connect → save no edits → reopen; connect to bus with Tower-LCC, read configs, save). Once verified, remove the `[BUG-INVEST]` instrumentation in [app/src-tauri/src/commands/layout_capture.rs](../app/src-tauri/src/commands/layout_capture.rs), [app/src/routes/+page.svelte](../app/src/routes/+page.svelte), and [app/src/lib/orchestration/configReadOrchestrator.ts](../app/src/lib/orchestration/configReadOrchestrator.ts) (`git grep '\[BUG-INVEST\]'` to find them). The bowties-core slice-2 behavior pins are the durable replacement.
    2. After `snip` / `pip_flags` are assessed, decide whether to move them to LayoutState or keep as bus-IO-only on the proxy. Current assessment: YAGNI — no bug class prevented. Revisit only if a concurrency bug or save-flow concern surfaces.
    3. R2's deeper question (why Tower-LCC's CDI doesn't end up in proxy state after a successful read) is no longer a data-loss issue after slice 2; reassess whether it still matters for live-state correctness.
    4. R3 (no-CDI node still offers "Read Configuration") did not reproduce in the 2026-06-28 session; capture flags on the pre-018 worktree if it surfaces again, then triage separately.
* Make manual bowtie deletion durably break its event connections
  * Root cause: deleting metadata alone leaves consumer event IDs in configuration, so a later catalog rebuild may rediscover the event. Spec 021 S5 preserves old names/events during migration and retires facility bowtie teardown; manual deletion remains a separate workflow.
  * Approach: decide and test a dedicated manual event-connection reset primitive before changing `BowtieCatalogPanel.confirmDeleteBowtie`; do not restore the retired Block Indicator composer.
  * Prerequisite: none.
  * Follow-up:
    1. Extract the primitive.
    2. Wire it into `BowtieCatalogPanel.confirmDeleteBowtie`.
    3. Add a regression test: delete a named manual bowtie; save; reopen; assert the promised connection-removal behavior.
* `bowtieCatalogStore` / `layoutStore.bowties` merge — investigate the upstream trigger that produced two catalog cards for one event id
  * Status (2026-07-03): the UI-crashing surface is closed. `buildEffectiveBowtiePreview` now gates the catalog phase on `seenEventIds.has(cardKey)` symmetrically with the other three merge phases (first-wins), so a duplicate `BowtieCard` from any upstream contributor can no longer surface as a Svelte `each_key_duplicate` crash in `BowtieCatalogPanel`. ADR-0010 addendum 2026-07-03 codifies the symmetric-dedup rule; regression `bowties.svelte.test.ts::dedupes duplicate catalog cards with the same event id (first wins)`.
  * Root cause still open: the specific upstream trigger that produced two `BowtieCard`s with the same `event_id_hex` in the 2026-07-03 session (reported alongside the orphan-binding bug — one card with two slots both classified as "Unknown role", another with three slots correctly typed as producer/consumer) has not been reproduced. Hex-format normalisation is closed by ADR-0010's 2026-07-03 `EventIdKey` extension. Remaining suspects: an ambiguous-classification path in the backend catalog builder ([bowties-core/src/bowtie/catalog.rs](../bowties-core/src/bowtie/catalog.rs) — `build_bowtie_catalog` + `merge_layout_metadata`); or a race between compose + HMR + `cdi-read-complete`. (Note: the entire catalog side-channel merge was eliminated 2026-07-04 — `merge_catalog_bowties_into` removed; bowtie metadata is now exclusively delta-backed. This eliminates the catalog write-back as a duplicate-card suspect.)
  * Approach: instrument the backend to log when `catalog.bowties` post-`merge_layout_metadata` has any duplicate `event_id_hex`; capture a reproducer trace from the same layout the user was working with (facility B4, single input binding, then attempt add-output-then-save). Once reproduced, fix at the specific source.
  * Prerequisite: none. Not urgent — the crash-shield holds the UI. Prioritise if a user reports a case where the surviving card is materially wrong (e.g. the classification card wins over the well-typed one).
  * Follow-up:
    1. Add a backend diagnostic log at catalog emit time asserting `event_id_hex` uniqueness across `catalog.bowties`.
    2. Reproduce with a real layout and capture the offending card pair.
    3. Fix at the true source and remove the diagnostic once solid.
* Profile-annotation timing race on live proxy trees — close at source
  * Root cause: [app/src-tauri/src/commands/cdi.rs](../app/src-tauri/src/commands/cdi.rs) around L2599 rebuilds a live proxy's tree via `build_node_config_tree` (no profile annotation) after a config-value read and stores it back on the proxy; the profile-annotation pass runs later, when the *last* node in the batch completes. Any backend read of the tree between those two events (Spec 018 / S6 facility bowtie composition, and any future reader) sees `event_role: None` on every EventId leaf.
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
    3. When designed, restore an appropriate user-facing affordance (picker, row tooltip, and/or slot state) driven by the chosen mechanism.
* Signaling UX — glossary gap in [product/glossary.md](../product/glossary.md)
  * Root cause: **signal type**, **head**, **section**, and **signalling system** are load-bearing terms in the [signaling UX charter](proposals/signaling-ux/goals-and-constraints.md) and [proposal](proposals/signaling-ux/signaling-ux-proposal.md) but have no glossary entries yet. (**Aspect** and **Indication** were added by Spec 020; **head** appears only as an _Avoid_ term under **Signal**.)
  * Follow-up:
    1. Add glossary entries for the four remaining terms before the proposal is treated as settled.
    2. Cross-reference the proposal §1 (signal type) and §4 (system-level terms).
* Signaling UX — extract the Signal-LCC manual's ABS/APB worked examples
  * Root cause: the manual's section 9, pages 35–47, contains worked ABS and APB configuration examples that were skipped in `profile-extractions/signal-lcc/recipes.yaml`. This is the highest-value gap in the current Signal-LCC profile extraction.
  * Follow-up:
    1. Extract the examples into `product/hardware/signal-lcc/` so the signaling UX proposal and future mockups can cite them.
    2. Confirm the extracted recipes match the cascade and condition derivation in `signaling-ux-proposal.md` §3.
* Bowtie catalog reads configuration through the effective view
  * The catalog (`app/src-tauri/src/commands/bowties.rs` ~385-450) builds its own overlay of Pending Changes on event leaves. Move it onto `LayoutState::effective_config_tree` (ADR-0015 2026-11-23) so unsaved drafts and Pending Changes resolve the same way for every backend reader.
  * The `modified_value` layer is gone (Spec 020 E3b-1) and `LayoutState` is the single owner of Pending Changes (E3b-0); this overlay is the last parallel reader. It already uses the shared still-waiting predicate.
* Railroad editor walk-through tests time out under full-suite load
  * In full `npx vitest run`, `RailroadPanel.panel.integration.test.ts` "creates a Wired panel with three lamp channels…" and `RailroadPanel.integration.test.ts` "shows the rules of a joined block with a turnout on Aspects" exceed the 5 s default timeout (≈5.0–5.5 s); both pass alone in under 2 s. Seen 2026-11-26 after the shared `SignalLcc/PinGrid` landed.
  * Approach: follow the aiwiki/owners.md guidance — seed saved state (`railroadPanelTestSupport.seedSavedBlockSignals`-style) instead of walking every editor step, or split the walks; raise a per-test timeout only as a stopgap. The Panel create walk already carries a 15 s stopgap timeout (Spec 021 / S2); remove it once the walk is shortened. Spec 021 S4 added five more Panel walks (edit, same-board move, block removal, Cancel, Release) with the same 15 s stopgap.
* Facility removal sequencing lives in `FacilitiesSection.svelte`
  * `startRemoval` / `confirmRemoval` (Spec 021 S4) sequence preview → dialog → `removeFacility` → re-preview on `planChanged` inside the component. Move that sequencing behind one `facilityOrchestrator` (or a small removal-dialog store) action so the component only renders the dialog state and emits confirm/cancel.
* Removing an Automatic Block Signals facility with board Logic placement has no backend test
  * `compose_facility_removal` (Spec 021 S4) carries over the Logic-placement filter of the old ABS removal verbatim, but no Rust test removes a facility whose groups appear in `logicPlacement`. Add one in `app/src-tauri/src/signaling/compose.rs`.
