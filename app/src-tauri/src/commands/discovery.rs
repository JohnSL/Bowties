//! Node discovery and SNIP query commands

use crate::state::AppState;
use lcc_rs::{DiscoveredNode, NodeAlias, SNIPData, SNIPStatus};
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

/// Query SNIP data for a single node
#[tauri::command]
pub async fn query_snip_single(
    alias: u16,
    state: tauri::State<'_, AppState>,
) -> Result<QuerySnipResponse, String> {
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
    
    // Lock and query SNIP
    let mut connection = connection_arc.lock().await;
    let (snip_data, status) = connection
        .query_snip(alias, None)
        .await
        .map_err(|e| format!("SNIP query failed: {}", e))?;
    drop(connection);
    
    // Update node in cache if it exists
    if let Some(node_id) = state.get_nodes().await.iter()
        .find(|n| n.alias.value() == alias)
        .map(|n| n.node_id)
    {
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

/// Refresh all discovered nodes to check their current status
#[tauri::command]
pub async fn refresh_all_nodes(
    timeout_ms: Option<u64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DiscoveredNode>, String> {
    let timeout = timeout_ms.unwrap_or(500);
    
    // Get current list of nodes
    let nodes = state.get_nodes().await;
    
    if nodes.is_empty() {
        return Ok(vec![]);
    }
    
    // Get connection reference
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err("Not connected to LCC network".to_string()),
        }
    };
    
    // Verify each node
    for node in &nodes {
        let alias = node.alias.value();
        
        let mut connection = connection_arc.lock().await;
        let result = connection.verify_node(alias, timeout).await;
        drop(connection);
        
        match result {
            Ok(Some(_node_id)) => {
                // Node responded - update status
                state.update_node(node.node_id, |n| {
                    n.connection_status = lcc_rs::types::ConnectionStatus::Connected;
                    n.last_verified = Some(chrono::Utc::now());
                    n.last_seen = chrono::Utc::now();
                }).await;
            }
            Ok(None) => {
                // Node did not respond - mark as not responding
                state.update_node(node.node_id, |n| {
                    n.connection_status = lcc_rs::types::ConnectionStatus::NotResponding;
                }).await;
            }
            Err(_e) => {
                // Error occurred - mark as unknown
                state.update_node(node.node_id, |n| {
                    n.connection_status = lcc_rs::types::ConnectionStatus::Unknown;
                }).await;
            }
        }
    }
    
    // Return updated nodes
    Ok(state.get_nodes().await)
}
