//! Layout file I/O operations.
//!
//! Holds path / filename knowledge for the companion-directory layout
//! (`bowties.yaml`, `nodes/`, `offline-changes.yaml`, etc.) and the
//! schema-validated read/write routines that sit underneath the
//! intent-shaped public API in [`super`].
//!
//! Persistence into the companion directory goes through
//! [`super::journal`] (ADR-0006). This module does not perform any
//! `MoveFileEx` on a directory or per-file `rename` over the wire; it
//! serializes YAML, builds a [`super::journal::SavePlan`], and lets the
//! journal carry out the writes in place under a `.save-in-progress`
//! marker.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use super::types::LayoutFile;
use super::manifest::LayoutManifest;
use super::node_snapshot::NodeSnapshot;
use super::offline_changes::OfflineChange;
use super::channels::ChannelsDocument;
use super::journal::{self, PlannedWrite, PrunePlan, SavePlan, WriteOp};

pub const BOWTIES_FILE: &str = "bowties.yaml";
pub const OFFLINE_CHANGES_FILE: &str = "offline-changes.yaml";
pub const EVENT_NAMES_FILE: &str = "event-names.yaml";
pub const CHANNELS_FILE: &str = "channels.yaml";
pub const NODES_DIR: &str = "nodes";
pub const MANIFEST_FILE: &str = "manifest.yaml";
const CDI_DIR: &str = "cdi";

/// Load a layout file from the given path.
///
/// Validates the schema after parsing. If the YAML is malformed but parseable,
/// returns a degraded layout with valid parts (FR-026).
pub fn load_file(path: &Path) -> Result<LayoutFile, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("Layout file not found: {}", path.display())
            } else {
                format!("Failed to read layout file: {}", e)
            }
        })?;

    let layout: LayoutFile = match serde_yaml_ng::from_str(&contents) {
        Ok(l) => l,
        Err(e) => {
            // FR-026: degraded mode — try to return a default layout with an error logged
            eprintln!(
                "[layout][WARN] Failed to parse layout file '{}': {}. Using empty layout.",
                path.display(), e
            );
            return Err(format!("Failed to parse layout file: {}", e));
        }
    };

    validate_schema(&layout)?;

    Ok(layout)
}

/// Validate the schema of a loaded layout file.
pub fn validate_schema(layout: &LayoutFile) -> Result<(), String> {
    layout.validate()
}

/// Save a layout file to the given path using atomic write (temp → flush → rename).
///
/// Creates parent directories if needed. Overwrites existing file at path.
pub fn save_file(path: &Path, layout: &LayoutFile) -> Result<(), String> {
    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create directory {}: {}", parent.display(), e))?;
    }

    let yaml = serde_yaml_ng::to_string(layout)
        .map_err(|e| format!("Failed to serialize layout: {}", e))?;

    // Atomic write: write to temp file, flush, then rename
    let temp_path = path.with_extension("yaml.tmp");
    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!("Cannot write to {}: permission denied", path.display())
            } else {
                format!("Failed to create temp file: {}", e)
            }
        })?;

    file.write_all(yaml.as_bytes())
        .map_err(|e| format!("Failed to write layout data: {}", e))?;

    file.flush()
        .map_err(|e| format!("Failed to flush layout data: {}", e))?;

    // Explicitly drop file handle before rename (required on Windows)
    drop(file);

    std::fs::rename(&temp_path, path)
        .map_err(|e| format!("Failed to save layout file: {}", e))?;

    Ok(())
}

/// Serialize a value to YAML bytes for inclusion in a journal
/// [`SavePlan`].
pub fn serialize_yaml<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_yaml_ng::to_string(value)
        .map(|s| s.into_bytes())
        .map_err(|e| format!("Failed to serialize YAML: {}", e))
}

/// Read and deserialize a YAML file.
pub fn read_yaml_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read YAML file {}: {}", path.display(), e))?;
    serde_yaml_ng::from_str::<T>(&contents)
        .map_err(|e| format!("Failed to parse YAML file {}: {}", path.display(), e))
}

pub fn derive_node_file_path(nodes_dir: &Path, node_id: &str) -> PathBuf {
    nodes_dir.join(format!("{}.yaml", node_id.to_uppercase()))
}

/// Derive the manifest file path inside a layout directory.
pub fn derive_manifest_path(layout_dir: &Path) -> PathBuf {
    layout_dir.join(MANIFEST_FILE)
}

#[derive(Debug, Clone)]
pub struct LayoutDirectoryWriteData {
    pub manifest: LayoutManifest,
    pub node_snapshots: Vec<NodeSnapshot>,
    pub bowties: LayoutFile,
    pub offline_changes: Vec<OfflineChange>,
    /// List of (cache_key, source_path_to_cdi_file) pairs for CDI files to copy
    pub cdi_files: Vec<(String, std::path::PathBuf)>,
    /// Information channel inventory for the layout.
    pub channels: ChannelsDocument,
}

#[derive(Debug, Clone)]
pub struct LayoutDirectoryReadData {
    pub manifest: LayoutManifest,
    pub node_snapshots: Vec<NodeSnapshot>,
    pub bowties: LayoutFile,
    pub offline_changes: Vec<OfflineChange>,
    /// True when [`read_layout_capture`] rolled back an interrupted
    /// prior save (ADR-0006) before parsing this layout. The frontend
    /// should surface a notice that the previous save was incomplete
    /// and has been restored.
    pub recovery_occurred: bool,
    /// Information channel inventory loaded from `channels.yaml`.
    /// Empty when the file is missing (pre-015 layouts).
    pub channels: ChannelsDocument,
}

pub fn write_layout_capture(layout_dir: &Path, data: &LayoutDirectoryWriteData) -> Result<(), String> {
    std::fs::create_dir_all(layout_dir)
        .map_err(|e| format!("Cannot create layout directory {}: {}", layout_dir.display(), e))?;

    let manifest = data.manifest.clone();

    // Build the journaled save plan. Every file the save will touch
    // goes through a single `.save-in-progress` marker (ADR-0006) so
    // an interrupted save can be rolled back on the next read.
    let mut plan = SavePlan {
        layout_dir: layout_dir.to_path_buf(),
        writes: Vec::new(),
        prune_dirs: Vec::new(),
    };

    // Manifest file (inside the layout directory).
    plan.writes.push(PlannedWrite {
        abs_path: layout_dir.join(MANIFEST_FILE),
        op: WriteOp::Bytes(serialize_yaml(&manifest)?),
    });

    // Top-level layout files.
    plan.writes.push(PlannedWrite {
        abs_path: layout_dir.join(BOWTIES_FILE),
        op: WriteOp::Bytes(serialize_yaml(&data.bowties)?),
    });
    plan.writes.push(PlannedWrite {
        abs_path: layout_dir.join(OFFLINE_CHANGES_FILE),
        op: WriteOp::Bytes(serialize_yaml(&data.offline_changes)?),
    });
    plan.writes.push(PlannedWrite {
        abs_path: layout_dir.join(EVENT_NAMES_FILE),
        op: WriteOp::Bytes(serialize_yaml(&std::collections::BTreeMap::<String, String>::new())?),
    });
    plan.writes.push(PlannedWrite {
        abs_path: layout_dir.join(CHANNELS_FILE),
        op: WriteOp::Bytes(serialize_yaml(&data.channels)?),
    });

    // Per-node snapshots: also prune extras left over from a previous
    // save (e.g. snapshots for nodes that have since been removed).
    let nodes_dir = layout_dir.join(NODES_DIR);
    let mut keep_nodes: HashSet<PathBuf> = HashSet::with_capacity(data.node_snapshots.len());
    for snapshot in &data.node_snapshots {
        let node_path = derive_node_file_path(&nodes_dir, &snapshot.filename_basis());
        keep_nodes.insert(node_path.clone());
        let bytes = serialize_yaml(snapshot)?;
        // Skip writing if the file already exists with identical content
        // (ADR-0006: "Files that did not change are not rewritten").
        if node_path.is_file() {
            if let Ok(existing) = std::fs::read(&node_path) {
                if existing == bytes {
                    continue;
                }
            }
        }
        plan.writes.push(PlannedWrite {
            abs_path: node_path,
            op: WriteOp::Bytes(bytes),
        });
    }
    plan.prune_dirs.push(PrunePlan {
        dir: nodes_dir,
        keep_abs: keep_nodes,
        extensions: vec!["yaml"],
    });

    // CDI files. When new CDI files are provided (fresh save from the
    // live bus) we plan a copy for each and prune any extras. When no
    // new CDI files are provided we leave the existing `cdi/`
    // directory alone — in-place writes preserve what is already
    // there, so the staging-dir "copy across" step is no longer
    // needed.
    let cdi_dir = layout_dir.join(CDI_DIR);
    if !data.cdi_files.is_empty() {
        let mut keep_cdi: HashSet<PathBuf> = HashSet::with_capacity(data.cdi_files.len());
        for (cache_key, source_path) in &data.cdi_files {
            let dest = cdi_dir.join(format!("{}.cdi.xml", cache_key));
            keep_cdi.insert(dest.clone());
            plan.writes.push(PlannedWrite {
                abs_path: dest,
                op: WriteOp::CopyFrom(source_path.clone()),
            });
        }
        plan.prune_dirs.push(PrunePlan {
            dir: cdi_dir,
            keep_abs: keep_cdi,
            extensions: vec!["xml"],
        });
    }

    journal::execute(plan)?;

    if !layout_dir.exists() {
        return Err(format!(
            "Layout save failed: layout directory missing after save: {}",
            layout_dir.display()
        ));
    }
    Ok(())
}

pub fn read_layout_capture(layout_dir: &Path) -> Result<LayoutDirectoryReadData, String> {
    if !layout_dir.exists() {
        return Err(format!(
            "Layout directory not found: {}",
            layout_dir.display()
        ));
    }

    // ADR-0006: roll back any interrupted prior save before parsing.
    let recovery_occurred = journal::recover_if_needed(layout_dir)?;

    let manifest_path = layout_dir.join(MANIFEST_FILE);
    let manifest: LayoutManifest = read_yaml_file(&manifest_path)?;
    manifest.validate()?;

    let (bowties, node_snapshots, offline_changes) = read_companion_contents(layout_dir, &manifest)?;

    // channels.yaml is optional — pre-015 layouts won't have it.
    let channels_path = layout_dir.join(CHANNELS_FILE);
    let channels: ChannelsDocument = if channels_path.exists() {
        read_yaml_file(&channels_path)?
    } else {
        ChannelsDocument::default()
    };

    Ok(LayoutDirectoryReadData {
        manifest,
        node_snapshots,
        bowties,
        offline_changes,
        recovery_occurred,
        channels,
    })
}

fn read_companion_contents(
    root_dir: &Path,
    _manifest: &LayoutManifest,
) -> Result<(LayoutFile, Vec<NodeSnapshot>, Vec<OfflineChange>), String> {
    let bowties_path = root_dir.join(BOWTIES_FILE);
    let bowties: LayoutFile = if bowties_path.exists() {
        read_yaml_file(&bowties_path)?
    } else {
        LayoutFile::default()
    };

    let offline_changes_path = root_dir.join(OFFLINE_CHANGES_FILE);
    let offline_changes: Vec<OfflineChange> = if offline_changes_path.exists() {
        read_yaml_file(&offline_changes_path)?
    } else {
        Vec::new()
    };

    let nodes_dir = root_dir.join(NODES_DIR);
    let mut node_snapshots = Vec::new();
    if nodes_dir.exists() {
        let entries = std::fs::read_dir(&nodes_dir)
            .map_err(|e| format!("Cannot read nodes directory {}: {}", nodes_dir.display(), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed reading node entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("yaml") {
                continue;
            }
            let snapshot: NodeSnapshot = read_yaml_file(&path)?;
            snapshot.validate()?;
            node_snapshots.push(snapshot);
        }
    }

    node_snapshots.sort_by(|a, b| a.node_key.cmp(&b.node_key));
    Ok((bowties, node_snapshots, offline_changes))
}

/// Locate CDI XML file for a snapshot within a layout directory.
///
/// Returns the path to the CDI file if it exists in the layout's cdi directory,
/// or None if the file is not present.
#[allow(dead_code)]
pub fn get_cdi_path_for_snapshot(
    layout_root: &Path,
    snapshot: &NodeSnapshot,
    _manifest: &LayoutManifest,
) -> Option<std::path::PathBuf> {
    let cdi_dir = layout_root.join(CDI_DIR);
    // Prefer .cdi.xml (current convention), fall back to .xml (legacy layouts)
    let primary = cdi_dir.join(format!("{}.cdi.xml", snapshot.cdi_ref.cache_key));
    if primary.exists() {
        return Some(primary);
    }
    let legacy = cdi_dir.join(format!("{}.xml", snapshot.cdi_ref.cache_key));
    if legacy.exists() {
        return Some(legacy);
    }
    None
}

/// Resolve CDI XML content for a snapshot by checking the global cache first,
/// then the layout companion directory.
///
/// Lookup order (S8.6 — single source: `snapshot.cdi_ref.cache_key`):
/// 1. Global cache: `{app_data_dir}/cdi_cache/{cache_key}.cdi.xml`
/// 2. Layout folder: `{companion_dir}/cdi/{cache_key}.cdi.xml`
/// 3. Layout folder (legacy): `{companion_dir}/cdi/{cache_key}.xml`
///
/// Returns the raw XML string on success.
pub fn resolve_cdi_xml(
    snapshot: &NodeSnapshot,
    app_data_dir: &Path,
    companion_dir: &Path,
) -> Result<String, String> {
    let cache_path = cdi_cache_path(snapshot, app_data_dir);

    if cache_path.exists() {
        return std::fs::read_to_string(&cache_path)
            .map_err(|e| format!("Cannot read CDI cache file {}: {}", cache_path.display(), e));
    }

    // Fallback: layout companion cdi/ directory
    let cdi_dir = companion_dir.join(CDI_DIR);
    let primary = cdi_dir.join(format!("{}.cdi.xml", snapshot.cdi_ref.cache_key));
    if primary.exists() {
        return std::fs::read_to_string(&primary)
            .map_err(|e| format!("Cannot read layout CDI {}: {}", primary.display(), e));
    }
    let legacy = cdi_dir.join(format!("{}.xml", snapshot.cdi_ref.cache_key));
    if legacy.exists() {
        return std::fs::read_to_string(&legacy)
            .map_err(|e| format!("Cannot read layout CDI {}: {}", legacy.display(), e));
    }

    Err(format!(
        "CDI not available (tried {} and {})",
        cache_path.display(),
        primary.display()
    ))
}

/// S8.6: global CDI cache path. Reads only `snapshot.cdi_ref.cache_key` —
/// no parallel SNIP-based derivation. The filename layout matches what
/// `commands/cdi.rs::write_cdi_to_cache` wrote at download time, because
/// both call sites mint the key via `CdiReference::from_snip`.
///
/// Format: `{app_data_dir}/cdi_cache/{cache_key}.cdi.xml`
pub fn cdi_cache_path(snapshot: &NodeSnapshot, app_data_dir: &Path) -> PathBuf {
    cdi_cache_path_for_key(&snapshot.cdi_ref.cache_key, app_data_dir)
}

/// S8.6: bare key → global cache path. Used directly by the live-node
/// download path before a `NodeSnapshot` exists for the node.
pub fn cdi_cache_path_for_key(cache_key: &str, app_data_dir: &Path) -> PathBuf {
    app_data_dir
        .join("cdi_cache")
        .join(format!("{}.cdi.xml", cache_key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use crate::layout::node_snapshot::{
        CaptureStatus, CdiReference, NodeSnapshot, SnapshotLeafValue, SnipSnapshot,
    };
    use crate::layout::types::{BowtieMetadata, RoleClassification};
    use lcc_rs::NodeID;

    fn test_node_snapshot(node_id: &str) -> NodeSnapshot {
        let nid = NodeID::from_hex_string(node_id).unwrap();
        let mut snapshot = NodeSnapshot {
            node_key: nid.to_canonical(),
            node_id: Some(nid),
            profile_stem: None,
            lifecycle: crate::layout::node_snapshot::NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-04-05T12:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot {
                user_name: "Node".to_string(),
                user_description: "Test".to_string(),
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
        snapshot.add_config_leaf(
            &["seg:0".to_string(), "elem:1".to_string()],
            SnapshotLeafValue {
                value: "42".to_string(),
                space: Some(253),
                offset: Some("0x00000010".to_string()),
            },
        );
        snapshot
    }

    #[test]
    fn roundtrip_save_load() {
        let dir = std::env::temp_dir().join("bowties_test_roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test-layout.bowties.yaml");

        let mut layout = LayoutFile::default();
        layout.bowties.insert(
            "05.01.01.01.FF.00.00.01".to_string(),
            BowtieMetadata {
                name: Some("Test Signal".to_string()),
                tags: vec!["signals".to_string(), "yard".to_string()],
            },
        );
        layout.role_classifications.insert(
            "05.02.01.02.03.00:Port/Line/Event".to_string(),
            RoleClassification { role: "Producer".to_string() },
        );

        save_file(&path, &layout).unwrap();
        let loaded = load_file(&path).unwrap();

        assert_eq!(loaded.schema_version, "1.0");
        assert_eq!(loaded.bowties.len(), 1);
        let meta = loaded.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(meta.name, Some("Test Signal".to_string()));
        assert_eq!(meta.tags, vec!["signals", "yard"]);
        assert_eq!(loaded.role_classifications.len(), 1);

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn load_missing_file() {
        let result = load_file(Path::new("/nonexistent/path/layout.yaml"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = std::env::temp_dir().join("bowties_test_parents").join("sub").join("dir");
        let path = dir.join("layout.bowties.yaml");

        let layout = LayoutFile::default();
        save_file(&path, &layout).unwrap();

        assert!(path.exists());

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("bowties_test_parents"));
    }

    #[test]
    fn write_read_base_file_capture_roundtrip() {
        let root = std::env::temp_dir().join("bowties_test_capture_base_roundtrip");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("np-layout");
        let manifest = LayoutManifest::new(
            "np-layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &data).unwrap();
        assert!(layout_dir.exists());

        let loaded = read_layout_capture(&layout_dir).unwrap();
        assert_eq!(loaded.manifest.layout_id, "np-layout");
        assert_eq!(loaded.node_snapshots.len(), 1);
        assert!(!loaded.node_snapshots[0].config.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn repeated_base_file_saves_are_deterministic() {
        let root = std::env::temp_dir().join("bowties_test_deterministic_save");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("layout");
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &data).unwrap();
        let first = std::fs::read_to_string(layout_dir.to_path_buf().join("nodes").join("050101011402.yaml")).unwrap();

        write_layout_capture(&layout_dir, &data).unwrap();
        let second = std::fs::read_to_string(layout_dir.to_path_buf().join("nodes").join("050101011402.yaml")).unwrap();

        assert_eq!(first, second);

        let _ = std::fs::remove_dir_all(&root);
    }

    fn test_node_no_cdi(node_id: &str) -> NodeSnapshot {
        let nid = NodeID::from_hex_string(node_id).unwrap();
        NodeSnapshot {
            node_key: nid.to_canonical(),
            node_id: Some(nid),
            profile_stem: None,
            lifecycle: crate::layout::node_snapshot::NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-04-05T12:00:00Z".to_string(),
            capture_status: CaptureStatus::Partial,
            missing: vec!["configuration tree not available".to_string()],
            snip: SnipSnapshot {
                user_name: "JMRI".to_string(),
                user_description: "".to_string(),
                manufacturer_name: "JMRI".to_string(),
                model_name: "LccPro".to_string(),
            },
            cdi_ref: CdiReference {
                cache_key: "JMRI_LccPro_5.14".to_string(),
                version: "5.14".to_string(),
                fingerprint: "not_supported".to_string(),
            },
            config: BTreeMap::new(),
            producer_identified_events: Vec::new(),
        }
    }

    #[test]
    fn roundtrip_node_without_cdi() {
        let root = std::env::temp_dir().join("bowties_test_no_cdi_roundtrip");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("layout");
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_no_cdi("0201120033CC")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &data).unwrap();
        let loaded = read_layout_capture(&layout_dir).unwrap();

        assert_eq!(loaded.node_snapshots.len(), 1);
        let snap = &loaded.node_snapshots[0];
        assert_eq!(snap.node_id, Some(NodeID::from_hex_string("0201120033CC").unwrap()));
        assert_eq!(snap.cdi_ref.fingerprint, "not_supported");
        assert_eq!(snap.capture_status, CaptureStatus::Partial);
        assert!(snap.config.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn roundtrip_mixed_cdi_and_no_cdi_nodes() {
        let root = std::env::temp_dir().join("bowties_test_mixed_cdi");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("mixed");
        let manifest = LayoutManifest::new(
            "mixed".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
        );

        // Create a fake CDI file in a temp location to simulate cache
        let cdi_source = root.join("acme_modelx_1.0.cdi.xml");
        std::fs::write(&cdi_source, "<cdi/>").unwrap();

        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![
                test_node_snapshot("050101011402"),
                test_node_no_cdi("0201120033CC"),
            ],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: vec![("acme_modelx_1.0".to_string(), cdi_source)],
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &data).unwrap();
        let loaded = read_layout_capture(&layout_dir).unwrap();

        assert_eq!(loaded.node_snapshots.len(), 2);

        let cdi_node = loaded.node_snapshots.iter().find(|n| n.node_id == Some(NodeID::from_hex_string("050101011402").unwrap())).unwrap();
        assert_eq!(cdi_node.cdi_ref.fingerprint, "len:123");
        assert!(!cdi_node.config.is_empty());

        let no_cdi_node = loaded.node_snapshots.iter().find(|n| n.node_id == Some(NodeID::from_hex_string("0201120033CC").unwrap())).unwrap();
        assert_eq!(no_cdi_node.cdi_ref.fingerprint, "not_supported");
        assert!(no_cdi_node.config.is_empty());

        // Verify CDI file was copied for the CDI node
        // layout_dir IS the layout folder
        let cdi_dest = layout_dir.join("cdi").join("acme_modelx_1.0.cdi.xml");
        assert!(cdi_dest.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    /// S8.5 / T5 parity test: a layout directory whose only NodeSnapshot is a
    /// placeholder (node_id: None, node_key starts with "placeholder:") writes
    /// to disk and reads back through the same `write_layout_capture` /
    /// `read_layout_capture` path used by real nodes — no special-case branch.
    /// The on-disk filename is derived from `filename_basis()`
    /// (`placeholder_<uuid>.yaml`).
    #[test]
    fn s8_5_placeholder_snapshot_round_trips_through_layout_io_like_real_node() {
        let root = std::env::temp_dir().join("bowties_test_s8_5_placeholder_parity");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("placeholder-parity");
        let manifest = LayoutManifest::new(
            "placeholder-parity".to_string(),
            "2026-05-25T00:00:00Z".to_string(),
            "2026-05-25T00:00:00Z".to_string(),
        );

        let placeholder_key = "placeholder:11111111-2222-4333-8444-555555555555".to_string();
        let mut placeholder_snap = NodeSnapshot {
            node_key: placeholder_key.clone(),
            node_id: None,
            profile_stem: Some("RR-CirKits_Tower-LCC".to_string()),
            lifecycle: crate::layout::node_snapshot::NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-05-25T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot {
                user_name: "My Tower".to_string(),
                user_description: String::new(),
                manufacturer_name: "RR-CirKits".to_string(),
                model_name: "Tower-LCC".to_string(),
            },
            cdi_ref: CdiReference {
                cache_key: "RR-CirKits_Tower-LCC".to_string(),
                version: "bundled".to_string(),
                fingerprint: "bundled".to_string(),
            },
            config: BTreeMap::new(),
            producer_identified_events: Vec::new(),
        };
        placeholder_snap.add_config_leaf(
            &["Segment 0".to_string(), "User Name".to_string()],
            SnapshotLeafValue {
                value: "Hello".to_string(),
                space: Some(253),
                offset: Some("0x00000000".to_string()),
            },
        );

        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![placeholder_snap],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &data).unwrap();

        // The on-disk filename uses the placeholder filename basis (no `:`).
        // layout_dir IS the layout folder
        let nodes_dir = layout_dir.join(NODES_DIR);
        let on_disk = nodes_dir.join("PLACEHOLDER_11111111-2222-4333-8444-555555555555.yaml");
        assert!(
            on_disk.exists(),
            "expected placeholder snapshot at {}, dir contains: {:?}",
            on_disk.display(),
            std::fs::read_dir(&nodes_dir)
                .ok()
                .map(|it| it.filter_map(|e| e.ok().map(|e| e.file_name())).collect::<Vec<_>>())
        );

        let loaded = read_layout_capture(&layout_dir).unwrap();
        assert_eq!(loaded.node_snapshots.len(), 1);
        let snap = &loaded.node_snapshots[0];
        assert_eq!(snap.node_key, placeholder_key);
        assert_eq!(snap.node_id, None);
        assert_eq!(snap.cdi_ref.cache_key, "RR-CirKits_Tower-LCC");
        assert_eq!(snap.capture_status, CaptureStatus::Complete);
        assert!(!snap.config.is_empty(), "config tree must round-trip");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resave_without_new_cdi_files_preserves_existing_cdi_directory() {
        let root = std::env::temp_dir().join("bowties_test_preserve_existing_cdi");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("layout");
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
        );

        let cdi_source = root.join("acme_modelx_1.0.cdi.xml");
        std::fs::write(&cdi_source, "<cdi version=\"1\"/>").unwrap();

        let initial_data = LayoutDirectoryWriteData {
            manifest: manifest.clone(),
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: vec![("acme_modelx_1.0".to_string(), cdi_source)],
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &initial_data).unwrap();

        let resave_data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &resave_data).unwrap();

        // layout_dir IS the layout folder
        let cdi_dest = layout_dir.join("cdi").join("acme_modelx_1.0.cdi.xml");
        assert!(cdi_dest.exists());
        assert_eq!(std::fs::read_to_string(&cdi_dest).unwrap(), "<cdi version=\"1\"/>");

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Spec 013 / S4: per-layout connections in manifest ─────────────────

    fn test_connection(id: &str, name: &str) -> crate::layout::types::ConnectionConfig {
        crate::layout::types::ConnectionConfig {
            id: id.to_string(),
            name: name.to_string(),
            adapter_type: crate::layout::types::AdapterType::Tcp,
            host: Some("localhost".to_string()),
            port: Some(12021),
            serial_port: None,
            baud_rate: None,
            flow_control: crate::layout::types::FlowControl::None,
        }
    }

    #[test]
    fn s4_existing_layout_without_connections_field_opens_with_empty_list() {
        // An older layout file written before Spec 013 / S4 had no
        // `connections` field. Opening it must succeed and produce an
        // empty connections list — no migration required.
        let root = std::env::temp_dir().join("bowties_s4_legacy_open");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("legacy");
        let manifest = LayoutManifest::new(
            "legacy".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };
        write_layout_capture(&layout_dir, &data).unwrap();

        // Rewrite the manifest with the `connections` field stripped so
        // the file looks like one written by an older Bowties build.
        let manifest_path = layout_dir.join(MANIFEST_FILE);
        let raw = std::fs::read_to_string(&manifest_path).unwrap();
        let stripped: String = raw
            .lines()
            .filter(|l| !l.starts_with("connections"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&manifest_path, stripped).unwrap();

        let loaded = read_layout_capture(&layout_dir).unwrap();
        assert!(loaded.manifest.connections.is_empty(),
            "legacy layout must open with empty connections list");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn s4_layout_connections_roundtrip_through_update_manifest_connections() {
        let root = std::env::temp_dir().join("bowties_s4_connections_roundtrip");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("rt");
        let manifest = LayoutManifest::new(
            "rt".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
            "2026-05-23T00:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };
        write_layout_capture(&layout_dir, &data).unwrap();

        // Initially empty.
        let initial = crate::layout::read_manifest(&layout_dir).unwrap();
        assert!(initial.connections.is_empty());

        // Write a single connection and round-trip it.
        let conn_a = test_connection("aaa", "JMRI hub");
        crate::layout::update_manifest_connections(&layout_dir, vec![conn_a.clone()]).unwrap();
        let after_one = crate::layout::read_manifest(&layout_dir).unwrap();
        assert_eq!(after_one.connections, vec![conn_a.clone()]);

        // The companion-directory contents must be untouched.
        // layout_dir IS the layout folder
        assert!(layout_dir.join("nodes").join("050101011402.yaml").exists());

        // Write multiple connections in a defined order.
        let conn_b = crate::layout::types::ConnectionConfig {
            id: "bbb".to_string(),
            name: "USB GridConnect".to_string(),
            adapter_type: crate::layout::types::AdapterType::GridConnectSerial,
            host: None,
            port: None,
            serial_port: Some("COM7".to_string()),
            baud_rate: Some(57600),
            flow_control: crate::layout::types::FlowControl::RtsCts,
        };
        let conn_c = test_connection("ccc", "Secondary hub");
        crate::layout::update_manifest_connections(
            &layout_dir,
            vec![conn_a.clone(), conn_b.clone(), conn_c.clone()],
        )
        .unwrap();

        let after_three = crate::layout::read_manifest(&layout_dir).unwrap();
        assert_eq!(after_three.connections, vec![conn_a, conn_b, conn_c]);

        // Round-trip through the full capture reader too, to confirm the
        // companion directory still loads cleanly alongside the new field.
        let full = read_layout_capture(&layout_dir).unwrap();
        assert_eq!(full.manifest.connections.len(), 3);
        assert_eq!(full.node_snapshots.len(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resave_preserves_existing_connections_via_build_save_manifest() {
        // Regression: opening a layout that already has a saved LCC
        // connection, making any change, and re-saving must not drop the
        // connection. The previous bug was in `save_layout_directory`,
        // which constructed the new manifest via `LayoutManifest::new(...)`
        // and silently zeroed `connections`. The fix is the
        // `manifest::build_save_manifest` helper used at that seam; this
        // test pins the contract at the io layer.
        let root = std::env::temp_dir().join("bowties_resave_preserves_connections");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("rs");
        let initial_manifest = LayoutManifest::new(
            "rs".to_string(),
            "2026-06-01T00:00:00Z".to_string(),
            "2026-06-01T00:00:00Z".to_string(),
        );
        let initial = LayoutDirectoryWriteData {
            manifest: initial_manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };
        write_layout_capture(&layout_dir, &initial).unwrap();

        // User adds a connection via ConnectionManager.
        let conn = test_connection("conn-1", "Home TCP");
        crate::layout::update_manifest_connections(&layout_dir, vec![conn.clone()]).unwrap();

        // Simulate `save_layout_directory`: read previous capture, build a
        // new manifest via the helper, and write through `save_capture`.
        let previous = crate::layout::read_capture(&layout_dir).unwrap();
        let new_manifest = crate::layout::manifest::build_save_manifest(
            Some(&previous.manifest),
            previous.manifest.layout_id.clone(),
            previous.manifest.captured_at.clone(),
            "2026-06-01T00:05:00Z".to_string(),
        );
        let resave = LayoutDirectoryWriteData {
            manifest: new_manifest,
            node_snapshots: previous.node_snapshots.clone(),
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };
        write_layout_capture(&layout_dir, &resave).unwrap();

        let after = crate::layout::read_manifest(&layout_dir).unwrap();
        assert_eq!(after.connections, vec![conn],
            "re-save must preserve connections from the previous manifest");

        let _ = std::fs::remove_dir_all(&root);
    }

    // ---- S8.6: single CDI artifact resolver (cdi_cache_path reads cache_key) ----

    fn snapshot_with_cdi_ref(cdi_ref: CdiReference) -> NodeSnapshot {
        NodeSnapshot {
            node_key: "050101010301".to_string(),
            node_id: Some(NodeID::from_hex_string("050101010301").unwrap()),
            profile_stem: None,
            lifecycle: crate::layout::node_snapshot::NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-05-25T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            // Deliberately set SNIP fields to values that would produce a
            // DIFFERENT filename under the legacy SNIP-synthesis rule, to
            // prove the resolver no longer touches SNIP.
            snip: SnipSnapshot {
                user_name: String::new(),
                user_description: String::new(),
                manufacturer_name: "DIFFERENT".to_string(),
                model_name: "VALUES".to_string(),
            },
            cdi_ref,
            config: BTreeMap::new(),
            producer_identified_events: Vec::new(),
        }
    }

    #[test]
    fn s8_6_cdi_cache_path_reads_only_cache_key() {
        let app_data = std::path::PathBuf::from("/fake/app_data");
        let cdi_ref = CdiReference::from_snip(
            &SnipSnapshot {
                user_name: String::new(),
                user_description: String::new(),
                manufacturer_name: "Mustangpeak Engineering".to_string(),
                model_name: "TurnoutBoss".to_string(),
            },
            "5.14",
            "len:1000",
        );
        let snap = snapshot_with_cdi_ref(cdi_ref);
        let path = cdi_cache_path(&snap, &app_data);
        assert_eq!(
            path,
            std::path::PathBuf::from("/fake/app_data/cdi_cache/Mustangpeak_Engineering_TurnoutBoss_5_14.cdi.xml"),
        );
    }

    #[test]
    fn s8_6_cdi_cache_path_for_placeholder_uses_profile_stem() {
        let app_data = std::path::PathBuf::from("/fake/app_data");
        // Regression: the placeholder save-flush previously failed because
        // `cdi_cache_path` derived `Mustangpeak_Engineering_TurnoutBoss_bundled.cdi.xml`
        // from SNIP+version, while the cache_key was the profile stem
        // `Mustangpeak-Engineering_TurnoutBoss` (hyphen preserved). After
        // S8.6 the resolver reads `cache_key` directly.
        let cdi_ref = CdiReference::from_profile_stem("Mustangpeak-Engineering_TurnoutBoss");
        let snap = snapshot_with_cdi_ref(cdi_ref);
        let path = cdi_cache_path(&snap, &app_data);
        assert_eq!(
            path,
            std::path::PathBuf::from("/fake/app_data/cdi_cache/Mustangpeak-Engineering_TurnoutBoss.cdi.xml"),
        );
    }

    #[test]
    fn s8_6_cdi_cache_path_for_key_matches_snapshot_resolver() {
        let app_data = std::path::PathBuf::from("/fake/app_data");
        let by_key = cdi_cache_path_for_key("acme_modelx_1_0", &app_data);
        let snap = snapshot_with_cdi_ref(CdiReference {
            cache_key: "acme_modelx_1_0".to_string(),
            version: "1.0".to_string(),
            fingerprint: "len:123".to_string(),
        });
        assert_eq!(by_key, cdi_cache_path(&snap, &app_data));
    }

    #[test]
    fn unchanged_node_snapshot_not_rewritten_on_resave() {
        // ADR-0006: "Files that did not change are not rewritten."
        // When a node snapshot is byte-identical to what is already on disk,
        // the save plan must skip the write so file timestamps and cloud
        // sync are not disturbed.
        let root = std::env::temp_dir().join("bowties_test_unchanged_skip");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let layout_dir = root.join("layout");
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest: manifest.clone(),
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
            channels: crate::layout::channels::ChannelsDocument::default(),
        };

        write_layout_capture(&layout_dir, &data).unwrap();
        let node_path = layout_dir.to_path_buf()
            .join("nodes").join("050101011402.yaml");
        let mtime_after_first = std::fs::metadata(&node_path).unwrap().modified().unwrap();

        // Brief sleep so filesystem timestamp would differ if rewritten
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Re-save with identical data
        write_layout_capture(&layout_dir, &data).unwrap();
        let mtime_after_second = std::fs::metadata(&node_path).unwrap().modified().unwrap();

        assert_eq!(
            mtime_after_first, mtime_after_second,
            "Unchanged node file should not have been rewritten"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}



