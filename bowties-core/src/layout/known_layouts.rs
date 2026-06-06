//! Known-layout registry (Spec 013 / S5).
//!
//! Persists the app-level list of layouts the user has previously
//! opened, so the layout picker (S6) can show "name, location,
//! last-opened date" without scanning the filesystem.
//!
//! Storage: `$APPDATA/bowties/known-layouts.json`. The registry is
//! pure app preferences — the layout files themselves do not know
//! about it. Removing an entry only forgets the path; the layout's
//! `.layout` file and companion directory are left untouched.
//!
//! Reads filter out entries whose paths no longer exist on disk
//! (defensive, mirrors the recent-layout helper). Writes are atomic
//! through the same temp→flush→fsync→rename pattern used for
//! `connections.json`, so a power-loss never leaves a half-written
//! registry file.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// A single layout known to the app.
///
/// Field naming follows `camelCase` over the JSON wire so the
/// frontend `KnownLayoutEntry` shape (Spec 013 / S5-T5) can use the
/// values directly without an adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KnownLayoutEntry {
    /// Display name shown in the layout picker.
    pub name: String,
    /// Absolute path to the `.layout` base file.
    pub path: String,
    /// ISO 8601 timestamp of the most recent open.
    pub last_opened: String,
}

/// Read all entries, returning only those whose `path` still exists.
///
/// A missing or unparseable registry file is treated as an empty
/// list (the picker should be usable on first launch).
pub fn load_known_layouts(registry_path: &Path) -> Vec<KnownLayoutEntry> {
    load_raw(registry_path)
        .into_iter()
        .filter(|e| Path::new(&e.path).exists())
        .collect()
}

/// Add or refresh an entry. If an entry with the same `path` already
/// exists it is replaced (preserving registry order — new entries
/// are appended). Returns the post-write filtered list.
pub fn add_known_layout(
    registry_path: &Path,
    entry: KnownLayoutEntry,
) -> Result<Vec<KnownLayoutEntry>, String> {
    let mut entries = load_raw(registry_path);
    entries.retain(|e| e.path != entry.path);
    entries.push(entry);
    write_registry(registry_path, &entries)?;
    Ok(load_known_layouts(registry_path))
}

/// Remove an entry by `path`. Does not touch the layout files on
/// disk. Returns the post-write filtered list.
pub fn remove_known_layout(
    registry_path: &Path,
    layout_path: &str,
) -> Result<Vec<KnownLayoutEntry>, String> {
    let mut entries = load_raw(registry_path);
    entries.retain(|e| e.path != layout_path);
    write_registry(registry_path, &entries)?;
    Ok(load_known_layouts(registry_path))
}

// ── internals ────────────────────────────────────────────────────────────

/// Read the registry without applying the stale-path filter.
/// Used internally so add/remove can keep entries that point to a
/// momentarily-unavailable network drive.
fn load_raw(registry_path: &Path) -> Vec<KnownLayoutEntry> {
    let contents = match std::fs::read_to_string(registry_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

/// Atomic temp→flush→fsync→rename write. Same pattern used by other
/// app-data registries so they all fail the same way under power-loss /
/// sync-agent contention.
fn write_registry(
    registry_path: &Path,
    entries: &[KnownLayoutEntry],
) -> Result<(), String> {
    if let Some(parent) = registry_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create app data dir: {}", e))?;
    }

    let tmp_path = registry_path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| format!("Failed to serialise known layouts: {}", e))?;

    {
        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        file.flush()
            .map_err(|e| format!("Failed to flush temp file: {}", e))?;
        file.sync_all()
            .map_err(|e| format!("Failed to fsync temp file: {}", e))?;
    }

    std::fs::rename(&tmp_path, registry_path)
        .map_err(|e| format!("Failed to rename temp file to final: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, b"").unwrap();
    }

    fn entry(name: &str, path: &Path, last_opened: &str) -> KnownLayoutEntry {
        KnownLayoutEntry {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            last_opened: last_opened.to_string(),
        }
    }

    #[test]
    fn empty_when_file_missing() {
        let dir = fresh_dir("bowties_known_empty");
        let registry = dir.join("known-layouts.json");
        assert!(load_known_layouts(&registry).is_empty());
    }

    #[test]
    fn add_persists_and_round_trips() {
        let dir = fresh_dir("bowties_known_round_trip");
        let registry = dir.join("known-layouts.json");
        let layout = dir.join("home.layout");
        touch(&layout);

        let returned = add_known_layout(
            &registry,
            entry("Home", &layout, "2026-05-23T10:00:00Z"),
        )
        .unwrap();

        assert_eq!(returned.len(), 1);
        assert_eq!(returned[0].name, "Home");

        // Reading from a fresh call returns the same data.
        let read_back = load_known_layouts(&registry);
        assert_eq!(read_back, returned);
    }

    #[test]
    fn add_replaces_existing_entry_by_path() {
        let dir = fresh_dir("bowties_known_replace");
        let registry = dir.join("known-layouts.json");
        let layout = dir.join("home.layout");
        touch(&layout);

        add_known_layout(
            &registry,
            entry("Home (old)", &layout, "2026-01-01T00:00:00Z"),
        )
        .unwrap();
        let after = add_known_layout(
            &registry,
            entry("Home (new)", &layout, "2026-05-23T10:00:00Z"),
        )
        .unwrap();

        assert_eq!(after.len(), 1);
        assert_eq!(after[0].name, "Home (new)");
        assert_eq!(after[0].last_opened, "2026-05-23T10:00:00Z");
    }

    #[test]
    fn remove_only_drops_registry_entry() {
        let dir = fresh_dir("bowties_known_remove");
        let registry = dir.join("known-layouts.json");
        let layout = dir.join("home.layout");
        touch(&layout);

        add_known_layout(&registry, entry("Home", &layout, "2026-05-23T10:00:00Z"))
            .unwrap();
        let after = remove_known_layout(&registry, &layout.to_string_lossy()).unwrap();

        assert!(after.is_empty());
        // The layout file on disk is untouched.
        assert!(layout.exists(), "remove must not delete the layout file");
    }

    #[test]
    fn load_filters_stale_paths() {
        let dir = fresh_dir("bowties_known_stale");
        let registry = dir.join("known-layouts.json");
        let layout_a = dir.join("a.layout");
        let layout_b = dir.join("b.layout");
        touch(&layout_a);
        touch(&layout_b);

        add_known_layout(&registry, entry("A", &layout_a, "2026-05-01T00:00:00Z"))
            .unwrap();
        add_known_layout(&registry, entry("B", &layout_b, "2026-05-02T00:00:00Z"))
            .unwrap();

        std::fs::remove_file(&layout_b).unwrap();

        let visible = load_known_layouts(&registry);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "A");

        // Raw load still sees both — add/remove operate on the
        // unfiltered list so a momentarily-missing path is not
        // silently dropped on the next write.
        let raw = load_raw(&registry);
        assert_eq!(raw.len(), 2);
    }

    #[test]
    fn write_is_atomic_no_tmp_remains() {
        let dir = fresh_dir("bowties_known_atomic");
        let registry = dir.join("known-layouts.json");
        let layout = dir.join("home.layout");
        touch(&layout);

        add_known_layout(&registry, entry("Home", &layout, "2026-05-23T10:00:00Z"))
            .unwrap();

        let tmp = registry.with_extension("json.tmp");
        assert!(!tmp.exists(), "temp file must be renamed away");
        assert!(registry.exists(), "final file must exist");
    }

    #[test]
    fn add_creates_parent_directory() {
        let dir = fresh_dir("bowties_known_mkparent");
        // Registry sits inside a nested directory that does not yet exist.
        let registry = dir.join("nested").join("known-layouts.json");
        let layout = dir.join("home.layout");
        touch(&layout);

        let after = add_known_layout(
            &registry,
            entry("Home", &layout, "2026-05-23T10:00:00Z"),
        )
        .unwrap();
        assert_eq!(after.len(), 1);
        assert!(registry.exists());
    }
}
