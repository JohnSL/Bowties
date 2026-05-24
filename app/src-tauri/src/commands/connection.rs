//! Connection configuration commands
//!
//! Per-layout connection profiles (TCP, GridConnect serial, SLCAN serial) and
//! serial port enumeration. The list of saved connections lives inside each
//! layout manifest (Spec 013 / S4) and is read/written via
//! `get_layout_connections` / `save_layout_connections`. There is no longer a
//! global `connections.json` registry (removed in Spec 013 / S7).

use std::path::Path;

// ── Shared types ──────────────────────────────────────────────────────────────

// `ConnectionConfig`, `AdapterType`, and `FlowControl` live in
// `crate::layout::types` because they are persisted inside layout
// manifests (Spec 013 / S4). Re-exported here so existing call sites that
// reference `commands::ConnectionConfig` continue to compile.
pub use crate::layout::types::{AdapterType, ConnectionConfig, FlowControl};

// ── Commands ─────────────────────────────────────────────────────────────────

/// List available serial port names on the host.
///
/// Returns port names suitable for display in a `<select>` dropdown.
/// Call again when the user clicks the Refresh button to detect hotplug changes.
#[tauri::command]
pub fn list_serial_ports() -> Result<Vec<String>, String> {
    let ports = serialport::available_ports()
        .map_err(|e| format!("Failed to list serial ports: {}", e))?;
    Ok(ports.into_iter().map(|p| p.port_name).collect())
}

// ── Per-layout connection commands (Spec 013 / S4) ──────────────────────────

/// Read the saved connections list from a layout manifest file.
///
/// Returns an empty list when the manifest has no `connections` field
/// (older layouts predating Spec 013 / S4).
#[tauri::command]
pub fn get_layout_connections(path: String) -> Result<Vec<ConnectionConfig>, String> {
    let manifest = crate::layout::read_manifest(Path::new(&path))?;
    Ok(manifest.connections)
}

/// Replace the saved connections list on a layout manifest file.
///
/// Writes the manifest in place through the layout journal so an
/// interrupted save is recoverable on the next read (ADR-0006).
#[tauri::command]
pub fn save_layout_connections(
    path: String,
    connections: Vec<ConnectionConfig>,
) -> Result<(), String> {
    crate::layout::update_manifest_connections(Path::new(&path), connections)
}
