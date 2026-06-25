//! Layout file persistence for bowtie metadata.
//!
//! Manages loading and saving user-managed YAML layout files (single-file
//! `.bowties.yaml` documents and folder-based layout captures), and
//! is the sole owner of the layout directory file structure.
//!
//! Code outside `layout/` must use only the intent-shaped functions exposed
//! here (or the single-file helpers in [`io`]). It must not compute
//! layout-directory paths, node-snapshot filenames, or write YAML directly
//! into a layout directory — every mutation must flow through this module so
//! that the journaled-write protocol in [`journal`] (ADR-0006) covers
//! every persistence path.

use std::path::Path;

pub mod capture;
pub mod channels;
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

/// Save a complete layout: manifest + directory contents
/// (bowtie metadata, node snapshots, offline changes, CDI files).
///
/// Persistence goes through the layout journal (ADR-0006): files are
/// written in place under a `.save-in-progress` marker so an
/// interrupted save can be rolled back on the next read, and so that
/// no `MoveFileEx` on a directory contends with Dropbox / OneDrive /
/// antivirus handles.
pub fn save_capture(layout_dir: &Path, data: &LayoutDirectoryWriteData) -> Result<(), String> {
    io::write_layout_capture(layout_dir, data)
}

/// Read a complete layout: manifest + directory contents.
pub fn read_capture(layout_dir: &Path) -> Result<LayoutDirectoryReadData, String> {
    io::read_layout_capture(layout_dir)
}

/// Read a single node snapshot from the layout directory.
///
/// `canonical_node_id` is the hex-string node id (with or without dots);
/// case is normalized to upper-case internally.
pub fn read_node_snapshot(
    layout_dir: &Path,
    canonical_node_id: &str,
) -> Result<NodeSnapshot, String> {
    let nodes_dir = layout_dir.join(io::NODES_DIR);
    let path = io::derive_node_file_path(&nodes_dir, canonical_node_id);
    io::read_yaml_file::<NodeSnapshot>(&path)
        .map_err(|e| format!("Cannot load snapshot {}: {}", path.display(), e))
}

/// Replace the offline-changes file in the layout directory.
///
/// The layout directory must already exist (the layout must have been
/// saved at least once). Routed through the layout journal so a crash
/// mid-write is recoverable on the next read.
pub fn update_offline_changes(
    layout_dir: &Path,
    changes: &[OfflineChange],
) -> Result<(), String> {
    if !layout_dir.exists() {
        return Err(format!(
            "Layout directory not found: {}",
            layout_dir.display()
        ));
    }
    let path = layout_dir.join(io::OFFLINE_CHANGES_FILE);
    let bytes = io::serialize_yaml(&changes.to_vec())?;
    journal::execute(journal::SavePlan {
        layout_dir: layout_dir.to_path_buf(),
        writes: vec![journal::PlannedWrite {
            abs_path: path,
            op: journal::WriteOp::Bytes(bytes),
        }],
        prune_dirs: Vec::new(),
    })
}

/// Replace the channels file in the layout directory.
///
/// The layout directory must already exist. Routed through the layout
/// journal so a crash mid-write is recoverable (ADR-0006).
pub fn update_channels(
    layout_dir: &Path,
    doc: &channels::ChannelsDocument,
) -> Result<(), String> {
    if !layout_dir.exists() {
        return Err(format!(
            "Layout directory not found: {}",
            layout_dir.display()
        ));
    }
    let path = layout_dir.join(io::CHANNELS_FILE);
    let bytes = io::serialize_yaml(doc)?;
    journal::execute(journal::SavePlan {
        layout_dir: layout_dir.to_path_buf(),
        writes: vec![journal::PlannedWrite {
            abs_path: path,
            op: journal::WriteOp::Bytes(bytes),
        }],
        prune_dirs: Vec::new(),
    })
}

/// Read the channels document from the layout directory.
///
/// Returns an empty `ChannelsDocument` when the file does not exist
/// (pre-015 layouts), matching the backward-compatibility rule.
pub fn read_channels(layout_dir: &Path) -> Result<channels::ChannelsDocument, String> {
    let path = layout_dir.join(io::CHANNELS_FILE);
    if !path.exists() {
        return Ok(channels::ChannelsDocument::default());
    }
    io::read_yaml_file(&path)
}

/// Write the given node snapshots into the layout directory.
///
/// Snapshots for nodes not in the list are left untouched. Creates the
/// `nodes/` subdirectory if it does not already exist. The layout
/// directory itself must already exist. Routed through the layout
/// journal so a crash mid-write is recoverable.
pub fn update_node_snapshots(
    layout_dir: &Path,
    snapshots: &[NodeSnapshot],
) -> Result<(), String> {
    if !layout_dir.exists() {
        return Err(format!(
            "Layout directory not found: {}",
            layout_dir.display()
        ));
    }
    let nodes_dir = layout_dir.join(io::NODES_DIR);
    std::fs::create_dir_all(&nodes_dir)
        .map_err(|e| format!("Cannot create nodes dir {}: {}", nodes_dir.display(), e))?;
    let mut writes = Vec::with_capacity(snapshots.len());
    for snapshot in snapshots {
        let canonical = snapshot.filename_basis();
        let path = io::derive_node_file_path(&nodes_dir, &canonical);
        let bytes = io::serialize_yaml(snapshot)?;
        writes.push(journal::PlannedWrite {
            abs_path: path,
            op: journal::WriteOp::Bytes(bytes),
        });
    }
    journal::execute(journal::SavePlan {
        layout_dir: layout_dir.to_path_buf(),
        writes,
        prune_dirs: Vec::new(),
    })
}

/// Resolve CDI XML for a node snapshot, checking the global app-data cache
/// first, then falling back to the layout directory.
pub fn resolve_cdi_xml_for_snapshot(
    layout_dir: &Path,
    snapshot: &NodeSnapshot,
    app_data_dir: &Path,
) -> Result<String, String> {
    io::resolve_cdi_xml(snapshot, app_data_dir, layout_dir)
}

/// Read just the layout manifest file (the `manifest.yaml` inside the
/// layout directory), without loading the rest of the contents. Useful
/// when callers only need a small piece of manifest data such as the
/// saved connections list (Spec 013 / S4).
pub fn read_manifest(layout_dir: &Path) -> Result<manifest::LayoutManifest, String> {
    let manifest_path = layout_dir.join(io::MANIFEST_FILE);
    let m: manifest::LayoutManifest = io::read_yaml_file(&manifest_path)?;
    m.validate()?;
    Ok(m)
}

/// Replace the saved connections list on a layout's manifest.
///
/// Loads the existing manifest, swaps in the new connections list,
/// and writes the manifest back through the layout journal so a
/// crash mid-write is recoverable (ADR-0006). The layout directory
/// must already exist (i.e. the layout must have been saved at least
/// once).
pub fn update_manifest_connections(
    layout_dir: &Path,
    connections: Vec<types::ConnectionConfig>,
) -> Result<(), String> {
    let manifest_path = layout_dir.join(io::MANIFEST_FILE);
    let mut manifest: manifest::LayoutManifest = io::read_yaml_file(&manifest_path)?;
    manifest.validate()?;
    manifest.connections = connections;

    if !layout_dir.exists() {
        return Err(format!(
            "Layout directory not found: {}",
            layout_dir.display()
        ));
    }
    let bytes = io::serialize_yaml(&manifest)?;
    journal::execute(journal::SavePlan {
        layout_dir: layout_dir.to_path_buf(),
        writes: vec![journal::PlannedWrite {
            abs_path: manifest_path,
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
        let nid = NodeID::from_hex_string(node_hex).unwrap();
        let mut snap = NodeSnapshot {
            node_key: nid.to_canonical(),
            node_id: Some(nid),
            profile_stem: None,
            lifecycle: crate::layout::node_snapshot::NodeSnapshotLifecycle::Persisted,
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
            node_key: Some("050101011402".to_string()),
            space: Some(253),
            offset: Some("0x00000010".to_string()),
            baseline_value: "0".to_string(),
            planned_value: "1".to_string(),
            status: OfflineChangeStatus::Pending,
            error: None,
            updated_at: "2026-05-23T00:00:00Z".to_string(),
        }
    }

    fn seed_layout(layout_dir: &Path, snapshots: Vec<NodeSnapshot>) {
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: snapshots,
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };
        save_capture(layout_dir, &data).unwrap();
    }

    #[test]
    fn read_node_snapshot_returns_value_written_by_save_capture() {
        let root = fresh_dir("bowties_s2d_read_after_save");
        let layout_dir = root.join("my-layout");
        let snap = make_snapshot("050101011402");
        seed_layout(&layout_dir, vec![snap.clone()]);

        let loaded = read_node_snapshot(&layout_dir, "050101011402").unwrap();
        assert_eq!(loaded.node_id, snap.node_id);
        assert_eq!(loaded.cdi_ref.cache_key, "acme_modelx_1.0");
        assert!(!loaded.config.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_offline_changes_roundtrips_through_read_capture() {
        let root = fresh_dir("bowties_s2d_offline_changes_roundtrip");
        let layout_dir = root.join("my-layout");
        seed_layout(&layout_dir, vec![make_snapshot("050101011402")]);

        let changes = vec![make_change("c1"), make_change("c2")];
        update_offline_changes(&layout_dir, &changes).unwrap();

        let loaded = read_capture(&layout_dir).unwrap();
        assert_eq!(loaded.offline_changes.len(), 2);
        assert_eq!(loaded.offline_changes[0].change_id, "c1");
        assert_eq!(loaded.offline_changes[1].change_id, "c2");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_node_snapshots_preserves_other_nodes() {
        let root = fresh_dir("bowties_s2d_node_snapshots_partial");
        let layout_dir = root.join("my-layout");
        let snap_a = make_snapshot("050101011402");
        seed_layout(&layout_dir, vec![snap_a.clone()]);

        // Add a second snapshot via partial update.
        let snap_b = make_snapshot("050101011403");
        update_node_snapshots(&layout_dir, &[snap_b.clone()]).unwrap();

        // Both snapshots should be readable.
        let read_a = read_node_snapshot(&layout_dir, "050101011402").unwrap();
        assert_eq!(read_a.node_id, snap_a.node_id);
        let read_b = read_node_snapshot(&layout_dir, "050101011403").unwrap();
        assert_eq!(read_b.node_id, snap_b.node_id);

        let loaded = read_capture(&layout_dir).unwrap();
        assert_eq!(loaded.node_snapshots.len(), 2);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_node_snapshot_returns_value_written_by_partial_update() {
        let root = fresh_dir("bowties_s2d_read_after_partial_update");
        let layout_dir = root.join("my-layout");
        seed_layout(&layout_dir, vec![make_snapshot("050101011402")]);

        let mut updated = make_snapshot("050101011402");
        updated.captured_at = "2026-06-01T12:00:00Z".to_string();
        update_node_snapshots(&layout_dir, &[updated]).unwrap();

        let loaded = read_node_snapshot(&layout_dir, "050101011402").unwrap();
        assert_eq!(loaded.captured_at, "2026-06-01T12:00:00Z");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_apis_error_when_layout_dir_missing() {
        let root = fresh_dir("bowties_s2d_missing_layout_dir");
        let layout_dir = root.join("nope");
        // No seed_layout — directory doesn't exist.
        let err = update_offline_changes(&layout_dir, &[]).unwrap_err();
        assert!(err.contains("not found"), "got: {}", err);
        let err = update_node_snapshots(&layout_dir, &[]).unwrap_err();
        assert!(err.contains("not found"), "got: {}", err);

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── S2: Channel persistence ───────────────────────────────────────────

    fn make_channel(id: &str, name: &str, input: u32) -> crate::layout::channels::InformationChannel {
        crate::layout::channels::InformationChannel {
            id: id.to_string(),
            name: name.to_string(),
            channel_type: crate::layout::channels::ChannelType::BlockOccupancy,
            hardware_ref: crate::layout::channels::HardwareReference {
                node_key: "05010101FF000001".to_string(),
                connector: "connector-a".to_string(),
                input,
            },
        }
    }

    #[test]
    fn channels_roundtrip_through_save_and_read_capture() {
        let root = fresh_dir("bowties_s2_channels_roundtrip");
        let layout_dir = root.join("my-layout");

        let channels = vec![
            make_channel("ch-001", "Block 1", 1),
            make_channel("ch-002", "Block 2", 2),
        ];
        let doc = crate::layout::channels::ChannelsDocument::new(channels.clone());

        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-06-24T00:00:00Z".to_string(),
            "2026-06-24T00:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: Vec::new(),
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: doc.clone(),
        };
        save_capture(&layout_dir, &data).unwrap();

        let loaded = read_capture(&layout_dir).unwrap();
        assert_eq!(loaded.channels.channels.len(), 2);
        assert_eq!(loaded.channels.channels[0].id, "ch-001");
        assert_eq!(loaded.channels.channels[1].name, "Block 2");
        assert_eq!(loaded.channels.schema_version, "1.0");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_capture_returns_empty_channels_when_file_missing() {
        let root = fresh_dir("bowties_s2_channels_missing_file");
        let layout_dir = root.join("my-layout");

        // Seed a layout without channels (simulates pre-015 layout)
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-06-24T00:00:00Z".to_string(),
            "2026-06-24T00:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: Vec::new(),
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };
        save_capture(&layout_dir, &data).unwrap();

        // Remove channels.yaml to simulate pre-015 layout
        let channels_path = layout_dir.join("channels.yaml");
        let _ = std::fs::remove_file(&channels_path);

        let loaded = read_capture(&layout_dir).unwrap();
        assert!(loaded.channels.channels.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_channels_roundtrips_through_read_capture() {
        let root = fresh_dir("bowties_s2_update_channels");
        let layout_dir = root.join("my-layout");
        seed_layout(&layout_dir, vec![]);

        let doc = crate::layout::channels::ChannelsDocument::new(vec![
            make_channel("ch-100", "Yard Lead", 1),
        ]);
        update_channels(&layout_dir, &doc).unwrap();

        let loaded = read_capture(&layout_dir).unwrap();
        assert_eq!(loaded.channels.channels.len(), 1);
        assert_eq!(loaded.channels.channels[0].name, "Yard Lead");

        let _ = std::fs::remove_dir_all(&root);
    }
}
