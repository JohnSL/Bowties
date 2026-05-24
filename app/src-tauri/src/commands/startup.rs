//! Startup / known-layout registry commands (Spec 013 / S5).
//!
//! Thin Tauri wrappers over [`crate::layout::known_layouts`]. The
//! domain module owns the on-disk format, the stale-path filter,
//! and the atomic write protocol; these commands only resolve the
//! `$APPDATA/bowties/known-layouts.json` path and translate errors
//! to strings for the IPC boundary.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::layout::known_layouts::{
    self, KnownLayoutEntry,
};

/// Resolve `$APPDATA/bowties/known-layouts.json` (or the platform
/// equivalent). Errors here are surfaced verbatim so the picker can
/// fall back to an empty list without crashing.
fn registry_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    Ok(dir.join("known-layouts.json"))
}

/// Return the known-layout registry, with stale entries filtered.
#[tauri::command]
pub fn get_known_layouts(app: AppHandle) -> Result<Vec<KnownLayoutEntry>, String> {
    let path = registry_path(&app)?;
    Ok(known_layouts::load_known_layouts(&path))
}

/// Add or refresh a known-layout entry (matched by `path`).
/// Returns the post-write filtered registry.
#[tauri::command]
pub fn add_known_layout(
    app: AppHandle,
    entry: KnownLayoutEntry,
) -> Result<Vec<KnownLayoutEntry>, String> {
    let registry = registry_path(&app)?;
    known_layouts::add_known_layout(&registry, entry)
}

/// Remove a known-layout entry by `path`. Does not touch the
/// `.layout` file or its companion directory.
#[tauri::command]
pub fn remove_known_layout(
    app: AppHandle,
    path: String,
) -> Result<Vec<KnownLayoutEntry>, String> {
    let registry = registry_path(&app)?;
    known_layouts::remove_known_layout(&registry, &path)
}
