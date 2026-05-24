//! Layout file persistence for bowtie metadata.
//!
//! Manages loading and saving user-managed YAML layout files (single-file
//! `.bowties.yaml` documents and companion-directory layout captures), and
//! is the sole owner of the companion-directory file structure.
//!
//! Code outside `layout/` must use only the intent-shaped functions exposed
//! here (or the single-file helpers in [`io`]). It must not compute
//! companion-directory paths, node-snapshot filenames, or write YAML directly
//! into a layout directory — every mutation must flow through this module so
//! that the journaled-write protocol in [`journal`] (ADR-0006) covers
//! every persistence path.

use std::path::Path;

pub mod io;
pub(crate) mod journal;
pub mod known_layouts;
pub mod manifest;
pub mod node_snapshot;
pub mod offline_changes;
pub mod serde_node_id;
pub mod types;

use node_snapshot::NodeSnapshot;
use offline_changes::OfflineChange;

pub use io::{LayoutDirectoryReadData, LayoutDirectoryWriteData};

/// Save a complete layout: manifest + companion-directory contents
/// (bowtie metadata, node snapshots, offline changes, CDI files).
///
/// Persistence goes through the layout journal (ADR-0006): files are
/// written in place under a `.save-in-progress` marker so an
/// interrupted save can be rolled back on the next read, and so that
/// no `MoveFileEx` on a directory contends with Dropbox / OneDrive /
/// antivirus handles.
pub fn save_capture(base_file: &Path, data: &LayoutDirectoryWriteData) -> Result<(), String> {
    io::write_layout_capture(base_file, data)
}

/// Read a complete layout: manifest + companion-directory contents.
pub fn read_capture(base_file: &Path) -> Result<LayoutDirectoryReadData, String> {
    io::read_layout_capture(base_file)
}

/// Read a single node snapshot from the layout's companion directory.
///
/// `canonical_node_id` is the hex-string node id (with or without dots);
/// case is normalized to upper-case internally.
pub fn read_node_snapshot(
    base_file: &Path,
    canonical_node_id: &str,
) -> Result<NodeSnapshot, String> {
    let companion_dir = io::derive_companion_dir_path(base_file)?;
    let nodes_dir = companion_dir.join(io::NODES_DIR);
    let path = io::derive_node_file_path(&nodes_dir, canonical_node_id);
    io::read_yaml_file::<NodeSnapshot>(&path)
        .map_err(|e| format!("Cannot load snapshot {}: {}", path.display(), e))
}

/// Replace the offline-changes file in the layout's companion directory.
///
/// The companion directory must already exist (the layout must have been
/// saved at least once). Routed through the layout journal so a crash
/// mid-write is recoverable on the next read.
pub fn update_offline_changes(
    base_file: &Path,
    changes: &[OfflineChange],
) -> Result<(), String> {
    let companion_dir = io::derive_companion_dir_path(base_file)?;
    if !companion_dir.exists() {
        return Err(format!(
            "Layout companion directory not found: {}",
            companion_dir.display()
        ));
    }
    let path = companion_dir.join(io::OFFLINE_CHANGES_FILE);
    let bytes = io::serialize_yaml(&changes.to_vec())?;
    journal::execute(journal::SavePlan {
        companion_dir,
        writes: vec![journal::PlannedWrite {
            abs_path: path,
            op: journal::WriteOp::Bytes(bytes),
        }],
        prune_dirs: Vec::new(),
    })
}

/// Write the given node snapshots into the layout's companion directory.
///
/// Snapshots for nodes not in the list are left untouched. Creates the
/// `nodes/` subdirectory if it does not already exist. The companion
/// directory itself must already exist. Routed through the layout
/// journal so a crash mid-write is recoverable.
pub fn update_node_snapshots(
    base_file: &Path,
    snapshots: &[NodeSnapshot],
) -> Result<(), String> {
    let companion_dir = io::derive_companion_dir_path(base_file)?;
    if !companion_dir.exists() {
        return Err(format!(
            "Layout companion directory not found: {}",
            companion_dir.display()
        ));
    }
    let nodes_dir = companion_dir.join(io::NODES_DIR);
    std::fs::create_dir_all(&nodes_dir)
        .map_err(|e| format!("Cannot create nodes dir {}: {}", nodes_dir.display(), e))?;
    let mut writes = Vec::with_capacity(snapshots.len());
    for snapshot in snapshots {
        let canonical = snapshot.node_id.to_canonical();
        let path = io::derive_node_file_path(&nodes_dir, &canonical);
        let bytes = io::serialize_yaml(snapshot)?;
        writes.push(journal::PlannedWrite {
            abs_path: path,
            op: journal::WriteOp::Bytes(bytes),
        });
    }
    journal::execute(journal::SavePlan {
        companion_dir,
        writes,
        prune_dirs: Vec::new(),
    })
}

/// Resolve CDI XML for a node snapshot, checking the global app-data cache
/// first, then falling back to the layout's companion directory.
pub fn resolve_cdi_xml_for_snapshot(
    base_file: &Path,
    snapshot: &NodeSnapshot,
    app_data_dir: &Path,
) -> Result<String, String> {
    let companion_dir = io::derive_companion_dir_path(base_file)?;
    io::resolve_cdi_xml(snapshot, app_data_dir, &companion_dir)
}

/// Read just the layout manifest file (the base `.layout` YAML),
/// without loading companion-directory contents. Useful when callers
/// only need a small piece of manifest data such as the saved
/// connections list (Spec 013 / S4).
pub fn read_manifest(base_file: &Path) -> Result<manifest::LayoutManifest, String> {
    let m: manifest::LayoutManifest = io::read_yaml_file(base_file)?;
    m.validate()?;
    Ok(m)
}

/// Replace the saved connections list on a layout's manifest.
///
/// Loads the existing manifest, swaps in the new connections list,
/// and writes the manifest back through the layout journal so a
/// crash mid-write is recoverable (ADR-0006). The companion directory
/// must already exist (i.e. the layout must have been saved at least
/// once).
pub fn update_manifest_connections(
    base_file: &Path,
    connections: Vec<types::ConnectionConfig>,
) -> Result<(), String> {
    let mut manifest: manifest::LayoutManifest = io::read_yaml_file(base_file)?;
    manifest.validate()?;
    manifest.connections = connections;

    let companion_dir = io::derive_companion_dir_path(base_file)?;
    if !companion_dir.exists() {
        return Err(format!(
            "Layout companion directory not found: {}",
            companion_dir.display()
        ));
    }
    let bytes = io::serialize_yaml(&manifest)?;
    journal::execute(journal::SavePlan {
        companion_dir,
        writes: vec![journal::PlannedWrite {
            abs_path: base_file.to_path_buf(),
            op: journal::WriteOp::Bytes(bytes),
        }],
        prune_dirs: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::manifest::LayoutManifest;
    use crate::layout::node_snapshot::{
        CaptureStatus, CdiReference, NodeSnapshot, SnapshotLeafValue, SnipSnapshot,
    };
    use crate::layout::offline_changes::{
        OfflineChange, OfflineChangeKind, OfflineChangeStatus,
    };
    use crate::layout::types::LayoutFile;
    use lcc_rs::NodeID;
    use std::collections::BTreeMap;

    fn fresh_dir(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_snapshot(node_hex: &str) -> NodeSnapshot {
        let mut snap = NodeSnapshot {
            node_id: NodeID::from_hex_string(node_hex).unwrap(),
            captured_at: "2026-05-23T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot {
                user_name: "n".to_string(),
                user_description: String::new(),
                manufacturer_name: "Acme".to_string(),
                model_name: "ModelX".to_string(),
            },
            cdi_ref: CdiReference {
                cache_key: "acme_modelx_1.0".to_string(),
                version: "1.0".to_string(),
                fingerprint: "len:123".to_string(),
            },
            config: BTreeMap::new(),
            producer_identified_events: Vec::new(),
        };
        snap.add_config_leaf(
            &["seg:0".to_string(), "elem:1".to_string()],
            SnapshotLeafValue {
                value: "1".to_string(),
                space: Some(253),
                offset: Some("0x00000010".to_string()),
            },
        );
        snap
    }

    fn make_change(change_id: &str) -> OfflineChange {
        OfflineChange {
            change_id: change_id.to_string(),
            kind: OfflineChangeKind::Config,
            node_id: Some(NodeID::from_hex_string("050101011402").unwrap()),
            space: Some(253),
            offset: Some("0x00000010".to_string()),
            baseline_value: "0".to_string(),
            planned_value: "1".to_string(),
            status: OfflineChangeStatus::Pending,
            error: None,
            updated_at: "2026-05-23T00:00:00Z".to_string(),
        }
    }

    fn seed_layout(base_file: &Path, snapshots: Vec<NodeSnapshot>) {
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
            "layout.layout.d".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: snapshots,
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
        };
        save_capture(base_file, &data).unwrap();
    }

    #[test]
    fn read_node_snapshot_returns_value_written_by_save_capture() {
        let root = fresh_dir("bowties_s2d_read_after_save");
        let base = root.join("layout.layout");
        let snap = make_snapshot("050101011402");
        seed_layout(&base, vec![snap.clone()]);

        let loaded = read_node_snapshot(&base, "050101011402").unwrap();
        assert_eq!(loaded.node_id, snap.node_id);
        assert_eq!(loaded.cdi_ref.cache_key, "acme_modelx_1.0");
        assert!(!loaded.config.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_offline_changes_roundtrips_through_read_capture() {
        let root = fresh_dir("bowties_s2d_offline_changes_roundtrip");
        let base = root.join("layout.layout");
        seed_layout(&base, vec![make_snapshot("050101011402")]);

        let changes = vec![make_change("c1"), make_change("c2")];
        update_offline_changes(&base, &changes).unwrap();

        let loaded = read_capture(&base).unwrap();
        assert_eq!(loaded.offline_changes.len(), 2);
        assert_eq!(loaded.offline_changes[0].change_id, "c1");
        assert_eq!(loaded.offline_changes[1].change_id, "c2");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_node_snapshots_preserves_other_nodes() {
        let root = fresh_dir("bowties_s2d_node_snapshots_partial");
        let base = root.join("layout.layout");
        let snap_a = make_snapshot("050101011402");
        seed_layout(&base, vec![snap_a.clone()]);

        // Add a second snapshot via partial update.
        let snap_b = make_snapshot("050101011403");
        update_node_snapshots(&base, &[snap_b.clone()]).unwrap();

        // Both snapshots should be readable.
        let read_a = read_node_snapshot(&base, "050101011402").unwrap();
        assert_eq!(read_a.node_id, snap_a.node_id);
        let read_b = read_node_snapshot(&base, "050101011403").unwrap();
        assert_eq!(read_b.node_id, snap_b.node_id);

        let loaded = read_capture(&base).unwrap();
        assert_eq!(loaded.node_snapshots.len(), 2);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_node_snapshot_returns_value_written_by_partial_update() {
        let root = fresh_dir("bowties_s2d_read_after_partial_update");
        let base = root.join("layout.layout");
        seed_layout(&base, vec![make_snapshot("050101011402")]);

        let mut updated = make_snapshot("050101011402");
        updated.captured_at = "2026-06-01T12:00:00Z".to_string();
        update_node_snapshots(&base, &[updated]).unwrap();

        let loaded = read_node_snapshot(&base, "050101011402").unwrap();
        assert_eq!(loaded.captured_at, "2026-06-01T12:00:00Z");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_apis_error_when_companion_dir_missing() {
        let root = fresh_dir("bowties_s2d_missing_companion");
        let base = root.join("nope.layout");
        // No seed_layout — companion dir doesn't exist.
        let err = update_offline_changes(&base, &[]).unwrap_err();
        assert!(err.contains("not found"), "got: {}", err);
        let err = update_node_snapshots(&base, &[]).unwrap_err();
        assert!(err.contains("not found"), "got: {}", err);

        let _ = std::fs::remove_dir_all(&root);
    }
}
