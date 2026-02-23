//! Tauri LCC Configuration Tool
//! 
//! Backend implementation for the LCC visual configuration tool.

mod commands;
mod menu;
mod state;
mod events;
mod traffic;

use menu::MenuHandles;

use lcc_rs::LccConnection;
use state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    // Disconnect existing connection if any
    state.disconnect().await;
    
    // Create new connection with dispatcher
    match LccConnection::connect_with_dispatcher(&host, port).await {
        Ok(connection) => {
            *state.host.write().await = host.clone();
            *state.port.write().await = port;
            state.set_connection_with_dispatcher(connection, app).await;
            
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
    state.disconnect().await;
    Ok(())
}

/// Update native menu item enabled states.
///
/// Called from the frontend whenever connection state, busy state, or node
/// selection changes — there is no "menu will open" event in Tauri v2, so
/// we push state eagerly at each well-defined change point.
#[tauri::command]
async fn update_menu_state(
    connected: bool,
    is_busy: bool,
    has_selection: bool,
    handles: tauri::State<'_, MenuHandles>,
) -> Result<(), String> {
    handles.disconnect     .set_enabled(connected)                      .map_err(|e| e.to_string())?;
    handles.refresh_nodes  .set_enabled(connected && !is_busy)          .map_err(|e| e.to_string())?;
    handles.traffic_monitor.set_enabled(connected)                      .map_err(|e| e.to_string())?;
    handles.view_cdi       .set_enabled(connected && has_selection)     .map_err(|e| e.to_string())?;
    handles.redownload_cdi .set_enabled(connected && has_selection)     .map_err(|e| e.to_string())?;
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
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            // Build and set the native menu
            let (app_menu, menu_handles) = menu::build_app_menu(app.handle())?;
            app.set_menu(app_menu)?;
            app.manage(menu_handles);

            // Forward native menu clicks to the frontend as Tauri events
            app.handle().on_menu_event(|app_h, event| {
                match event.id().as_ref() {
                    "menu-disconnect"     => { let _ = app_h.emit("menu-disconnect", ()); }
                    "menu-refresh"        => { let _ = app_h.emit("menu-refresh", ()); }
                    "menu-traffic"        => { let _ = app_h.emit("menu-traffic", ()); }
                    "menu-view-cdi"       => { let _ = app_h.emit("menu-view-cdi", ()); }
                    "menu-redownload-cdi" => { let _ = app_h.emit("menu-redownload-cdi", ()); }
                    "menu-discovery-opts" => { let _ = app_h.emit("menu-discovery-opts", ()); }
                    _ => {}
                }
            });

            // Show the window after state restoration to prevent flickering
            if let Some(window) = app.get_webview_window("main") {
                window.show().unwrap();
            }
            Ok(())
        })
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
            commands::get_discovered_nodes,
            commands::get_cdi_structure,
            commands::get_column_items,
            commands::get_element_details,
            commands::expand_replicated_group,
            commands::read_config_value,  // T034: Register read_config_value command
            commands::read_all_config_values,  // T055: Register read_all_config_values command
            commands::cancel_config_reading,
            commands::get_card_elements,
            commands::get_segment_elements,
            update_menu_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
