# Layout module owns all companion-dir file structure

## Context

ADR-0001 → ADR-0004 reshaped the **save data flow** so the backend is the sole owner of layout file *content*: deltas in, persisted `LayoutFile` out. The backend command surface (`commands/layout_capture.rs`, `commands/sync_panel.rs`, `commands/cdi.rs`) became the only path that mutates layout files.

That fixed *who decides what to write*. It did not fix *who knows the on-disk shape*. Before S2d, knowledge of the companion-directory layout — that node snapshots live under `<base>.layout.d/nodes/<UPPER_HEX>.yaml`, that offline edits live in `offline-changes.yaml`, that bowtie metadata lives in `bowties.yaml`, that CDI XML lives under `cdi/` — was duplicated across three command modules. Each call site independently constructed `derive_companion_dir_path(...).join("nodes").join(...)` and called the low-level `write_yaml_file` directly:

- `commands/sync_panel.rs` re-derived companion paths in 3 places to persist `offline-changes.yaml` and per-node snapshot baselines.
- `commands/cdi.rs` re-derived companion paths in 2 places to merge snapshot values into freshly-built node trees and to persist snapshot updates after live writes.
- `commands/layout_capture.rs` re-derived companion paths to load single snapshots and to resolve CDI XML during offline catalog rebuild.

This duplication was load-bearing for the next slice (S2e). The Dropbox/OneDrive sharing-violation failure reported by a user is rooted in the directory-rename step at the end of `save_directory_atomic`. The intended fix is a write-ahead journal with in-place file writes (no `.staging` directory rename). For that journal to be reliable, **every** file write into a layout must flow through it. With path/format knowledge duplicated in three command modules, any single bypass — including the partial-update writes that already exist — would silently break recovery.

## Decision

**The `layout/` module is the sole owner of the companion-directory file structure.** It exposes an intent-shaped public API in `layout/mod.rs`:

- `save_capture(base_file, &LayoutDirectoryWriteData)` — full layout save.
- `read_capture(base_file) -> LayoutDirectoryReadData` — full layout read.
- `read_node_snapshot(base_file, canonical_node_id) -> NodeSnapshot` — single-node read.
- `update_offline_changes(base_file, &[OfflineChange])` — partial update of `offline-changes.yaml`.
- `update_node_snapshots(base_file, &[NodeSnapshot])` — partial update of one or more node snapshot files; other snapshots untouched.
- `resolve_cdi_xml_for_snapshot(base_file, snapshot, app_data_dir) -> String` — CDI lookup (global cache first, layout `cdi/` second).

All path derivation (`derive_companion_dir_path`, `derive_node_file_path`, `derive_companion_dir_name`), filename constants (`BOWTIES_FILE`, `OFFLINE_CHANGES_FILE`, `EVENT_NAMES_FILE`, `NODES_DIR`), low-level YAML I/O (`write_yaml_file`, `read_yaml_file`), and the directory-swap primitive (`save_directory_atomic`) are `pub(crate)` and visible only inside `layout/`. The single-file `load_file`/`save_file` helpers (used by `commands/bowties.rs` for stand-alone `.bowties.yaml` documents — a different format that does not use a companion directory) remain `pub`. The global app-data CDI cache path (`cdi_cache_path`) also remains `pub` because it is not part of the per-layout structure.

The partial-update APIs go through the same write code path as the full save. Today that path is "write_yaml_file → temp + flush + rename". After S2e it will be "write_yaml_file → journaled in-place write". Either way, **every** mutation of a layout file flows through one place.

## Considered options

- **Leave path derivation public and add a code-review rule.** Rejected: this is the status-quo that produced the bug. Code review does not survive multi-session AI development or contributor turnover, and the next sharing-violation regression would not be detectable from a diff that "looks reasonable".

- **Move only `update_offline_changes` and `update_node_snapshots` to the layout module but leave path helpers public.** Rejected: the journal in S2e still has bypass paths, which defeats the point. The cost of also making the path helpers `pub(crate)` is zero — no caller outside `layout/` has a legitimate reason to know the on-disk layout.

- **Expose the partial-update APIs as new backend commands directly.** Rejected: this confuses *IPC boundary* with *file-structure ownership*. The frontend never asks for partial updates — it asks for "save my edits" or "apply this sync result". The backend command modules decide *when* to update; the layout module decides *how*.

## Consequences

**Positive**
- S2e's journal can be introduced as a single change inside `layout/mod.rs` (and the existing internals it routes through) and instantly covers every persistence path, including the partial-update writes done during sync apply and live-write snapshot baseline updates.
- New backend commands that need to read or write layout data have one obvious entry point and cannot accidentally introduce a divergent path.
- The `layout/` module is now a deep module by the Ousterhout definition: a small public surface (six intent-shaped functions) hides a large amount of implementation (YAML serialization, atomic directory swap, CDI cache fallback, schema-version validation, companion-directory naming for both legacy and current suffix conventions).
- Test coverage: the partial-update APIs share the round-trip property "read what was written, regardless of which API wrote it" with the full save — encoded in `layout::tests` so any future refactor preserves it.

**Negative**
- Callers that need both a snapshot read and a CDI resolution against the same companion directory pay a tiny duplicated `derive_companion_dir_path` call (currently a string-format on the file stem). Negligible.
- `LayoutManifest::new` still takes a `companion_dir: String` parameter, but `save_capture` overrides it from the base-file name. The command surface now passes an empty string. A future cleanup may remove the parameter entirely, but doing so was out of scope for S2d's no-behavior-change refactor.

## Status

Accepted (2026-05-23) — slice S2d.
