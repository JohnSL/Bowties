//! Tauri LCC Configuration Tool
//! 
//! Backend implementation for the LCC visual configuration tool.

mod commands;
mod cdi;
mod menu;
mod state;
mod events;
mod traffic;
pub mod layout;
pub mod node_tree;
pub mod node_proxy;
pub mod node_registry;
pub mod profile;
pub mod diagnostics;

use menu::MenuHandles;

use lcc_rs::{LccConnection, NodeID, GridConnectSerialTransport, SlcanSerialTransport};
use state::AppState;
use commands::{ConnectionConfig, AdapterType};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

// Our Node ID (6 bytes)
const OUR_NODE_ID: [u8; 6] = [0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF];

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionInfo {
    connected: bool,
    config: Option<ConnectionConfig>,
}

/// Retry a fallible async port-open operation up to 3 times, with a 300 ms
/// delay between attempts, but only when the OS error looks like
/// "access denied" (port still claimed by a previous connection).
async fn open_with_retry<T, E, Fut, F>(mut f: F, label: &str) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut last_err = String::new();
    for attempt in 0..3u8 {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = format!("Failed to open {}: {}", label, e);
                let msg = e.to_string().to_lowercase();
                if attempt < 2 && (msg.contains("access") || msg.contains("denied")) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                } else {
                    break;
                }
            }
        }
    }
    Err(last_err)
}

/// Connect to an LCC network
#[tauri::command]
async fn connect_lcc(
    config: ConnectionConfig,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    // Disconnect existing connection if any
    state.disconnect().await;

    let node_id = NodeID::new(OUR_NODE_ID);

    let connection = match &config.adapter_type {
        AdapterType::Tcp => {
            let host = config.host.as_deref().unwrap_or("localhost");
            let port = config.port.unwrap_or(12021);
            LccConnection::connect_with_dispatcher(host, port, node_id)
                .await
                .map_err(|e| format!("Failed to connect via TCP: {}", e))?
        }
        AdapterType::GridConnectSerial => {
            let serial_port = config
                .serial_port
                .as_deref()
                .ok_or_else(|| "serial_port is required for GridConnect".to_string())?;
            let baud_rate = config.baud_rate.unwrap_or(57600);
            let transport = open_with_retry(
                || GridConnectSerialTransport::open(serial_port, baud_rate),
                "GridConnect serial port",
            )
            .await?;
            LccConnection::connect_with_dispatcher_and_transport(
                Box::new(transport),
                node_id,
            )
            .await
            .map_err(|e| format!("Failed to connect via GridConnect serial: {}", e))?
        }
        AdapterType::SlcanSerial => {
            let serial_port = config
                .serial_port
                .as_deref()
                .ok_or_else(|| "serial_port is required for SLCAN".to_string())?;
            let baud_rate = config.baud_rate.unwrap_or(115200);
            let transport = open_with_retry(
                || SlcanSerialTransport::open(serial_port, baud_rate),
                "SLCAN serial port",
            )
            .await?;
            LccConnection::connect_with_dispatcher_and_transport(
                Box::new(transport),
                node_id,
            )
            .await
            .map_err(|e| format!("Failed to connect via SLCAN serial: {}", e))?
        }
    };

    *state.active_connection.write().await = Some(config.clone());
    state.set_connection_with_dispatcher(connection, app).await;

    // Log connection event.
    let adapter_label = match &config.adapter_type {
        AdapterType::Tcp => {
            let host = config.host.as_deref().unwrap_or("localhost");
            let port = config.port.unwrap_or(12021);
            format!("{}:{}", host, port)
        }
        AdapterType::GridConnectSerial | AdapterType::SlcanSerial => {
            config.serial_port.clone().unwrap_or_default()
        }
    };
    bwlog!(state.inner(), "Connected: adapter={:?} label={}", config.adapter_type, adapter_label);
    {
        let mut stats = state.diag_stats.write().await;
        stats.connected_at = Some(chrono::Utc::now());
        stats.adapter_type = Some(format!("{:?}", config.adapter_type));
        stats.connection_label = Some(adapter_label);
        stats.discovery.initial_probe_at = Some(chrono::Utc::now());
    }

    // Phase 1: For TCP connections, fire a second probe at T+2 s to pick up nodes
    // that were not yet visible when the initial probe fired (common behind JMRI hubs).
    if config.adapter_type == AdapterType::Tcp {
        let state_clone = state.inner().clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            let conn_opt = state_clone.connection.read().await.clone();
            if let Some(conn_arc) = conn_opt {
                let node_count = state_clone.node_registry.len().await;
                bwlog!(state_clone, "TCP second probe fired at T+2s ({} nodes visible before probe)", node_count);
                {
                    let mut stats = state_clone.diag_stats.write().await;
                    stats.discovery.second_probe_at = Some(chrono::Utc::now());
                    stats.discovery.second_probe_node_count = Some(node_count);
                }
                let mut conn = conn_arc.lock().await;
                let _ = conn.probe_nodes().await;
            }
        });
    }

    Ok(ConnectionInfo {
        connected: true,
        config: Some(config),
    })
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
    can_view_cdi: bool,
    can_redownload_cdi: bool,
    can_open_layout: bool,
    can_save_layout: bool,
    can_save_layout_as: bool,
    handles: tauri::State<'_, MenuHandles>,
) -> Result<(), String> {
    handles.disconnect     .set_enabled(connected)                      .map_err(|e| e.to_string())?;
    handles.refresh_nodes  .set_enabled(connected && !is_busy)          .map_err(|e| e.to_string())?;
    handles.traffic_monitor.set_enabled(connected)                      .map_err(|e| e.to_string())?;
    handles.diagnostics    .set_enabled(connected)                      .map_err(|e| e.to_string())?;
    handles.view_cdi       .set_enabled(can_view_cdi)                   .map_err(|e| e.to_string())?;
    handles.redownload_cdi .set_enabled(can_redownload_cdi)             .map_err(|e| e.to_string())?;
    handles.open_layout    .set_enabled(can_open_layout)                .map_err(|e| e.to_string())?;
    handles.save_layout    .set_enabled(can_save_layout)                .map_err(|e| e.to_string())?;
    handles.save_layout_as .set_enabled(can_save_layout_as)             .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get current connection status
#[tauri::command]
async fn get_connection_status(
    state: tauri::State<'_, AppState>,
) -> Result<ConnectionInfo, String> {
    Ok(ConnectionInfo {
        connected: state.is_connected().await,
        config: state.active_connection.read().await.clone(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
                    "menu-open-layout"    => { let _ = app_h.emit("menu-open-layout", ()); }
                    "menu-save-layout"    => { let _ = app_h.emit("menu-save-layout", ()); }
                    "menu-save-layout-as" => { let _ = app_h.emit("menu-save-layout-as", ()); }
                    "menu-exit"           => { let _ = app_h.emit("menu-exit", ()); }
                    "menu-diagnostics"    => { let _ = app_h.emit("menu-diagnostics", ()); }
                    _ => {}
                }
            });

            // Set window icon and show the window after state restoration
            if let Some(window) = app.get_webview_window("main") {
                // Load the icon from the bundled PNG
                let icon_bytes = include_bytes!("../icons/icon.png");
                if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
                    let _ = window.set_icon(icon);
                }
                window.show().unwrap();

                // Disconnect gracefully before the window (and Tokio runtime) tears
                // down so the LCC hub receives a FIN rather than a RST.
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let state = app_handle.state::<AppState>().inner().clone();
                        let app_h = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            state.disconnect().await;
                            if let Some(traffic) = app_h.get_webview_window("traffic") {
                                let _ = traffic.close();
                            }
                            app_h.exit(0);
                        });
                    }
                });
            }

            Ok(())
        })
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            connect_lcc,
            disconnect_lcc,
            get_connection_status,
            commands::discover_nodes,
            commands::probe_nodes,
            commands::register_node,
            commands::query_snip_single,
            commands::query_snip_batch,
            commands::query_pip_single,
            commands::query_pip_batch,
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
            commands::cancel_cdi_download,
            commands::get_card_elements,
            commands::get_bowties,  // T011: Feature 006 bowtie catalog
            commands::build_bowtie_catalog_command,  // Feature 009: rebuild with layout merge
            commands::load_layout,  // Feature 009: layout file persistence
            commands::save_layout,  // Feature 009: layout file persistence
            commands::get_recent_layout,  // Feature 009: recent layout tracking
            commands::set_recent_layout,  // Feature 009: recent layout tracking
            commands::capture_layout_snapshot,  // Spec 010: offline layout capture scaffolding
            commands::save_layout_directory,  // Spec 010: directory persistence scaffolding
            commands::open_layout_directory,  // Spec 010: offline open scaffolding
            commands::close_layout,  // Spec 010: layout context lifecycle scaffolding
            commands::create_new_layout_capture,  // Spec 010: new capture lifecycle scaffolding
            commands::set_offline_change,  // Spec 010: offline change scaffolding
            commands::revert_offline_change,  // Spec 010: offline change scaffolding
            commands::list_offline_changes,  // Spec 010: offline change scaffolding
            commands::compute_layout_match_status,  // Spec 010: sync match scaffolding
            commands::build_sync_session,  // Spec 010: sync session scaffolding
            commands::set_sync_mode,  // Spec 010: sync mode scaffolding
            commands::apply_sync_changes,  // Spec 010: sync apply scaffolding
            cdi::bundle::export_cdi_bundle,  // Spec 010: CDI portability scaffolding
            cdi::bundle::import_cdi_bundle,  // Spec 010: CDI portability scaffolding
            commands::get_node_tree,  // Spec 007: unified node tree
            commands::write_config_value,  // Spec 007: write config value
            commands::send_update_complete,  // Spec 007: send update complete
            commands::set_modified_value,  // Modified value: set pending edit on tree
            commands::discard_modified_values,  // Modified value: discard pending edits
            commands::write_modified_values,  // Modified value: write all pending edits
            commands::has_modified_values,  // Modified value: check for pending edits
            commands::trigger_action,         // Action element: fire-once write
            commands::list_serial_ports,
            commands::load_connection_prefs,
            commands::save_connection_prefs,
            diagnostics::get_diagnostic_report,
            update_menu_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
