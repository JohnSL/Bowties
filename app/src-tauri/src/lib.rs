//! Tauri LCC Configuration Tool
//! 
//! Backend implementation for the LCC visual configuration tool.

mod commands;
mod state;

use lcc_rs::LccConnection;
use state::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionInfo {
    host: String,
    port: u16,
    connected: bool,
}

/// Connect to an LCC network
#[tauri::command]
async fn connect_lcc(
    host: String,
    port: u16,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    // Close existing connection if any
    state.set_connection(None).await;
    
    // Create new connection
    match LccConnection::connect(&host, port).await {
        Ok(connection) => {
            *state.host.write().await = host.clone();
            *state.port.write().await = port;
            state.set_connection(Some(connection)).await;
            
            Ok(ConnectionInfo {
                host,
                port,
                connected: true,
            })
        }
        Err(e) => Err(format!("Failed to connect: {}", e)),
    }
}

/// Disconnect from the LCC network
#[tauri::command]
async fn disconnect_lcc(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.set_connection(None).await;
    Ok(())
}

/// Get current connection status
#[tauri::command]
async fn get_connection_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    Ok(ConnectionInfo {
        host: state.host.read().await.clone(),
        port: *state.port.read().await,
        connected: state.is_connected().await,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            connect_lcc,
            disconnect_lcc,
            get_connection_status,
            commands::discover_nodes,
            commands::query_snip_single,
            commands::query_snip_batch,
            commands::verify_node_status,
            commands::refresh_all_nodes,
            commands::get_cdi_xml,
            commands::download_cdi,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
