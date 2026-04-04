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

#[derive(Debug, Clone)]
pub struct LayoutDirectoryWriteData {
    pub manifest: LayoutManifest,
    pub node_snapshots: Vec<NodeSnapshot>,
    pub bowties: LayoutFile,
    pub offline_changes: Vec<OfflineChange>,
}

#[derive(Debug, Clone)]
pub struct LayoutDirectoryReadData {
    pub manifest: LayoutManifest,
    pub node_snapshots: Vec<NodeSnapshot>,
    pub bowties: LayoutFile,
    pub offline_changes: Vec<OfflineChange>,
}

pub fn write_layout_directory(root_dir: &Path, data: &LayoutDirectoryWriteData) -> Result<(), String> {
    save_directory_atomic(root_dir, |staging_dir| {
        write_yaml_file(&staging_dir.join("manifest.yaml"), &data.manifest)?;
        write_yaml_file(&staging_dir.join("bowties.yaml"), &data.bowties)?;
        write_yaml_file(&staging_dir.join("offline-changes.yaml"), &data.offline_changes)?;
        write_yaml_file(&staging_dir.join("event-names.yaml"), &std::collections::BTreeMap::<String, String>::new())?;

        let nodes_dir = staging_dir.join("nodes");
        std::fs::create_dir_all(&nodes_dir)
            .map_err(|e| format!("Cannot create nodes dir {}: {}", nodes_dir.display(), e))?;
        for snapshot in &data.node_snapshots {
            let node_path = derive_node_file_path(&nodes_dir, &snapshot.node_id.replace('.', ""));
            write_yaml_file(&node_path, snapshot)?;
        }

        Ok(())
    })
}

pub fn read_layout_directory(root_dir: &Path) -> Result<LayoutDirectoryReadData, String> {
    let manifest_path = root_dir.join("manifest.yaml");
    let manifest: LayoutManifest = read_yaml_file(&manifest_path)?;
    manifest.validate()?;

    let bowties_path = root_dir.join(&manifest.files.bowties);
    let bowties: LayoutFile = if bowties_path.exists() {
        read_yaml_file(&bowties_path)?
    } else {
        LayoutFile::default()
    };

    let offline_changes_path = root_dir.join(&manifest.files.offline_changes);
    let offline_changes: Vec<OfflineChange> = if offline_changes_path.exists() {
        read_yaml_file(&offline_changes_path)?
    } else {
        Vec::new()
    };

    let nodes_dir = root_dir.join(&manifest.files.nodes_dir);
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

    Ok(LayoutDirectoryReadData {
        manifest,
        node_snapshots,
        bowties,
        offline_changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::types::{BowtieMetadata, RoleClassification};

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
}
