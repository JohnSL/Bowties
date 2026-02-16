//! Tauri LCC Configuration Tool
//! 
//! Backend implementation for the LCC visual configuration tool.

use lcc_rs::{LccConnection, DiscoveredNode};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

/// Application state managing the LCC connection
struct AppState {
    connection: Option<Arc<Mutex<LccConnection>>>,
    host: String,
    port: u16,
}

impl AppState {
    fn new() -> Self {
        Self {
            connection: None,
            host: "localhost".to_string(),
            port: 12021,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionInfo {
    host: String,
    port: u16,
    connected: bool,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

/// Connect to an LCC network
#[tauri::command]
async fn connect_lcc(
    host: String,
    port: u16,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<ConnectionInfo, String> {
    let mut app_state = state.lock().await;
    
    // Close existing connection if any
    if let Some(conn) = app_state.connection.take() {
        // Connection will be dropped and closed
        drop(conn);
    }
    
    // Create new connection
    match LccConnection::connect(&host, port).await {
        Ok(connection) => {
            app_state.connection = Some(Arc::new(Mutex::new(connection)));
            app_state.host = host.clone();
            app_state.port = port;
            
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
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let mut app_state = state.lock().await;
    
    if let Some(conn) = app_state.connection.take() {
        drop(conn);
    }
    
    Ok(())
}

/// Get current connection status
#[tauri::command]
async fn get_connection_status(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<ConnectionInfo, String> {
    let app_state = state.lock().await;
    
    Ok(ConnectionInfo {
        host: app_state.host.clone(),
        port: app_state.port,
        connected: app_state.connection.is_some(),
    })
}

/// Discover nodes on the LCC network
#[tauri::command]
async fn discover_nodes(
    timeout_ms: Option<u64>,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<DiscoveredNode>, String> {
    let app_state = state.lock().await;
    
    match &app_state.connection {
        Some(conn) => {
            let mut connection = conn.lock().await;
            let timeout = timeout_ms.unwrap_or(250);
            
            connection
                .discover_nodes(timeout)
                .await
                .map_err(|e| format!("Discovery failed: {}", e))
        }
        None => Err("Not connected to LCC network".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Mutex::new(AppState::new())))
        .invoke_handler(tauri::generate_handler![
            connect_lcc,
            disconnect_lcc,
            get_connection_status,
            discover_nodes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
