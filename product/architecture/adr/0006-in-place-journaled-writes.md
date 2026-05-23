# Companion-directory saves use in-place journaled writes

## Context

The pre-S2e save path used `save_directory_atomic` to publish a fresh copy of the companion directory: write all files into a sibling `<base>.layout.staging/` directory, then rename the current `<base>.layout.d/` to `<base>.layout.backup/`, rename staging into place, and finally delete the backup. The directory rename was the commit point, picked for the same reason `write_yaml_file` uses temp-file + rename for individual files: a single atomic operation on Windows and POSIX file systems.

That assumption holds for files on a real local file system. It does **not** hold for cloud-mirrored folders. A Bowties user storing their layout under Dropbox reported repeated save failures with `MoveFileEx` returning Windows error 32 (`ERROR_SHARING_VIOLATION`). The Dropbox sync agent — and the same pattern occurs with OneDrive and Google Drive — keeps file handles open against directory contents while uploading deltas. Renaming the parent directory races against that scanner. Per-file writes already worked because `write_yaml_file` retries via temp + rename; the directory-level rename had no retry and no way to coordinate with the external scanner.

A surface-level fix (retry loop around the directory rename, or a short sleep) was rejected: it would mask the real issue, leave a window where partially-renamed state is visible to the sync agent, and still fail under heavy upload pressure. The deeper problem is that *renaming a directory* is the wrong primitive for "publish new file contents" when that directory is being watched.

## Decision

**The companion directory is mutated in place under a write-ahead journal.** The directory is never renamed during a save. Individual files are overwritten directly (no per-file temp + rename either), with crash recovery driven by a marker file and a `.restore/` mirror of pre-existing contents.

The protocol is owned by `app/src-tauri/src/layout/journal.rs` and exposed only inside `layout/`:

- `pub(crate) fn execute(plan: SavePlan) -> Result<(), String>`
- `pub(crate) fn recover_if_needed(base_file: &Path) -> Result<bool, String>`

A `SavePlan` is a flat list of `PlannedWrite`s (each carrying an absolute path and either `WriteOp::Bytes(_)` or `WriteOp::CopyFrom(_)`) plus a list of `PrunePlan`s describing per-directory extension filters and a `keep_abs` allowlist used to compute deletions of stale files (extra node snapshots, extra CDI XML).

`execute` runs eight phases:

1. Best-effort cleanup of legacy `<name>.staging/` and `<name>.backup/` siblings left over from the pre-S2e scheme.
2. Ensure the companion directory exists; run `recover_if_needed` first to roll any prior interrupted save before touching it.
3. Clean and recreate `<companion>/.restore/`.
4. Walk the writes and prune plan to build the journal `entries` (each entry records its absolute path, a `.restore/NNNN.bin` slot if `pre_existed`, and whether it is a delete).
5. Write the marker `<companion>/.save-in-progress` with `phase: preparing` and fsync.
6. Copy every pre-existing target into `.restore/NNNN.bin` and fsync each one + the directory.
7. Flip the marker to `phase: writing` and fsync.
8. Apply writes and deletes in plan order, then delete the marker and `.restore/`.

`recover_if_needed` is invoked at the start of `read_capture`. If the marker is missing, no recovery is needed. If the marker says `phase: preparing`, no destructive writes ran yet — just delete the marker and `.restore/`. If the marker says `phase: writing`, walk the journal entries: for every `pre_existed` entry restore from `.restore/NNNN.bin`; for every newly-created entry (no `pre_existed`) delete the partial target. Then delete the marker and `.restore/`. A boolean `recovery_occurred` flag is plumbed up through `LayoutDirectoryReadData` → `OpenLayoutResult` → the frontend `OpenLayoutResult` type so the UI can surface a toast.

All Bowties persistence paths route through `journal::execute`: full saves (`save_capture`), partial offline-change writes (`update_offline_changes`), partial snapshot writes (`update_node_snapshots`). The pre-S2e helpers `save_directory_atomic` and the temp-rename codepath in `write_yaml_file` were removed; the legacy staging/backup constants were dropped.

## Considered options

- **Keep the directory rename and add a retry loop with backoff.** Rejected: the failure window is owned by the cloud-sync agent, not Bowties. No bounded retry loop can guarantee success, and the partially-renamed state is visible to the agent during the race. Repeated rename attempts on a 50+ MB layout also generate substantial spurious sync traffic.

- **Keep per-file temp + rename but stop renaming the directory.** Rejected: temp + rename still produces a sibling file (`foo.yaml.tmp` → `foo.yaml`) that the sync agent uploads and then deletes, doubling upload traffic. More importantly, it leaves no transactional boundary across multiple files: a crash mid-save would leave a mix of new and old contents with no marker to recover from.

- **Move the layout out of the user's chosen folder into an internal app-data location and sync from there.** Rejected: the layout file is a user document. Users expect to keep it next to their other modeling files, share it via Dropbox/OneDrive, and pick its location.

## Consequences

**Positive**
- No directory rename, so cloud-sync agents never see the companion directory move. The reported Dropbox error 32 is gone.
- The marker + `.restore/` give a precise, testable transactional boundary across the multi-file save. Two test seams (`FAIL_AFTER_BACKUP`, `FAIL_MID_WRITES`, both thread-local under `#[cfg(test)]`) drive the crash-and-recover round-trips that prove rollback correctness.
- Files that did not change are not rewritten. With cloud sync, this drops per-save upload size from "every YAML in the layout" to "only the files that actually changed".
- A single owner (`layout/journal.rs`) makes the write protocol auditable. ADR-0005 already locked down `layout/` as the sole owner of companion-directory file structure; this slice gives that owner a single mutation primitive.

**Negative**
- More fsyncs per save (marker flip, every backup, every write, the directory). On a local SSD this is fast; on a network-mounted home directory it adds latency. Acceptable for the reliability gain.
- Recovery now requires the read path to check for a marker on every open. The cost is one `Path::exists` call when no marker is present.
- `.save-in-progress` and `.restore/` appear briefly inside the user's layout folder during a save. They are removed on completion or on the next open via recovery, but a sync agent may upload and then delete them. Names are chosen to make their purpose obvious.
- A save interrupted between the `phase: writing` flip and the first write is recovered correctly (no `pre_existed` entry has been touched), but the user's *intent* — the new contents — is lost. This matches the pre-S2e behavior: a partial save was never committed. The new toast on next open ("Previous save was interrupted and has been restored.") makes the loss explicit.

## Status

Accepted (2026-05-23) — slice S2e.
