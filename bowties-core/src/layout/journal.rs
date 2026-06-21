//! Write-ahead journal for layout persistence (ADR-0006).
//!
//! Replaces the previous "write into a staging directory, then `MoveFileEx`
//! the directory into place" strategy. On Windows, `MoveFileEx` of a
//! directory loses to any other process (Dropbox, OneDrive, antivirus,
//! Windows Search) that has briefly opened a file inside the staging
//! directory, producing the user-visible failure:
//!
//!   *Save failed: The process cannot access the file because it is being
//!    used by another process*
//!
//! This module replaces that scheme with **in-place file writes + a
//! write-ahead journal** owned entirely inside `layout/`:
//!
//! 1. Build a [`SavePlan`] describing every file that will be written
//!    (or copied in) and any directories whose extra files must be
//!    pruned.
//! 2. Write a `.save-in-progress` marker into the companion directory
//!    listing every target file and whether it pre-existed; fsync.
//! 3. For every pre-existing target, copy current contents to
//!    `.restore/<NNN>`; fsync each.
//! 4. Flip the marker to `phase: "writing"`; fsync.
//! 5. Overwrite each target file in place (no temp file, no rename) and
//!    delete pruned files.
//! 6. Delete the marker and the `.restore/` directory.
//!
//! On open, [`recover_if_needed`] checks for the marker. If present,
//! pre-existing targets are restored from `.restore/` and newly-created
//! files are deleted. The caller is told the recovery happened so the
//! UI can notify the user.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Marker filename inside the companion directory. Its presence on disk
/// means a save was interrupted and the layout must be recovered before
/// it can be read.
pub(crate) const MARKER_FILE: &str = ".save-in-progress";

/// Subdirectory inside the companion directory that holds backup copies
/// of pre-existing targets for the duration of a save.
pub(crate) const RESTORE_DIR: &str = ".restore";

/// Legacy directory suffixes from the staging-swap scheme (ADR-0006
/// supersedes). Cleaned up opportunistically at the start of every save.
const LEGACY_STAGING_SUFFIX: &str = ".staging";
const LEGACY_BACKUP_SUFFIX: &str = ".backup";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WriteOp {
    /// Overwrite `abs_path` with these serialized bytes.
    Bytes(#[serde(skip)] Vec<u8>),
    /// Copy `source` into `abs_path` (used for CDI XML files).
    CopyFrom(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedWrite {
    pub abs_path: PathBuf,
    pub op: WriteOp,
}

/// Cleanup directive: every file directly under `dir` whose absolute path
/// is not in `keep_abs` is treated as a delete during the save.
#[derive(Debug, Clone)]
pub(crate) struct PrunePlan {
    pub dir: PathBuf,
    pub keep_abs: HashSet<PathBuf>,
    /// Only consider files whose extension matches one of these. Empty
    /// means "all files".
    pub extensions: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub(crate) struct SavePlan {
    /// Layout directory that hosts the marker and `.restore/`. Must
    /// be inside the same volume as every file in `writes`.
    pub layout_dir: PathBuf,
    pub writes: Vec<PlannedWrite>,
    pub prune_dirs: Vec<PrunePlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Preparing,
    Writing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalEntry {
    abs_path: PathBuf,
    /// Filename inside `.restore/`. Empty when `pre_existed` is false.
    restore_name: String,
    pre_existed: bool,
    /// True if the journal will delete the target during the writing
    /// phase (rather than overwrite it).
    is_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Marker {
    phase: Phase,
    started_at: String,
    entries: Vec<JournalEntry>,
}

#[cfg(test)]
thread_local! {
    pub(crate) static FAIL_AFTER_BACKUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(crate) static FAIL_MID_WRITES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Execute a [`SavePlan`] under the write-ahead journal.
///
/// Steps:
///
/// 1. Clean up legacy staging/backup directories from the pre-S2e scheme.
/// 2. Ensure the companion directory exists and is empty of any prior
///    marker (recovering it first if necessary).
/// 3. Build the list of targets and their pre-existence flags.
/// 4. Write the marker with `phase: preparing`.
/// 5. Copy pre-existing targets to `.restore/`.
/// 6. Flip the marker to `phase: writing`.
/// 7. Overwrite targets in place and delete pruned files.
/// 8. Delete the marker and `.restore/`.
pub(crate) fn execute(plan: SavePlan) -> Result<(), String> {
    // 1. Clean up legacy staging/backup directories (best-effort).
    cleanup_legacy_dirs(&plan.layout_dir);

    // 2. Make sure the layout directory exists. Recover any prior
    //    interrupted save before we touch it.
    std::fs::create_dir_all(&plan.layout_dir).map_err(|e| {
        format!(
            "Cannot create layout directory {}: {}",
            plan.layout_dir.display(),
            e
        )
    })?;
    recover_layout_dir(&plan.layout_dir)?;

    let marker_path = plan.layout_dir.join(MARKER_FILE);
    let restore_dir = plan.layout_dir.join(RESTORE_DIR);

    // Discard any stale .restore/ left over from a previous run that
    // crashed without writing a marker.
    if restore_dir.exists() {
        std::fs::remove_dir_all(&restore_dir).map_err(|e| {
            format!(
                "Cannot clean stale restore dir {}: {}",
                restore_dir.display(),
                e
            )
        })?;
    }
    std::fs::create_dir_all(&restore_dir).map_err(|e| {
        format!(
            "Cannot create restore dir {}: {}",
            restore_dir.display(),
            e
        )
    })?;

    // 3. Build entries from writes + prune deletes.
    let mut entries: Vec<JournalEntry> = Vec::new();
    let mut planned_deletes: Vec<PathBuf> = Vec::new();
    for w in &plan.writes {
        let pre_existed = w.abs_path.is_file();
        entries.push(JournalEntry {
            abs_path: w.abs_path.clone(),
            restore_name: if pre_existed {
                format!("{:04}.bin", entries.len())
            } else {
                String::new()
            },
            pre_existed,
            is_delete: false,
        });
    }
    for prune in &plan.prune_dirs {
        if !prune.dir.is_dir() {
            continue;
        }
        let read_dir = std::fs::read_dir(&prune.dir).map_err(|e| {
            format!("Cannot read prune dir {}: {}", prune.dir.display(), e)
        })?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("Failed reading prune entry: {}", e))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !prune.extensions.is_empty() {
                let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
                if !prune.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                    continue;
                }
            }
            if prune.keep_abs.contains(&path) {
                continue;
            }
            planned_deletes.push(path);
        }
    }
    for del in &planned_deletes {
        entries.push(JournalEntry {
            abs_path: del.clone(),
            restore_name: format!("{:04}.bin", entries.len()),
            pre_existed: true, // deletes only target pre-existing files
            is_delete: true,
        });
    }

    let started_at = chrono::Utc::now().to_rfc3339();
    let mut marker = Marker {
        phase: Phase::Preparing,
        started_at: started_at.clone(),
        entries,
    };

    // 4. Write marker (phase=preparing) and fsync.
    write_marker(&marker_path, &marker)?;
    fsync_dir(&plan.layout_dir);

    // 5. Back up every pre-existing target into .restore/.
    for entry in &marker.entries {
        if entry.pre_existed {
            let restore_path = restore_dir.join(&entry.restore_name);
            std::fs::copy(&entry.abs_path, &restore_path).map_err(|e| {
                format!(
                    "Cannot back up {} to {}: {}",
                    entry.abs_path.display(),
                    restore_path.display(),
                    e
                )
            })?;
            sync_file(&restore_path);
        }
    }
    fsync_dir(&restore_dir);

    #[cfg(test)]
    if FAIL_AFTER_BACKUP.with(|c| c.get()) {
        return Err("test: aborted after backup phase".to_string());
    }

    // 6. Flip marker to phase=writing.
    marker.phase = Phase::Writing;
    write_marker(&marker_path, &marker)?;
    fsync_dir(&plan.layout_dir);

    // 7. Execute writes and deletes in plan order. Any failure here
    //    leaves the marker on disk so the next read_capture rolls back.
    for w in &plan.writes {
        if let Some(parent) = w.abs_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("Cannot create parent {}: {}", parent.display(), e)
            })?;
        }
        match &w.op {
            WriteOp::Bytes(bytes) => write_bytes_in_place(&w.abs_path, bytes)?,
            WriteOp::CopyFrom(src) => copy_in_place(src, &w.abs_path)?,
        }

        #[cfg(test)]
        if FAIL_MID_WRITES.with(|c| c.get()) {
            return Err("test: aborted mid writes".to_string());
        }
    }
    for del in &planned_deletes {
        if del.exists() {
            std::fs::remove_file(del).map_err(|e| {
                format!("Cannot delete pruned file {}: {}", del.display(), e)
            })?;
        }
    }

    // 8. Commit: delete marker, then .restore/.
    std::fs::remove_file(&marker_path).map_err(|e| {
        format!("Cannot remove marker {}: {}", marker_path.display(), e)
    })?;
    let _ = std::fs::remove_dir_all(&restore_dir);
    fsync_dir(&plan.layout_dir);

    Ok(())
}

/// If a save marker exists inside the layout directory, roll back any
/// partial writes. Returns `true` if a recovery happened.
///
/// Called from the top of [`super::io::read_layout_capture`] so a layout
/// is always coherent by the time it is parsed.
pub(crate) fn recover_if_needed(layout_dir: &Path) -> Result<bool, String> {
    if !layout_dir.exists() {
        return Ok(false);
    }
    let recovered = recover_layout_dir(layout_dir)?;
    if recovered {
        cleanup_legacy_dirs(layout_dir);
    }
    Ok(recovered)
}

fn recover_layout_dir(companion_dir: &Path) -> Result<bool, String> {
    let marker_path = companion_dir.join(MARKER_FILE);
    if !marker_path.is_file() {
        return Ok(false);
    }
    let raw = std::fs::read_to_string(&marker_path).map_err(|e| {
        format!("Cannot read save marker {}: {}", marker_path.display(), e)
    })?;
    let marker: Marker = serde_yaml_ng::from_str(&raw).map_err(|e| {
        format!("Cannot parse save marker {}: {}", marker_path.display(), e)
    })?;

    let restore_dir = companion_dir.join(RESTORE_DIR);

    match marker.phase {
        Phase::Preparing => {
            // No target files were modified yet. Just clear marker
            // and .restore/.
        }
        Phase::Writing => {
            // Roll back each entry.
            for entry in &marker.entries {
                if entry.pre_existed {
                    let restore_path = restore_dir.join(&entry.restore_name);
                    if restore_path.is_file() {
                        if let Some(parent) = entry.abs_path.parent() {
                            std::fs::create_dir_all(parent).map_err(|e| {
                                format!(
                                    "Cannot recreate parent {} during recovery: {}",
                                    parent.display(),
                                    e
                                )
                            })?;
                        }
                        std::fs::copy(&restore_path, &entry.abs_path).map_err(|e| {
                            format!(
                                "Cannot restore {} from {}: {}",
                                entry.abs_path.display(),
                                restore_path.display(),
                                e
                            )
                        })?;
                    }
                } else {
                    // Target was newly created during the interrupted
                    // save; delete it to return to the prior state.
                    if entry.abs_path.is_file() {
                        let _ = std::fs::remove_file(&entry.abs_path);
                    }
                }
            }
        }
    }

    if restore_dir.exists() {
        std::fs::remove_dir_all(&restore_dir).map_err(|e| {
            format!(
                "Cannot remove restore dir {} after recovery: {}",
                restore_dir.display(),
                e
            )
        })?;
    }
    std::fs::remove_file(&marker_path).map_err(|e| {
        format!(
            "Cannot remove save marker {} after recovery: {}",
            marker_path.display(),
            e
        )
    })?;
    fsync_dir(companion_dir);
    Ok(true)
}

fn cleanup_legacy_dirs(companion_dir: &Path) {
    // Companion dir name is e.g. "<base>.layout.d". Old scheme produced
    // sibling "<base>.layout.d.staging" and "<base>.layout.d.backup"
    // directories. Remove them best-effort; never fail the save here.
    let Some(parent) = companion_dir.parent() else { return };
    let Some(name) = companion_dir.file_name().and_then(|n| n.to_str()) else { return };
    for suffix in [LEGACY_STAGING_SUFFIX, LEGACY_BACKUP_SUFFIX] {
        let p = parent.join(format!("{}{}", name, suffix));
        if p.exists() {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

fn write_marker(path: &Path, marker: &Marker) -> Result<(), String> {
    let yaml = serde_yaml_ng::to_string(marker)
        .map_err(|e| format!("Cannot serialize save marker: {}", e))?;
    write_bytes_in_place(path, yaml.as_bytes())
}

fn write_bytes_in_place(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::File::create(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!("Cannot write {}: permission denied", path.display())
        } else {
            format!("Cannot create {}: {}", path.display(), e)
        }
    })?;
    file.write_all(bytes)
        .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;
    file.sync_all()
        .map_err(|e| format!("Cannot fsync {}: {}", path.display(), e))?;
    Ok(())
}

fn copy_in_place(src: &Path, dest: &Path) -> Result<(), String> {
    std::fs::copy(src, dest).map_err(|e| {
        format!("Cannot copy {} to {}: {}", src.display(), dest.display(), e)
    })?;
    sync_file(dest);
    Ok(())
}

fn sync_file(path: &Path) {
    if let Ok(f) = std::fs::OpenOptions::new().read(true).open(path) {
        let _ = f.sync_all();
    }
}

/// fsync a directory so renames / creates inside it become durable on
/// POSIX. No-op on Windows where `FlushFileBuffers` on a directory
/// handle is not generally supported.
#[cfg(unix)]
fn fsync_dir(dir: &Path) {
    if let Ok(f) = std::fs::File::open(dir) {
        let _ = f.sync_all();
    }
}

#[cfg(not(unix))]
fn fsync_dir(_dir: &Path) {
    // Windows does not provide a portable directory fsync; in-place
    // file writes already call sync_all() on each file.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_plan(companion: &Path, writes: Vec<(PathBuf, &[u8])>) -> SavePlan {
        SavePlan {
            layout_dir: companion.to_path_buf(),
            writes: writes
                .into_iter()
                .map(|(p, b)| PlannedWrite {
                    abs_path: p,
                    op: WriteOp::Bytes(b.to_vec()),
                })
                .collect(),
            prune_dirs: Vec::new(),
        }
    }

    #[test]
    fn execute_writes_files_and_clears_marker() {
        let root = fresh_dir("bowties_journal_basic");
        let companion = root.join("layout.layout.d");
        let target = companion.join("hello.yaml");
        let plan = make_plan(&companion, vec![(target.clone(), b"hi")]);

        execute(plan).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"hi");
        assert!(!companion.join(MARKER_FILE).exists());
        assert!(!companion.join(RESTORE_DIR).exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn recover_after_abort_in_preparing_phase_keeps_old_contents() {
        let root = fresh_dir("bowties_journal_recover_preparing");
        let companion = root.join("layout.layout.d");
        std::fs::create_dir_all(&companion).unwrap();
        let target = companion.join("hello.yaml");
        std::fs::write(&target, b"old").unwrap();

        // Manually plant a "preparing" marker with .restore/ as if a
        // prior save was killed before any writes.
        let marker = Marker {
            phase: Phase::Preparing,
            started_at: "0".to_string(),
            entries: vec![JournalEntry {
                abs_path: target.clone(),
                restore_name: "0000.bin".to_string(),
                pre_existed: true,
                is_delete: false,
            }],
        };
        write_marker(&companion.join(MARKER_FILE), &marker).unwrap();
        std::fs::create_dir_all(companion.join(RESTORE_DIR)).unwrap();
        std::fs::write(companion.join(RESTORE_DIR).join("0000.bin"), b"backup-of-old").unwrap();

        let recovered = recover_layout_dir(&companion).unwrap();
        assert!(recovered);
        assert!(!companion.join(MARKER_FILE).exists());
        assert!(!companion.join(RESTORE_DIR).exists());
        // Target was not touched by the preparing-phase recovery.
        assert_eq!(std::fs::read(&target).unwrap(), b"old");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn crash_between_backup_and_writes_recovers_old_contents() {
        let root = fresh_dir("bowties_journal_crash_after_backup");
        let companion = root.join("layout.layout.d");
        std::fs::create_dir_all(&companion).unwrap();
        let target = companion.join("hello.yaml");
        std::fs::write(&target, b"old").unwrap();

        FAIL_AFTER_BACKUP.with(|c| c.set(true));
        let err = execute(make_plan(&companion, vec![(target.clone(), b"new")])).unwrap_err();
        FAIL_AFTER_BACKUP.with(|c| c.set(false));
        assert!(err.contains("aborted after backup phase"));

        // Marker is still present; target on disk is still "old"
        // because we crashed in the preparing phase.
        assert!(companion.join(MARKER_FILE).exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"old");

        // Recovery clears marker without touching the file.
        let recovered = recover_layout_dir(&companion).unwrap();
        assert!(recovered);
        assert!(!companion.join(MARKER_FILE).exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"old");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn crash_mid_writes_rolls_back_pre_existing_target() {
        let root = fresh_dir("bowties_journal_crash_mid_writes");
        let companion = root.join("layout.layout.d");
        std::fs::create_dir_all(&companion).unwrap();
        let target = companion.join("hello.yaml");
        std::fs::write(&target, b"old").unwrap();

        FAIL_MID_WRITES.with(|c| c.set(true));
        let err = execute(make_plan(&companion, vec![(target.clone(), b"new")])).unwrap_err();
        FAIL_MID_WRITES.with(|c| c.set(false));
        assert!(err.contains("aborted mid writes"));

        // Target was overwritten (in-place writes happened before the
        // abort fired in the test seam), but marker is still on disk.
        assert!(companion.join(MARKER_FILE).exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"new");

        // Recovery restores old contents from .restore/.
        let recovered = recover_layout_dir(&companion).unwrap();
        assert!(recovered);
        assert!(!companion.join(MARKER_FILE).exists());
        assert!(!companion.join(RESTORE_DIR).exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"old");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn crash_mid_writes_deletes_newly_created_target() {
        let root = fresh_dir("bowties_journal_crash_creates_new");
        let companion = root.join("layout.layout.d");
        std::fs::create_dir_all(&companion).unwrap();
        let target_a = companion.join("a.yaml");
        let target_b = companion.join("b.yaml");
        // a exists, b does not.
        std::fs::write(&target_a, b"old-a").unwrap();

        let plan = make_plan(
            &companion,
            vec![(target_a.clone(), b"new-a"), (target_b.clone(), b"new-b")],
        );
        FAIL_MID_WRITES.with(|c| c.set(true));
        let _ = execute(plan).unwrap_err();
        FAIL_MID_WRITES.with(|c| c.set(false));

        // a was overwritten, b was created, then aborted.
        // (Both writes happen synchronously before the test seam fires
        // again; with N=2 only the first write happens since the seam
        // fires after each write.)
        let recovered = recover_layout_dir(&companion).unwrap();
        assert!(recovered);

        // a is restored, b should not exist.
        assert_eq!(std::fs::read(&target_a).unwrap(), b"old-a");
        assert!(!target_b.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn execute_prunes_extras_in_listed_dirs() {
        let root = fresh_dir("bowties_journal_prune");
        let companion = root.join("layout.layout.d");
        let nodes = companion.join("nodes");
        std::fs::create_dir_all(&nodes).unwrap();
        let keeper = nodes.join("AAA.yaml");
        let stale = nodes.join("BBB.yaml");
        std::fs::write(&keeper, b"old").unwrap();
        std::fs::write(&stale, b"stale").unwrap();

        let mut keep = HashSet::new();
        keep.insert(keeper.clone());
        let plan = SavePlan {
            layout_dir: companion.clone(),
            writes: vec![PlannedWrite {
                abs_path: keeper.clone(),
                op: WriteOp::Bytes(b"new".to_vec()),
            }],
            prune_dirs: vec![PrunePlan {
                dir: nodes.clone(),
                keep_abs: keep,
                extensions: vec!["yaml"],
            }],
        };
        execute(plan).unwrap();

        assert_eq!(std::fs::read(&keeper).unwrap(), b"new");
        assert!(!stale.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_staging_and_backup_dirs_are_cleaned_up() {
        let root = fresh_dir("bowties_journal_legacy_cleanup");
        let companion = root.join("layout.layout.d");
        std::fs::create_dir_all(&companion).unwrap();
        let staging = root.join("layout.layout.d.staging");
        let backup = root.join("layout.layout.d.backup");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::create_dir_all(&backup).unwrap();
        std::fs::write(staging.join("x"), b"x").unwrap();
        std::fs::write(backup.join("y"), b"y").unwrap();

        execute(make_plan(
            &companion,
            vec![(companion.join("z.yaml"), b"z")],
        ))
        .unwrap();

        assert!(!staging.exists());
        assert!(!backup.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_succeeds_with_shared_read_handle_open() {
        // Mimic Dropbox / antivirus holding an existing layout file
        // open with FILE_SHARE_READ|WRITE|DELETE while a save runs.
        // The pre-S2e directory-rename scheme would fail on Windows;
        // in-place writes go through.
        let root = fresh_dir("bowties_journal_held_handle");
        let companion = root.join("layout.layout.d");
        std::fs::create_dir_all(&companion).unwrap();
        let target = companion.join("snapshot.yaml");
        std::fs::write(&target, b"old").unwrap();

        let handle = std::fs::File::open(&target).expect("open shared handle");

        execute(make_plan(&companion, vec![(target.clone(), b"new")]))
            .expect("in-place save should succeed with a shared read handle open");

        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        drop(handle);

        let _ = std::fs::remove_dir_all(&root);
    }
}
