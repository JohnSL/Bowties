//! Node discovery and SNIP query commands

use crate::state::AppState;
use lcc_rs::{ConnectionStatus, DiscoveredNode, MTI, NodeAlias, NodeID, PIPStatus, ProtocolFlags, SNIPData, SNIPStatus};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Response from a SNIP query
#[derive(Debug, Serialize)]
pub struct QuerySnipResponse {
    pub alias: u16,
    pub snip_data: Option<SNIPData>,
    pub status: SNIPStatus,
}

/// Discover all nodes on the LCC network
#[tauri::command]
pub async fn discover_nodes(
    timeout_ms: Option<u64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DiscoveredNode>, String> {
    let timeout = timeout_ms.unwrap_or(250);
    
    // Get connection reference
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };
    
    // Lock and perform discovery
    let mut connection = connection_arc.lock().await;
    let result = connection.discover_nodes(timeout).await;
    drop(connection); // Release lock
    
    // Process result
    let nodes = result.map_err(|e| format!("Discovery failed: {}", e))?;
    state.set_nodes(nodes.clone()).await;
    
    Ok(nodes)
}

/// Fire a `VerifyNodeGlobal` probe and return immediately.
///
/// All `VerifiedNode` replies are forwarded to the frontend as `lcc-node-discovered`
/// Tauri events by the persistent `EventRouter`. Subscribe to that event before
/// calling this command so no replies are missed.
#[tauri::command]
pub async fn probe_nodes(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    eprintln!("[probe_nodes] command invoked");
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };
    let mut connection = connection_arc.lock().await;
    connection.probe_nodes().await.map_err(|e| format!("Probe failed: {}", e))
}

/// Register a newly appeared node in the backend state cache.
///
/// Called by the frontend when it receives a `lcc-node-discovered` event so that
/// subsequent commands (SNIP update caching, CDI, bowtie catalog) can find the
/// node by alias or node-ID.  Does nothing if the node is already registered.
#[tauri::command]
pub async fn register_node(
    node_id_hex: String,
    alias: u16,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let node_alias = NodeAlias::new(alias).map_err(|e| format!("Invalid alias: {}", e))?;

    let bytes: Vec<u8> = node_id_hex
        .split('.')
        .map(|s| u8::from_str_radix(s, 16).map_err(|e| format!("Bad node ID byte '{}': {}", s, e)))
        .collect::<Result<Vec<_>, _>>()?;
    if bytes.len() != 6 {
        return Err(format!("Expected 6 bytes in node ID, got {}", bytes.len()));
    }
    let node_id = NodeID::from_slice(&bytes).map_err(|e| e.to_string())?;

    state.add_node(DiscoveredNode {
        node_id,
        alias: node_alias,
        snip_data: None,
        snip_status: SNIPStatus::Unknown,
        connection_status: ConnectionStatus::Connected,
        last_verified: None,
        last_seen: chrono::Utc::now(),
        cdi: None,
        pip_flags: None,
        pip_status: PIPStatus::Unknown,
    }).await;

    Ok(())
}

/// Query SNIP data for a single node
#[tauri::command]
pub async fn query_snip_single(
    alias: u16,
    state: tauri::State<'_, AppState>,
) -> Result<QuerySnipResponse, String> {
    // Validate alias
    let _node_alias = NodeAlias::new(alias).map_err(|e| format!("Invalid alias: {}", e))?;

    // Look up node ID for logging before we query
    let cached_node_id = state.get_nodes().await.iter()
        .find(|n| n.alias.value() == alias)
        .map(|n| n.node_id);
    if let Some(node_id) = cached_node_id {
        eprintln!("[SNIP] alias=0x{:03X}  node_id={}", alias, node_id);
    }

    // Get connection reference
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };
    
    // Lock and query SNIP
    let mut connection = connection_arc.lock().await;
    let (snip_data, status) = connection
        .query_snip(alias, None)
        .await
        .map_err(|e| format!("SNIP query failed: {}", e))?;
    drop(connection);
    
    // Update node in cache if it exists
    if let Some(node_id) = cached_node_id {
        state.update_node(node_id, |node| {
            node.snip_data = snip_data.clone();
            node.snip_status = status;
        }).await;
    }
    
    Ok(QuerySnipResponse {
        alias,
        snip_data,
        status,
    })
}

/// Query SNIP data for multiple nodes concurrently
#[tauri::command]
pub async fn query_snip_batch(
    aliases: Vec<u16>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QuerySnipResponse>, String> {
    if aliases.is_empty() {
        return Ok(vec![]);
    }
    
    // Validate all aliases
    for &alias in &aliases {
        NodeAlias::new(alias).map_err(|e| format!("Invalid alias {}: {}", alias, e))?;
    }
    
    // Get connection reference
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };
    
    // Create shared semaphore for concurrency limiting (max 5 concurrent)
    let semaphore = Arc::new(Semaphore::new(5));
    
    // Query SNIP for each node (sequential for now due to mutable borrow)
    // TODO: Refactor to support true concurrency with Arc<Mutex<Transport>>
    let mut results = Vec::new();
    
    for alias in aliases {
        let mut connection = connection_arc.lock().await;
        let (snip_data, status) = connection
            .query_snip(alias, Some(semaphore.clone()))
            .await
            .unwrap_or((None, SNIPStatus::Error));
        drop(connection);
        
        results.push(QuerySnipResponse {
            alias,
            snip_data,
            status,
        });
    }
    
    // Update nodes in cache
    for response in &results {
        if let Some(node_id) = state.get_nodes().await.iter()
            .find(|n| n.alias.value() == response.alias)
            .map(|n| n.node_id)
        {
            state.update_node(node_id, |node| {
                node.snip_data = response.snip_data.clone();
                node.snip_status = response.status;
            }).await;
        }
    }
    
    Ok(results)
}

/// Verify the status of a single node
#[tauri::command]
pub async fn verify_node_status(
    alias: u16,
    timeout_ms: Option<u64>,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let timeout = timeout_ms.unwrap_or(500);
    
    // Validate alias
    let _node_alias = NodeAlias::new(alias).map_err(|e| format!("Invalid alias: {}", e))?;
    
    // Get connection reference
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };
    
    // Lock and verify the node
    let mut connection = connection_arc.lock().await;
    let node_id_opt = connection
        .verify_node(alias, timeout)
        .await
        .map_err(|e| format!("Verification failed: {}", e))?;
    drop(connection);
    
    // Update node status in cache
    let is_online = node_id_opt.is_some();
    
    if let Some(node_id) = node_id_opt {
        state.update_node(node_id, |node| {
            node.connection_status = lcc_rs::types::ConnectionStatus::Connected;
            node.last_verified = Some(chrono::Utc::now());
            node.last_seen = chrono::Utc::now();
        }).await;
    } else {
        // Find node by alias and mark as not responding
        if let Some(node_id) = state.get_nodes().await.iter()
            .find(|n| n.alias.value() == alias)
            .map(|n| n.node_id)
        {
            state.update_node(node_id, |node| {
                node.connection_status = lcc_rs::types::ConnectionStatus::NotResponding;
            }).await;
        }
    }
    
    Ok(is_online)
}

/// Re-probe the network and return the dotted-hex Node IDs of nodes that did not respond.
///
/// Sends a `VerifyNodeGlobal` frame, then waits for a 500 ms liveness window while
/// collecting all `VerifiedNode` replies via the dispatcher.  Known nodes that do not
/// reply within the window are removed from the state cache and their IDs are returned
/// so the frontend can remove them from the UI.
///
/// Active nodes that **do** respond also trigger `lcc-node-discovered` events (via the
/// persistent `EventRouter`), so any genuinely new nodes appear in the UI automatically.
#[tauri::command]
pub async fn refresh_all_nodes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };

    let expected_nodes = state.get_nodes().await;

    // If no known nodes, just probe and let node-appeared events handle everything.
    if expected_nodes.is_empty() {
        let mut conn = connection_arc.lock().await;
        conn.probe_nodes().await.map_err(|e| format!("Probe failed: {}", e))?;
        return Ok(vec![]);
    }

    // Subscribe to VerifiedNode replies *before* sending the probe so we don't
    // miss fast responders.
    let mut rx = {
        let conn = connection_arc.lock().await;
        conn.dispatcher().map(|disp| {
            // We need to unlock dispatcher to get the channel; capture it first.
            disp
        })
    };
    let mut maybe_rx = match rx.take() {
        Some(disp_arc) => {
            let disp = disp_arc.lock().await;
            Some(disp.subscribe_mti(MTI::VerifiedNode).await)
        }
        None => None,
    };

    // Send the probe.
    {
        let mut conn = connection_arc.lock().await;
        conn.probe_nodes().await.map_err(|e| format!("Probe failed: {}", e))?;
    }

    // Collect respondents during the liveness window (500 ms, no early exit).
    const LIVENESS_MS: u64 = 500;
    let mut responded: std::collections::HashSet<NodeID> = std::collections::HashSet::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(LIVENESS_MS);

    if let Some(ref mut recv) = maybe_rx {
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() { break; }
            match tokio::time::timeout(remaining, recv.recv()).await {
                Ok(Ok(msg)) => {
                    if msg.frame.data.len() == 6 {
                        if let Ok(node_id) = NodeID::from_slice(&msg.frame.data) {
                            responded.insert(node_id);
                        }
                    }
                }
                Ok(Err(_)) => continue, // broadcast lagged
                Err(_) => break,        // timeout elapsed
            }
        }
    } else {
        // No dispatcher (direct-transport mode) — just wait.
        tokio::time::sleep(std::time::Duration::from_millis(LIVENESS_MS)).await;
    }

    // Determine which previously-known nodes did not respond.
    let stale: Vec<NodeID> = expected_nodes
        .iter()
        .filter(|n| !responded.contains(&n.node_id))
        .map(|n| n.node_id)
        .collect();

    // Remove stale nodes from the backend cache.
    if !stale.is_empty() {
        let mut nodes_guard = state.nodes.write().await;
        nodes_guard.retain(|n| !stale.contains(&n.node_id));
    }

    // Return stale node IDs as dotted-hex strings for the frontend to cull its list.
    let stale_strings: Vec<String> = stale
        .iter()
        .map(|id| {
            let b = id.as_bytes();
            format!(
                "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
                b[0], b[1], b[2], b[3], b[4], b[5]
            )
        })
        .collect();

    Ok(stale_strings)
}

// ── Protocol Identification Protocol (PIP) ────────────────────────────────────

/// Response from a PIP query
#[derive(Debug, Serialize)]
pub struct QueryPipResponse {
    pub alias: u16,
    pub pip_flags: Option<ProtocolFlags>,
    pub status: PIPStatus,
}

/// Query Protocol Identification Protocol data for a single node
#[tauri::command]
pub async fn query_pip_single(
    alias: u16,
    state: tauri::State<'_, AppState>,
) -> Result<QueryPipResponse, String> {
    let _node_alias = NodeAlias::new(alias).map_err(|e| format!("Invalid alias: {}", e))?;

    // Look up the node ID for logging before we query (pip.rs logs alias-only)
    let cached_node_id = state.get_nodes().await.iter()
        .find(|n| n.alias.value() == alias)
        .map(|n| n.node_id);
    if let Some(node_id) = cached_node_id {
        eprintln!("[PIP] alias=0x{:03X}  node_id={}", alias, node_id);
    }

    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };

    let mut connection = connection_arc.lock().await;
    let (pip_flags, status) = connection
        .query_pip(alias, None)
        .await
        .map_err(|e| format!("PIP query failed: {}", e))?;
    drop(connection);

    // Update node in cache if it exists
    if let Some(node_id) = cached_node_id {
        state.update_node(node_id, |node| {
            node.pip_flags = pip_flags;
            node.pip_status = status;
        }).await;
    }

    Ok(QueryPipResponse { alias, pip_flags, status })
}

/// Query Protocol Identification Protocol data for multiple nodes
#[tauri::command]
pub async fn query_pip_batch(
    aliases: Vec<u16>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QueryPipResponse>, String> {
    if aliases.is_empty() {
        return Ok(vec![]);
    }

    for &alias in &aliases {
        NodeAlias::new(alias).map_err(|e| format!("Invalid alias {}: {}", alias, e))?;
    }

    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };

    let semaphore = Arc::new(Semaphore::new(5));
    let mut results = Vec::new();

    for alias in aliases {
        let mut connection = connection_arc.lock().await;
        let (pip_flags, status) = connection
            .query_pip(alias, Some(semaphore.clone()))
            .await
            .unwrap_or((None, PIPStatus::Error));
        drop(connection);

        results.push(QueryPipResponse { alias, pip_flags, status });
    }

    // Update nodes in cache
    for response in &results {
        if let Some(node_id) = state.get_nodes().await.iter()
            .find(|n| n.alias.value() == response.alias)
            .map(|n| n.node_id)
        {
            state.update_node(node_id, |node| {
                node.pip_flags = response.pip_flags;
                node.pip_status = response.status;
            }).await;
        }
    }

    Ok(results)
}
