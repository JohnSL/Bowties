//! Node discovery and SNIP query commands

use crate::state::AppState;
use lcc_rs::{ConnectionStatus, DiscoveredNode, MTI, NodeAlias, NodeID, PIPStatus, ProtocolFlags, SNIPData, SNIPStatus};
use serde::Serialize;

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
    let node_count = nodes.len();

    // Register a proxy for each discovered node
    for node in &nodes {
        let _ = state.node_registry.get_or_create(node.node_id, node.alias.value()).await;
    }

    crate::bwlog!(state.inner(), "[discovery] initial probe complete: {} node(s) found", node_count);
    {
        let mut stats = state.diag_stats.write().await;
        stats.discovery.initial_probe_node_count = node_count;
    }
    
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
    let _node_alias = NodeAlias::new(alias).map_err(|e| format!("Invalid alias: {}", e))?;

    let bytes: Vec<u8> = node_id_hex
        .split('.')
        .map(|s| u8::from_str_radix(s, 16).map_err(|e| format!("Bad node ID byte '{}': {}", s, e)))
        .collect::<Result<Vec<_>, _>>()?;
    if bytes.len() != 6 {
        return Err(format!("Expected 6 bytes in node ID, got {}", bytes.len()));
    }
    let node_id = NodeID::from_slice(&bytes).map_err(|e| e.to_string())?;

    // Create proxy in the registry (or get existing)
    let _ = state.node_registry.get_or_create(node_id, alias).await?;

    Ok(())
}

/// Query SNIP data for a single node
#[tauri::command]
pub async fn query_snip_single(
    alias: u16,
    state: tauri::State<'_, AppState>,
) -> Result<QuerySnipResponse, String> {
    let _node_alias = NodeAlias::new(alias).map_err(|e| format!("Invalid alias: {}", e))?;

    let proxy = state.node_registry.get_by_alias(alias).await
        .ok_or_else(|| format!("No node registered with alias 0x{:03X}", alias))?;

    eprintln!("[SNIP] alias=0x{:03X}  node_id={}", alias, proxy.node_id);

    let (snip_data, status) = proxy.query_snip().await?;

    Ok(QuerySnipResponse {
        alias,
        snip_data,
        status,
    })
}

/// Query SNIP data for multiple nodes concurrently via per-node proxies.
///
/// Each proxy independently handles its own SNIP query using the `TransportHandle`
/// (no connection mutex), so all nodes are queried in parallel.
#[tauri::command]
pub async fn query_snip_batch(
    aliases: Vec<u16>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QuerySnipResponse>, String> {
    if aliases.is_empty() {
        return Ok(vec![]);
    }

    for &alias in &aliases {
        NodeAlias::new(alias).map_err(|e| format!("Invalid alias {}: {}", alias, e))?;
    }

    // Collect proxy handles for all aliases
    let mut proxy_aliases = Vec::with_capacity(aliases.len());
    for &alias in &aliases {
        if let Some(proxy) = state.node_registry.get_by_alias(alias).await {
            proxy_aliases.push((alias, proxy));
        }
    }

    // Query all in parallel using JoinSet
    let mut join_set = tokio::task::JoinSet::new();
    for (alias, proxy) in proxy_aliases {
        let proxy = proxy.clone();
        join_set.spawn(async move {
            let result = proxy.query_snip().await;
            (alias, proxy.node_id, result)
        });
    }

    let mut results = Vec::new();
    while let Some(join_result) = join_set.join_next().await {
        let (alias, _node_id, result) = join_result.map_err(|e| e.to_string())?;
        let (snip_data, status) = result.unwrap_or((None, SNIPStatus::Error));

        results.push(QuerySnipResponse {
            alias,
            snip_data,
            status,
        });
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
    
    // Update node status via proxy
    let is_online = node_id_opt.is_some();
    
    if let Some(node_id) = node_id_opt {
        if let Some(proxy) = state.node_registry.get(&node_id).await {
            let _ = proxy.update_connection_status(
                ConnectionStatus::Connected,
                Some(chrono::Utc::now()),
            ).await;
        }
    } else {
        // Find node by alias and mark as not responding
        if let Some(proxy) = state.node_registry.get_by_alias(alias).await {
            let _ = proxy.update_connection_status(
                ConnectionStatus::NotResponding,
                None,
            ).await;
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

    let expected_nodes = state.node_registry.get_all_snapshots().await;

    // If no known nodes, just probe and let node-appeared events handle everything.
    if expected_nodes.is_empty() {
        let mut conn = connection_arc.lock().await;
        conn.probe_nodes().await.map_err(|e| format!("Probe failed: {}", e))?;
        return Ok(vec![]);
    }

    // Subscribe to VerifiedNode replies *before* sending the probe so we don't
    // miss fast responders.
    let maybe_rx = {
        let conn = connection_arc.lock().await;
        if let Some(handle) = conn.transport_handle() {
            Some(handle.subscribe_mti(MTI::VerifiedNode).await)
        } else {
            None
        }
    };
    let mut maybe_rx = maybe_rx;

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
        // No transport handle — just wait.
        tokio::time::sleep(std::time::Duration::from_millis(LIVENESS_MS)).await;
    }

    // Determine which previously-known nodes did not respond.
    let stale: Vec<NodeID> = expected_nodes
        .iter()
        .filter(|n| !responded.contains(&n.node_id))
        .map(|n| n.node_id)
        .collect();

    // Remove stale nodes from the registry.
    if !stale.is_empty() {
        for id in &stale {
            state.node_registry.remove(id).await;
        }
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

    let proxy = state.node_registry.get_by_alias(alias).await
        .ok_or_else(|| format!("No node registered with alias 0x{:03X}", alias))?;

    eprintln!("[PIP] alias=0x{:03X}  node_id={}", alias, proxy.node_id);

    let (pip_flags, status) = proxy.query_pip().await?;

    Ok(QueryPipResponse { alias, pip_flags, status })
}

/// Query Protocol Identification Protocol data for multiple nodes concurrently.
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

    let mut proxy_aliases = Vec::with_capacity(aliases.len());
    for &alias in &aliases {
        if let Some(proxy) = state.node_registry.get_by_alias(alias).await {
            proxy_aliases.push((alias, proxy));
        }
    }

    let mut join_set = tokio::task::JoinSet::new();
    for (alias, proxy) in proxy_aliases {
        let proxy = proxy.clone();
        join_set.spawn(async move {
            let result = proxy.query_pip().await;
            (alias, proxy.node_id, result)
        });
    }

    let mut results = Vec::new();
    while let Some(join_result) = join_set.join_next().await {
        let (alias, _node_id, result) = join_result.map_err(|e| e.to_string())?;
        let (pip_flags, status) = result.unwrap_or((None, PIPStatus::Error));

        results.push(QueryPipResponse { alias, pip_flags, status });
    }

    Ok(results)
}
