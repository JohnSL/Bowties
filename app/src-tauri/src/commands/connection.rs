//! Connection configuration commands
//!
//! Manages saved connection profiles (TCP, GridConnect serial, SLCAN serial)
//! and serial port enumeration.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri::Manager;
use std::io::Write;

// ── Shared types ──────────────────────────────────────────────────────────────

/// The transport/protocol variant for a connection.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AdapterType {
    /// Network hub (JMRI, standalone TCP/IP bridge). Default LCC port 12021.
    Tcp,
    /// GridConnect framing over USB serial.
    /// Compatible: RR-Cirkits Buffer LCC, CAN2USBINO, CANRS.
    GridConnectSerial,
    /// SLCAN (Lawicel) framing over USB serial.
    /// Compatible: Canable, Lawicel CANUSB, any slcand-compatible adapter.
    SlcanSerial,
}

/// A saved connection configuration entry.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    /// Unique identifier (UUID v4)
    pub id: String,
    /// User-visible label for this connection
    pub name: String,
    /// Protocol / adapter type
    pub adapter_type: AdapterType,
    /// TCP hostname or IP (TCP only)
    pub host: Option<String>,
    /// TCP port number (TCP only, default 12021)
    pub port: Option<u16>,
    /// Serial port path, e.g. "COM3" or "/dev/ttyUSB0" (serial only)
    pub serial_port: Option<String>,
    /// Serial baud rate (serial only; USB CDC devices use this for host-side
    /// configuration, though the adapters themselves typically ignore it)
    pub baud_rate: Option<u32>,
}

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

/// Load saved connection preferences from `$APPDATA/bowties/connections.json`.
///
/// Returns an empty Vec if the file does not exist yet.
#[tauri::command]
pub fn load_connection_prefs(app: AppHandle) -> Vec<ConnectionConfig> {
    let path = match app.path().app_data_dir() {
        Ok(dir) => dir.join("connections.json"),
        Err(_) => return vec![],
    };

    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    serde_json::from_str(&contents).unwrap_or_default()
}

/// Save connection preferences to `$APPDATA/bowties/connections.json` atomically.
///
/// Writes to a temporary file and renames it so the file is never partially written.
#[tauri::command]
pub fn save_connection_prefs(
    app: AppHandle,
    connections: Vec<ConnectionConfig>,
) -> Result<(), String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    let final_path = dir.join("connections.json");
    let tmp_path = dir.join("connections.json.tmp");

    let json = serde_json::to_string_pretty(&connections)
        .map_err(|e| format!("Failed to serialise connections: {}", e))?;

    {
        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        file.flush()
            .map_err(|e| format!("Failed to flush temp file: {}", e))?;
    }

    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| format!("Failed to rename temp file to final: {}", e))?;

    Ok(())
}
