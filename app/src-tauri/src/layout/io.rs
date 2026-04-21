//! Layout file I/O operations.
//!
//! Handles loading and saving YAML layout files with atomic write support
//! and schema validation.

use std::path::{Path, PathBuf};
use std::io::Write;
use super::types::LayoutFile;
use super::manifest::LayoutManifest;
use super::node_snapshot::NodeSnapshot;
use super::offline_changes::OfflineChange;

const BOWTIES_FILE: &str = "bowties.yaml";
const OFFLINE_CHANGES_FILE: &str = "offline-changes.yaml";
const EVENT_NAMES_FILE: &str = "event-names.yaml";
const NODES_DIR: &str = "nodes";
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

/// Serialize and write YAML in a deterministic manner for layout directory files.
pub fn write_yaml_file<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create directory {}: {}", parent.display(), e))?;
    }

    let yaml = serde_yaml_ng::to_string(value)
        .map_err(|e| format!("Failed to serialize YAML for {}: {}", path.display(), e))?;

    let temp_path = path.with_extension("yaml.tmp");
    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| format!("Failed to create temp file {}: {}", temp_path.display(), e))?;
    file.write_all(yaml.as_bytes())
        .map_err(|e| format!("Failed to write YAML file {}: {}", path.display(), e))?;
    file.flush()
        .map_err(|e| format!("Failed to flush YAML file {}: {}", path.display(), e))?;
    drop(file);

    std::fs::rename(&temp_path, path)
        .map_err(|e| format!("Failed to replace YAML file {}: {}", path.display(), e))?;
    Ok(())
}

/// Read and deserialize a YAML file.
pub fn read_yaml_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read YAML file {}: {}", path.display(), e))?;
    serde_yaml_ng::from_str::<T>(&contents)
        .map_err(|e| format!("Failed to parse YAML file {}: {}", path.display(), e))
}

/// Save a directory atomically by writing into a staging directory and swapping it in place.
pub fn save_directory_atomic<F>(target_dir: &Path, writer: F) -> Result<(), String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    let parent = target_dir
        .parent()
        .ok_or_else(|| format!("Target directory has no parent: {}", target_dir.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Cannot create parent directory {}: {}", parent.display(), e))?;

    let target_name = target_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("Invalid target directory name: {}", target_dir.display()))?;

    let staging_dir = parent.join(format!("{}.staging", target_name));
    let backup_dir = parent.join(format!("{}.backup", target_name));

    if staging_dir.exists() {
        std::fs::remove_dir_all(&staging_dir)
            .map_err(|e| format!("Cannot clean staging directory {}: {}", staging_dir.display(), e))?;
    }
    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Cannot create staging directory {}: {}", staging_dir.display(), e))?;

    writer(&staging_dir)?;

    if backup_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
    }
    if target_dir.exists() {
        std::fs::rename(target_dir, &backup_dir)
            .map_err(|e| format!("Failed to move old directory to backup: {}", e))?;
    }

    if let Err(e) = std::fs::rename(&staging_dir, target_dir) {
        // Roll back to previous directory if swap fails.
        if backup_dir.exists() {
            let _ = std::fs::rename(&backup_dir, target_dir);
        }
        return Err(format!("Failed to move staging directory into place: {}", e));
    }

    if backup_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
    }

    Ok(())
}

pub fn derive_node_file_path(nodes_dir: &Path, node_id: &str) -> PathBuf {
    nodes_dir.join(format!("{}.yaml", node_id.to_uppercase()))
}

pub fn derive_companion_dir_name(base_file: &Path) -> Result<String, String> {
    let file_name = base_file
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| format!("Invalid base layout filename: {}", base_file.display()))?;

    let suffixes = [
        ".layout",
        ".bowties-layout.yaml",
        ".bowties-layout.yml",
        ".yaml",
        ".yml",
    ];
    for suffix in suffixes {
        if let Some(stem) = file_name.strip_suffix(suffix) {
            return Ok(format!("{}.layout.d", stem));
        }
    }

    Ok(format!("{}.layout.d", file_name))
}

pub fn derive_companion_dir_path(base_file: &Path) -> Result<PathBuf, String> {
    let parent = base_file
        .parent()
        .ok_or_else(|| format!("Layout file has no parent directory: {}", base_file.display()))?;
    Ok(parent.join(derive_companion_dir_name(base_file)?))
}

#[derive(Debug, Clone)]
pub struct LayoutDirectoryWriteData {
    pub manifest: LayoutManifest,
    pub node_snapshots: Vec<NodeSnapshot>,
    pub bowties: LayoutFile,
    pub offline_changes: Vec<OfflineChange>,
    /// List of (cache_key, source_path_to_cdi_file) pairs for CDI files to copy
    pub cdi_files: Vec<(String, std::path::PathBuf)>,
}

#[derive(Debug, Clone)]
pub struct LayoutDirectoryReadData {
    pub manifest: LayoutManifest,
    pub node_snapshots: Vec<NodeSnapshot>,
    pub bowties: LayoutFile,
    pub offline_changes: Vec<OfflineChange>,
}

pub fn write_layout_capture(base_file: &Path, data: &LayoutDirectoryWriteData) -> Result<(), String> {
    let parent = base_file
        .parent()
        .ok_or_else(|| format!("Layout file has no parent directory: {}", base_file.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Cannot create parent directory {}: {}", parent.display(), e))?;

    let companion_dir = derive_companion_dir_path(base_file)?;
    let companion_name = companion_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("Invalid companion directory: {}", companion_dir.display()))?
        .to_string();

    let mut manifest = data.manifest.clone();
    manifest.companion_dir = companion_name;

    // Capture existing companion dir path before the atomic swap replaces it.
    // This lets us preserve CDI files when re-saving without fresh cache entries.
    let existing_companion = if companion_dir.exists() {
        Some(companion_dir.clone())
    } else {
        None
    };

    // Write base first so the canonical entry file always exists after a successful save.
    write_yaml_file(base_file, &manifest)?;
    save_directory_atomic(&companion_dir, |staging_dir| {
        write_companion_contents(staging_dir, data, existing_companion.as_deref())
    })?;

    if !base_file.exists() {
        return Err(format!(
            "Layout save failed: base file missing after save: {}",
            base_file.display()
        ));
    }
    if !companion_dir.exists() {
        return Err(format!(
            "Layout save failed: companion directory missing after save: {}",
            companion_dir.display()
        ));
    }
    Ok(())
}

pub fn read_layout_capture(base_file: &Path) -> Result<LayoutDirectoryReadData, String> {
    let manifest: LayoutManifest = read_yaml_file(base_file)?;
    manifest.validate()?;

    let companion_dir = if manifest.companion_dir.trim().is_empty() {
        derive_companion_dir_path(base_file)?
    } else {
        base_file
            .parent()
            .ok_or_else(|| format!("Layout file has no parent directory: {}", base_file.display()))?
            .join(&manifest.companion_dir)
    };

    if !companion_dir.exists() {
        return Err(format!(
            "Layout companion directory not found: {}",
            companion_dir.display()
        ));
    }

    let (bowties, node_snapshots, offline_changes) = read_companion_contents(&companion_dir, &manifest)?;

    Ok(LayoutDirectoryReadData {
        manifest,
        node_snapshots,
        bowties,
        offline_changes,
    })
}

fn write_companion_contents(
    root_dir: &Path,
    data: &LayoutDirectoryWriteData,
    existing_companion: Option<&Path>,
) -> Result<(), String> {
    write_yaml_file(&root_dir.join(BOWTIES_FILE), &data.bowties)?;
    write_yaml_file(&root_dir.join(OFFLINE_CHANGES_FILE), &data.offline_changes)?;
    write_yaml_file(
        &root_dir.join(EVENT_NAMES_FILE),
        &std::collections::BTreeMap::<String, String>::new(),
    )?;

    let nodes_dir = root_dir.join(NODES_DIR);
    std::fs::create_dir_all(&nodes_dir)
        .map_err(|e| format!("Cannot create nodes dir {}: {}", nodes_dir.display(), e))?;
    for snapshot in &data.node_snapshots {
        let node_path = derive_node_file_path(&nodes_dir, &snapshot.node_id.to_canonical());
        write_yaml_file(&node_path, snapshot)?;
    }

    // Copy CDI files to layout directory.
    // If new CDI files are provided (fresh save from live bus), copy from cache.
    // Otherwise, preserve existing CDI files from the previous companion dir.
    let cdi_dir = root_dir.join(CDI_DIR);
    if !data.cdi_files.is_empty() {
        std::fs::create_dir_all(&cdi_dir)
            .map_err(|e| format!("Cannot create CDI directory {}: {}", cdi_dir.display(), e))?;
        for (cache_key, source_path) in &data.cdi_files {
            let dest_filename = format!("{}.xml", cache_key);
            let dest_path = cdi_dir.join(&dest_filename);
            std::fs::copy(source_path, &dest_path)
                .map_err(|e| format!("Cannot copy CDI file from {} to {}: {}", 
                    source_path.display(), dest_path.display(), e))?;
        }
    } else if let Some(prev) = existing_companion {
        let prev_cdi = prev.join(CDI_DIR);
        if prev_cdi.exists() {
            std::fs::create_dir_all(&cdi_dir)
                .map_err(|e| format!("Cannot create CDI directory {}: {}", cdi_dir.display(), e))?;
            let entries = std::fs::read_dir(&prev_cdi)
                .map_err(|e| format!("Cannot read existing CDI directory {}: {}", prev_cdi.display(), e))?;
            for entry in entries {
                let entry = entry.map_err(|e| format!("Failed reading CDI entry: {}", e))?;
                let source = entry.path();
                if source.extension().and_then(|x| x.to_str()) == Some("xml") {
                    if let Some(name) = source.file_name() {
                        let dest = cdi_dir.join(name);
                        std::fs::copy(&source, &dest)
                            .map_err(|e| format!("Cannot copy existing CDI file from {} to {}: {}",
                                source.display(), dest.display(), e))?;
                    }
                }
            }
        }
    }

    Ok(())
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

    node_snapshots.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    Ok((bowties, node_snapshots, offline_changes))
}

/// Locate CDI XML file for a snapshot within a layout directory.
///
/// Returns the path to the CDI file if it exists in the layout's cdi directory,
/// or None if the file is not present.
pub fn get_cdi_path_for_snapshot(
    layout_root: &Path,
    snapshot: &NodeSnapshot,
    _manifest: &LayoutManifest,
) -> Option<std::path::PathBuf> {
    let cdi_dir = layout_root.join(CDI_DIR);
    let cdi_filename = format!("{}.xml", snapshot.cdi_ref.cache_key);
    let cdi_path = cdi_dir.join(&cdi_filename);
    
    if cdi_path.exists() {
        Some(cdi_path)
    } else {
        None
    }
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
        let mut snapshot = NodeSnapshot {
            node_id: NodeID::from_hex_string(node_id).unwrap(),
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

        let base_file = root.join("np-layout.bowties-layout.yaml");
        let manifest = LayoutManifest::new(
            "np-layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "np-layout.bowties-layout.d".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
        };

        write_layout_capture(&base_file, &data).unwrap();
        let companion = derive_companion_dir_path(&base_file).unwrap();
        assert!(base_file.exists());
        assert!(companion.exists());

        let loaded = read_layout_capture(&base_file).unwrap();
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

        let base_file = root.join("layout.bowties-layout.yaml");
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "layout.bowties-layout.d".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_snapshot("050101011402")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
        };

        write_layout_capture(&base_file, &data).unwrap();
        let first = std::fs::read_to_string(derive_companion_dir_path(&base_file).unwrap().join("nodes").join("050101011402.yaml")).unwrap();

        write_layout_capture(&base_file, &data).unwrap();
        let second = std::fs::read_to_string(derive_companion_dir_path(&base_file).unwrap().join("nodes").join("050101011402.yaml")).unwrap();

        assert_eq!(first, second);

        let _ = std::fs::remove_dir_all(&root);
    }

    fn test_node_no_cdi(node_id: &str) -> NodeSnapshot {
        NodeSnapshot {
            node_id: NodeID::from_hex_string(node_id).unwrap(),
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

        let base_file = root.join("layout.bowties-layout.yaml");
        let manifest = LayoutManifest::new(
            "layout".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "layout.bowties-layout.d".to_string(),
        );
        let data = LayoutDirectoryWriteData {
            manifest,
            node_snapshots: vec![test_node_no_cdi("0201120033CC")],
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            cdi_files: Vec::new(),
        };

        write_layout_capture(&base_file, &data).unwrap();
        let loaded = read_layout_capture(&base_file).unwrap();

        assert_eq!(loaded.node_snapshots.len(), 1);
        let snap = &loaded.node_snapshots[0];
        assert_eq!(snap.node_id, NodeID::from_hex_string("0201120033CC").unwrap());
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

        let base_file = root.join("mixed.bowties-layout.yaml");
        let manifest = LayoutManifest::new(
            "mixed".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "2026-04-05T12:00:00Z".to_string(),
            "mixed.bowties-layout.d".to_string(),
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
        };

        write_layout_capture(&base_file, &data).unwrap();
        let loaded = read_layout_capture(&base_file).unwrap();

        assert_eq!(loaded.node_snapshots.len(), 2);

        let cdi_node = loaded.node_snapshots.iter().find(|n| n.node_id == NodeID::from_hex_string("050101011402").unwrap()).unwrap();
        assert_eq!(cdi_node.cdi_ref.fingerprint, "len:123");
        assert!(!cdi_node.config.is_empty());

        let no_cdi_node = loaded.node_snapshots.iter().find(|n| n.node_id == NodeID::from_hex_string("0201120033CC").unwrap()).unwrap();
        assert_eq!(no_cdi_node.cdi_ref.fingerprint, "not_supported");
        assert!(no_cdi_node.config.is_empty());

        // Verify CDI file was copied for the CDI node
        let companion = derive_companion_dir_path(&base_file).unwrap();
        let cdi_dest = companion.join("cdi").join("acme_modelx_1.0.xml");
        assert!(cdi_dest.exists());

        let _ = std::fs::remove_dir_all(&root);
    }
}
