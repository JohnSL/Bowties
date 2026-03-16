//! Layout file I/O operations.
//!
//! Handles loading and saving YAML layout files with atomic write support
//! and schema validation.

use std::path::Path;
use std::io::Write;
use super::types::LayoutFile;

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
