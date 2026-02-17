//! CDI (Configuration Description Information) XML viewer commands

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::Utc;
use tauri::Manager;

/// Error types for CDI operations
#[derive(Debug, thiserror::Error)]
pub enum CdiError {
    #[error("CdiNotRetrieved: CDI not yet retrieved for node {0}")]
    CdiNotRetrieved(String),
    
    #[error("CdiUnavailable: Node {0} does not provide CDI")]
    CdiUnavailable(String),
    
    #[error("RetrievalFailed: {0}")]
    RetrievalFailed(String),
    
    #[error("InvalidXml: {0}")]
    InvalidXml(String),
    
    #[error("NodeNotFound: Node {0} not found")]
    NodeNotFound(String),

    #[error("IoError: {0}")]
    IoError(String),
}

/// Convert CdiError to String for Tauri (implements Display via thiserror)
impl From<CdiError> for String {
    fn from(err: CdiError) -> String {
        err.to_string()
    }
}

/// Response containing CDI XML data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCdiXmlResponse {
    /// Raw CDI XML content as string (null if not available)
    pub xml_content: Option<String>,
    
    /// Size of XML content in bytes (null if xml_content is null)
    pub size_bytes: Option<usize>,
    
    /// Timestamp when CDI was retrieved (ISO 8601 format)
    pub retrieved_at: Option<String>,
}

/// Generate CDI cache file path based on node SNIP data
/// 
/// Uses format: {manufacturer}_{model}_{software_version}.cdi.xml
fn get_cdi_cache_path(
    app_handle: &tauri::AppHandle,
    manufacturer: &str,
    model: &str,
    version: &str,
) -> Result<PathBuf, CdiError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| CdiError::IoError(format!("Failed to get app data dir: {}", e)))?;

    // Create cdi_cache subdirectory
    let cdi_cache_dir = app_data_dir.join("cdi_cache");
    std::fs::create_dir_all(&cdi_cache_dir)
        .map_err(|e| CdiError::IoError(format!("Failed to create CDI cache dir: {}", e)))?;

    // Sanitize parts for filename (replace invalid characters)
    let sanitize = |s: &str| {
        s.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>()
    };

    let filename = format!(
        "{}_{}_{}. cdi.xml",
        sanitize(manufacturer),
        sanitize(model),
        sanitize(version)
    );

    Ok(cdi_cache_dir.join(filename))
}

/// Read CDI from file cache if it exists
async fn read_cdi_from_cache(cache_path: &PathBuf) -> Option<String> {
    tokio::fs::read_to_string(cache_path).await.ok()
}

/// Write CDI to file cache
async fn write_cdi_to_cache(cache_path: &PathBuf, xml_content: &str) -> Result<(), CdiError> {
    tokio::fs::write(cache_path, xml_content)
        .await
        .map_err(|e| CdiError::IoError(format!("Failed to write CDI cache: {}", e)))
}

/// Download CDI from a node over the network
/// 
/// Retrieves CDI XML using the Memory Configuration Protocol and caches it
/// both in memory (node.cdi) and on disk (cdi_cache directory).
#[tauri::command]
pub async fn download_cdi(
    node_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<GetCdiXmlResponse, String> {
    println!("[CDI] download_cdi called for node: {}", node_id);
    
    // Parse node ID
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;

    // Get node alias and SNIP data
    let (alias, snip_data) = {
        let nodes = state.nodes.read().await;
        let node = nodes
            .iter()
            .find(|n| n.node_id == parsed_node_id)
            .ok_or_else(|| CdiError::NodeNotFound(node_id.clone()))?;

        (node.alias.value(), node.snip_data.clone())
    };
    
    println!("[CDI] Found node with alias: 0x{:03X}", alias);

    // Get connection
    let mut conn_guard = state.connection.write().await;
    let connection = conn_guard
        .as_mut()
        .ok_or_else(|| CdiError::RetrievalFailed("Not connected to LCC network".to_string()))?;

    println!("[CDI] Starting CDI download from alias 0x{:03X}...", alias);
    
    // Download CDI from node (5 second timeout per chunk to accommodate slower nodes)
    let xml_content = connection
        .read_cdi(alias, 5000)
        .await
        .map_err(|e| {
            println!("[CDI] Download failed: {}", e);
            CdiError::RetrievalFailed(format!("CDI download failed: {}", e))
        })?;

    println!("[CDI] Download complete, size: {} bytes", xml_content.len());
    
    let retrieved_at = Utc::now();

    // Create CdiData
    let cdi_data = lcc_rs::CdiData {
        xml_content: xml_content.clone(),
        retrieved_at,
    };

    // Update node cache with CDI
    drop(conn_guard); // Release connection lock before updating nodes
    state
        .update_node(parsed_node_id, |node| {
            node.cdi = Some(cdi_data.clone());
        })
        .await;

    // Write to file cache if we have SNIP data
    if let Some(snip) = snip_data {
        let cache_path = get_cdi_cache_path(
            &app_handle,
            &snip.manufacturer,
            &snip.model,
            &snip.software_version,
        )?;

        write_cdi_to_cache(&cache_path, &xml_content).await?;
    }

    Ok(GetCdiXmlResponse {
        xml_content: Some(xml_content.clone()),
        size_bytes: Some(xml_content.len()),
        retrieved_at: Some(retrieved_at.to_rfc3339()),
    })
}

/// Get CDI XML for a specific node
/// 
/// Retrieves CDI from (in order of priority):
/// 1. Memory cache (node.cdi)
/// 2. File cache (cdi_cache/{manufacturer}_{model}_{version}.cdi.xml)
/// 
/// If not found in either cache, returns CdiNotRetrieved error.
/// Use download_cdi command to retrieve from network.
#[tauri::command]
pub async fn get_cdi_xml(
    node_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<GetCdiXmlResponse, String> {
    // Parse node ID
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;
    
    // Access node cache
    let nodes = state.nodes.read().await;
    
    // Find node
    let node = nodes
        .iter()
        .find(|n| n.node_id == parsed_node_id)
        .ok_or_else(|| CdiError::NodeNotFound(node_id.clone()))?;
    
    // Check memory cache first
    if let Some(cdi) = &node.cdi {
        return Ok(GetCdiXmlResponse {
            xml_content: Some(cdi.xml_content.clone()),
            size_bytes: Some(cdi.xml_content.len()),
            retrieved_at: Some(cdi.retrieved_at.to_rfc3339()),
        });
    }

    // Check file cache if we have SNIP data
    if let Some(snip) = &node.snip_data {
        let cache_path = get_cdi_cache_path(
            &app_handle,
            &snip.manufacturer,
            &snip.model,
            &snip.software_version,
        )?;

        if let Some(xml_content) = read_cdi_from_cache(&cache_path).await {
            // Found in file cache - update memory cache for future requests
            let retrieved_at = Utc::now();
            let cdi_data = lcc_rs::CdiData {
                xml_content: xml_content.clone(),
                retrieved_at,
            };

            drop(nodes); // Release read lock before updating
            state
                .update_node(parsed_node_id, |node| {
                    node.cdi = Some(cdi_data);
                })
                .await;

            return Ok(GetCdiXmlResponse {
                xml_content: Some(xml_content.clone()),
                size_bytes: Some(xml_content.len()),
                retrieved_at: Some(retrieved_at.to_rfc3339()),
            });
        }
    }

    // Not found in either cache
    Err(CdiError::CdiNotRetrieved(node_id.clone()).into())
}
