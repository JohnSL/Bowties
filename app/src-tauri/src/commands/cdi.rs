//! CDI (Configuration Description Information) XML viewer commands

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::Utc;
use tauri::{Manager, Emitter};
use std::collections::HashMap;
use uuid::Uuid;

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
        "{}_{}_{}.cdi.xml",
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

/// Check if CDI is available for a node (memory cache or file cache)
/// 
/// Checks in order of priority:
/// 1. Memory cache (node.cdi) - fastest
/// 2. File cache (cdi_cache/{manufacturer}_{model}_{version}.cdi.xml)
async fn has_cdi_available(
    node: &lcc_rs::DiscoveredNode,
    app_handle: &tauri::AppHandle,
) -> bool {
    // 1. Check memory cache first (fastest)
    if node.cdi.is_some() {
        return true;
    }
    
    // 2. Check file cache if we have SNIP data
    if let Some(snip) = &node.snip_data {
        if !snip.manufacturer.is_empty() && !snip.model.is_empty() {
            if let Ok(cache_path) = get_cdi_cache_path(
                app_handle,
                &snip.manufacturer,
                &snip.model,
                &snip.software_version,
            ) {
                return cache_path.exists();
            }
        }
    }
    
    false
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
    use std::time::Instant as StdInstant;
    println!("[CDI] download_cdi called for node: {}", node_id);
    
    // Parse node ID
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;

    // Get node alias, SNIP data, and PIP flags from proxy
    let proxy = state.node_registry.get(&parsed_node_id).await
        .ok_or_else(|| CdiError::NodeNotFound(node_id.clone()))?;
    let snap = proxy.get_snapshot().await
        .map_err(|e| format!("Failed to get node snapshot: {}", e))?;
    let (alias, snip_data, pip_status, pip_flags) = (
        snap.alias.value(), snap.snip_data.clone(), snap.pip_status, snap.pip_flags,
    );

    // If PIP has been queried and the node does not advertise CDI or Memory
    // Configuration, there is nothing to download — fail early with a clear message.
    if pip_status == lcc_rs::PIPStatus::Complete {
        if let Some(flags) = pip_flags {
            if !flags.cdi && !flags.memory_configuration {
                return Err(CdiError::CdiUnavailable(node_id.clone()).into());
            }
        }
    }

    let snip_label = snip_data.as_ref().map(|s| {
        if !s.user_name.is_empty() { s.user_name.clone() } else { s.model.clone() }
    }).unwrap_or_else(|| node_id.clone());
    
    println!("[CDI] Found node with alias: 0x{:03X}", alias);

    // Get connection reference
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err(CdiError::RetrievalFailed("Not connected to LCC network".to_string()).into()),
        }
    };

    println!("[CDI] Starting CDI download from alias 0x{:03X}...", alias);
    crate::bwlog!(state.inner(), "[cdi] Downloading CDI for {} (alias={:#05x})", snip_label, alias);
    let dl_start = StdInstant::now();

    // Reset the cancel flag before starting (mirrors cancel_config_reading pattern).
    state.cdi_download_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    let cancel_flag = state.cdi_download_cancel.clone();

    // Download CDI from node (5 second timeout per chunk to accommodate slower nodes)
    let xml_content = {
        let mut connection = connection_arc.lock().await;
        connection
            .read_cdi_cancellable(alias, 5000, cancel_flag)
            .await
            .map_err(|e| {
                println!("[CDI] Download failed: {}", e);
                CdiError::RetrievalFailed(format!("CDI download failed: {}", e))
            })?
    };

    let dl_ms = dl_start.elapsed().as_millis() as u64;
    println!("[CDI] Download complete, size: {} bytes", xml_content.len());
    crate::bwlog!(state.inner(), "[cdi] CDI download complete for {}: {} bytes in {}ms",
        snip_label, xml_content.len(), dl_ms);
    
    let retrieved_at = Utc::now();

    // Record CdiDownloadStats.
    {
        let stats_entry = crate::diagnostics::CdiDownloadStats {
            node_id: node_id.clone(),
            snip_name: snip_data.as_ref().map(|_s| snip_label.clone()),
            from_cache: false,
            total_bytes: xml_content.len(),
            chunks: 0,  // not exposed by read_cdi
            chunk_durations_ms: vec![],
            total_duration_ms: dl_ms,
        };
        state.diag_stats.write().await.cdi_downloads.insert(node_id.clone(), stats_entry);
    }

    // Create CdiData
    let cdi_data = lcc_rs::CdiData {
        xml_content: xml_content.clone(),
        retrieved_at,
    };

    // Store in proxy (primary)
    if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        let _ = proxy.set_cdi_data(cdi_data.clone()).await;
    }

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

    // Check proxy first (primary source)
    if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        if let Ok(Some(cdi_data)) = proxy.get_cdi_data().await {
            return Ok(GetCdiXmlResponse {
                xml_content: Some(cdi_data.xml_content.clone()),
                size_bytes: Some(cdi_data.xml_content.len()),
                retrieved_at: Some(cdi_data.retrieved_at.to_rfc3339()),
            });
        }
    }

    // Get node SNIP data from proxy snapshot for file cache lookup
    let snip_data = if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        if let Ok(snap) = proxy.get_snapshot().await {
            snap.snip_data.clone()
        } else {
            None
        }
    } else {
        None
    };

    // Try to get CDI from active layout first (if one is loaded)
    if let Some(active_layout) = state.active_layout.read().await.clone() {
        if let Some(snip) = &snip_data {
            let cache_key = format!(
                "{}_{}_{}",
                snip.manufacturer.replace(' ', "_"),
                snip.model.replace(' ', "_"),
                snip.software_version.replace(' ', "_")
            );
            
            // Derive companion directory from root_path
            let base_file = std::path::Path::new(&active_layout.root_path);
            if let Some(parent) = base_file.parent() {
                if let Some(file_name) = base_file.file_name().and_then(|v| v.to_str()) {
                    // Strip common suffixes to get stem
                    let suffixes = [".layout", ".bowties-layout.yaml", ".bowties-layout.yml", ".yaml", ".yml"];
                    let stem = suffixes.iter()
                        .find_map(|suffix| file_name.strip_suffix(suffix))
                        .unwrap_or(file_name);
                    
                    let companion_dir = parent.join(format!("{}.layout.d", stem));
                    let cdi_path = companion_dir.join("cdi").join(format!("{}.xml", cache_key));
                    
                    if cdi_path.exists() {
                        if let Ok(xml_content) = std::fs::read_to_string(&cdi_path) {
                            let retrieved_at = Utc::now();
                            return Ok(GetCdiXmlResponse {
                                xml_content: Some(xml_content.clone()),
                                size_bytes: Some(xml_content.len()),
                                retrieved_at: Some(retrieved_at.to_rfc3339()),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check file cache if we have SNIP data
    if let Some(snip) = &snip_data {
        let cache_path = get_cdi_cache_path(
            &app_handle,
            &snip.manufacturer,
            &snip.model,
            &snip.software_version,
        )?;

        if let Some(xml_content) = read_cdi_from_cache(&cache_path).await {
            let retrieved_at = Utc::now();
            let cdi_data = lcc_rs::CdiData {
                xml_content: xml_content.clone(),
                retrieved_at,
            };

            // Store in proxy
            if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
                let _ = proxy.set_cdi_data(cdi_data).await;
            }

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

// ============================================================================
// Miller Columns Navigation Commands
// ============================================================================

/// Discovered node information for the Nodes column
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredNode {
    pub node_id: String,
    pub node_name: String,
    pub has_cdi: bool,
}

/// Response for get_discovered_nodes command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDiscoveredNodesResponse {
    pub nodes: Vec<DiscoveredNode>,
}

/// Get list of discovered nodes for Nodes column (leftmost)
#[tauri::command]
pub async fn get_discovered_nodes(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<GetDiscoveredNodesResponse, String> {
    // Primary: build from registry snapshots
    let snapshots = state.node_registry.get_all_snapshots().await;
    
    let mut discovered_nodes = Vec::new();
    
    for node in &snapshots {
        let node_name = node
            .snip_data
            .as_ref()
            .and_then(|s| {
                if !s.user_name.is_empty() {
                    Some(s.user_name.clone())
                } else if !s.manufacturer.is_empty() && !s.model.is_empty() {
                    Some(format!("{} {}", s.manufacturer, s.model))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| format!("Node {}", node.node_id.to_hex_string()));
        
        // Check if CDI is available (memory cache or file cache)
        let has_cdi = has_cdi_available(node, &app_handle).await;
        
        discovered_nodes.push(DiscoveredNode {
            node_id: node.node_id.to_hex_string(),
            node_name,
            has_cdi,
        });
    }
    
    Ok(GetDiscoveredNodesResponse {
        nodes: discovered_nodes,
    })
}

/// CDI structure response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiStructureResponse {
    pub node_id: String,
    pub node_name: String,
    pub segments: Vec<SegmentInfo>,
    pub max_depth: usize,
}

/// Segment information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentInfo {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub space: u8,
    pub has_groups: bool,
    pub has_elements: bool,
    pub metadata: Option<serde_json::Value>,
}

/// Parse and return the CDI structure for a node
/// T104: Uses cached parsed CDI if available
#[tauri::command]
pub async fn get_cdi_structure(
    node_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<CdiStructureResponse, String> {
    let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await?;
    
    // Calculate max depth
    let max_depth = lcc_rs::cdi::hierarchy::calculate_max_depth(&cdi);
    
    // Convert segments to SegmentInfo
    let segments = cdi
        .segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let has_groups = seg.elements.iter().any(|e| matches!(e, lcc_rs::cdi::DataElement::Group(_)));
            let has_elements = !seg.elements.is_empty();
            let path_id = format!("seg:{}", i);
            
            SegmentInfo {
                id: Uuid::new_v4().to_string(),
                name: seg.name.clone(),
                description: seg.description.clone(),
                space: seg.space,
                has_groups,
                has_elements,
                metadata: Some(serde_json::json!({
                    "pathId": path_id,
                    "space": seg.space,
                })),
            }
        })
        .collect();
    
    // Get node name from proxy
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;
    
    let node_name = if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        if let Ok(snap) = proxy.get_snapshot().await {
            snap.snip_data.as_ref()
                .map(|s| s.user_name.clone())
                .unwrap_or_else(|| format!("Node {}", node_id))
        } else {
            format!("Node {}", node_id)
        }
    } else {
        format!("Node {}", node_id)
    };
    
    Ok(CdiStructureResponse {
        node_id,
        node_name,
        segments,
        max_depth,
    })
}

/// Column item for navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnItem {
    pub id: String,
    pub name: String,
    pub full_name: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub has_children: bool,
    pub metadata: Option<serde_json::Value>,
}

/// Column items response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetColumnItemsResponse {
    pub depth: usize,
    pub column_type: String,
    pub items: Vec<ColumnItem>,
}

/// Get items for a specific column based on parent selection
/// T103: Tracks performance and logs if > 500ms
/// T104: Uses cached parsed CDI to avoid redundant parsing
#[tauri::command]
pub async fn get_column_items(
    node_id: String,
    parent_path: Vec<String>,
    depth: usize,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<GetColumnItemsResponse, String> {
    // T103: Start performance tracking
    let start_time = std::time::Instant::now();

    let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await?;
    
    // If parent_path is empty, we're at the segments level (should not happen - segments are in get_cdi_structure)
    if parent_path.is_empty() {
        return Err("Invalid path: parent_path cannot be empty for get_column_items".to_string());
    }
    
    // Navigate to the parent path
    let navigation_result = lcc_rs::cdi::hierarchy::navigate_to_path(&cdi, &parent_path)
        .map_err(|e| format!("Invalid path: {}", e))?;
    
    // Get elements from the navigation result
    let elements = match navigation_result {
        lcc_rs::cdi::hierarchy::NavigationResult::Segment(segment) => {
            &segment.elements
        }
        lcc_rs::cdi::hierarchy::NavigationResult::Element(element) => {
            // If the element is a group, return its children
            match element {
                lcc_rs::cdi::DataElement::Group(group) => &group.elements,
                _ => {
                    // Primitive element - no children
                    return Ok(GetColumnItemsResponse {
                        depth,
                        column_type: "elements".to_string(),
                        items: vec![],
                    });
                }
            }
        }
    };
    
    // Determine column type and generate items
    // First, check if there are any groups
    let has_groups = elements
        .iter()
        .any(|e| matches!(e, lcc_rs::cdi::DataElement::Group(_)));
    
    if has_groups {
        // Return groups (filter empty groups per Footnote 4)
        let mut group_items = Vec::new();
        
        for (i, e) in elements.iter().enumerate() {
            if let lcc_rs::cdi::DataElement::Group(group) = e {
                // Filter per Footnote 4
                if !group.should_render() {
                    continue;
                }
                
                // Check if group is replicated (T084: instance-specific address calculation)
                if group.replication > 1 {
                    // For replicated groups, expand into separate instances
                    let base_address = group.offset;
                    let expanded = group.expand_replications(base_address);
                    
                    // T081, T082, T085: Return separate items for each instance
                    for expanded_instance in expanded {
                        let path_id = format!("elem:{}#{}", i, expanded_instance.index + 1);
                        
                        group_items.push(ColumnItem {
                            id: Uuid::new_v4().to_string(),
                            name: expanded_instance.name.clone(),
                            full_name: group.description.clone(),
                            item_type: Some("group".to_string()),
                            has_children: !group.elements.is_empty(),
                            metadata: Some(serde_json::json!({
                                "pathId": path_id,
                                "replicated": true,
                                "instanceIndex": expanded_instance.index,
                                "instanceNumber": expanded_instance.index + 1,
                                "totalInstances": group.replication,
                                "baseAddress": base_address,
                                "instanceAddress": expanded_instance.address,
                            })),
                        });
                    }
                } else {
                    // Single instance group
                    let path_id = format!("elem:{}", i);
                    
                    group_items.push(ColumnItem {
                        id: Uuid::new_v4().to_string(),
                        name: group.name.clone().unwrap_or_else(|| format!("Group {}", i)),
                        full_name: group.description.clone(),
                        item_type: Some("group".to_string()),
                        has_children: !group.elements.is_empty(),
                        metadata: Some(serde_json::json!({
                            "pathId": path_id,
                            "replicated": false,
                            "replication": 1,
                        })),
                    });
                }
            }
        }
        
        // T103: Track elapsed time and log if > 500ms
        let elapsed = start_time.elapsed();
        if elapsed.as_millis() > 500 {
            eprintln!(
                "[PERF] get_column_items slow: {}ms (node: {}, depth: {}, path: {:?})",
                elapsed.as_millis(),
                node_id,
                depth,
                parent_path
            );
        }
        
        Ok(GetColumnItemsResponse {
            depth,
            column_type: "groups".to_string(),
            items: group_items,
        })
    } else {
        // Return elements (primitives: Int, String, EventId, etc.)
        let element_items = elements
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let (name, description, item_type) = match e {
                    lcc_rs::cdi::DataElement::Int(int_elem) => (
                        int_elem.name.clone().unwrap_or_else(|| format!("Int {}", i)),
                        int_elem.description.clone(),
                        "int".to_string(),
                    ),
                    lcc_rs::cdi::DataElement::String(str_elem) => (
                        str_elem.name.clone().unwrap_or_else(|| format!("String {}", i)),
                        str_elem.description.clone(),
                        "string".to_string(),
                    ),
                    lcc_rs::cdi::DataElement::EventId(evt_elem) => (
                        evt_elem.name.clone().unwrap_or_else(|| format!("EventID {}", i)),
                        evt_elem.description.clone(),
                        "eventid".to_string(),
                    ),
                    lcc_rs::cdi::DataElement::Float(flt_elem) => (
                        flt_elem.name.clone().unwrap_or_else(|| format!("Float {}", i)),
                        flt_elem.description.clone(),
                        "float".to_string(),
                    ),
                    lcc_rs::cdi::DataElement::Action(act_elem) => (
                        act_elem.name.clone().unwrap_or_else(|| format!("Action {}", i)),
                        act_elem.description.clone(),
                        "action".to_string(),
                    ),
                    lcc_rs::cdi::DataElement::Blob(blob_elem) => (
                        blob_elem.name.clone().unwrap_or_else(|| format!("Blob {}", i)),
                        blob_elem.description.clone(),
                        "blob".to_string(),
                    ),
                    lcc_rs::cdi::DataElement::Group(_) => {
                        // Should not happen (filtered above)
                        return None;
                    }
                };
                
                Some(ColumnItem {
                    id: Uuid::new_v4().to_string(),
                    name: name.clone(),
                    full_name: description,
                    item_type: Some(item_type),
                    has_children: false,
                    metadata: Some(serde_json::json!({
                        "pathId": format!("elem:{}", i),
                    })),
                })
            })
            .collect::<Vec<_>>();
        
        // T103: Track elapsed time and log if > 500ms
        let elapsed = start_time.elapsed();
        if elapsed.as_millis() > 500 {
            eprintln!(
                "[PERF] get_column_items slow: {}ms (node: {}, depth: {}, path: {:?})",
                elapsed.as_millis(),
                node_id,
                depth,
                parent_path
            );
        }
        
        Ok(GetColumnItemsResponse {
            depth,
            column_type: "elements".to_string(),
            items: element_items,
        })
    }
}

/// Constraint information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Constraint {
    #[serde(rename = "type")]
    pub constraint_type: String,
    pub description: String,
    pub value: serde_json::Value,
}

/// Element details response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementDetailsResponse {
    pub name: String,
    pub description: Option<String>,
    pub data_type: String,
    pub full_path: String,
    pub element_path: Vec<String>,
    pub constraints: Vec<Constraint>,
    pub default_value: Option<String>,
    pub memory_address: i32,
}

/// Get detailed information for a selected element
#[tauri::command]
pub async fn get_element_details(
    node_id: String,
    element_path: Vec<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ElementDetailsResponse, String> {
    // Get CDI from parse cache (avoids redundant XML fetch and parse on every element click)
    let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await?;
    
    // Navigate to the element
    let navigation_result = lcc_rs::cdi::hierarchy::navigate_to_path(&cdi, &element_path)
        .map_err(|e| format!("Element not found: {}", e))?;
    
    let element = match navigation_result {
        lcc_rs::cdi::hierarchy::NavigationResult::Element(elem) => elem,
        lcc_rs::cdi::hierarchy::NavigationResult::Segment(_) => {
            return Err("Path points to segment, not element".to_string());
        }
    };
    
    // Get node name from proxy
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;
    
    let node_name = if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        if let Ok(snap) = proxy.get_snapshot().await {
            snap.snip_data.as_ref()
                .map(|s| s.user_name.clone())
                .unwrap_or_else(|| format!("Node {}", node_id))
        } else {
            format!("Node {}", node_id)
        }
    } else {
        format!("Node {}", node_id)
    };
    
    // Build full path breadcrumb
    let full_path = format!("{} › {}", node_name, element_path.join(" › "));
    
    // Extract element details based on type
    match element {
        lcc_rs::cdi::DataElement::EventId(evt) => {
            Ok(ElementDetailsResponse {
                name: evt.name.clone().unwrap_or_else(|| "Event ID".to_string()),
                description: evt.description.clone(),
                data_type: "Event ID (8 bytes)".to_string(),
                full_path,
                element_path,
                constraints: vec![],
                default_value: None,
                memory_address: evt.offset,
            })
        }
        lcc_rs::cdi::DataElement::Int(int_elem) => {
            let mut constraints = vec![];
            
            // Add range constraint if min/max specified
            if int_elem.min.is_some() || int_elem.max.is_some() {
                constraints.push(Constraint {
                    constraint_type: "range".to_string(),
                    description: format!(
                        "Range: {} to {}",
                        int_elem.min.map(|v| v.to_string()).unwrap_or_else(|| "−∞".to_string()),
                        int_elem.max.map(|v| v.to_string()).unwrap_or_else(|| "∞".to_string())
                    ),
                    value: serde_json::json!({
                        "min": int_elem.min,
                        "max": int_elem.max,
                    }),
                });
            }
            
            // Add map constraint if present
            if let Some(map) = &int_elem.map {
                let map_entries: Vec<_> = map.entries.iter().map(|e| {
                    serde_json::json!({
                        "value": e.value,
                        "label": e.label,
                    })
                }).collect();
                
                constraints.push(Constraint {
                    constraint_type: "map".to_string(),
                    description: "Value mapping".to_string(),
                    value: serde_json::json!({ "entries": map_entries }),
                });
            }
            
            Ok(ElementDetailsResponse {
                name: int_elem.name.clone().unwrap_or_else(|| "Integer".to_string()),
                description: int_elem.description.clone(),
                data_type: format!("Integer ({} bytes)", int_elem.size),
                full_path,
                element_path: element_path.clone(),
                constraints,
                default_value: int_elem.default.map(|v| v.to_string()),
                memory_address: int_elem.offset,
            })
        }
        lcc_rs::cdi::DataElement::String(str_elem) => {
            let constraints = vec![Constraint {
                constraint_type: "length".to_string(),
                description: format!("Max length: {} bytes", str_elem.size),
                value: serde_json::json!({ "maxLength": str_elem.size }),
            }];
            
            Ok(ElementDetailsResponse {
                name: str_elem.name.clone().unwrap_or_else(|| "String".to_string()),
                description: str_elem.description.clone(),
                data_type: format!("String (max {} bytes)", str_elem.size),
                full_path,
                element_path: element_path.clone(),
                constraints,
                default_value: None,
                memory_address: str_elem.offset,
            })
        }
        lcc_rs::cdi::DataElement::Float(flt_elem) => {
            Ok(ElementDetailsResponse {
                name: flt_elem.name.clone().unwrap_or_else(|| "Float".to_string()),
                description: flt_elem.description.clone(),
                data_type: "Float (4 bytes)".to_string(),
                full_path,
                element_path: element_path.clone(),
                constraints: vec![],
                default_value: None,
                memory_address: flt_elem.offset,
            })
        }
        lcc_rs::cdi::DataElement::Action(act_elem) => {
            Ok(ElementDetailsResponse {
                name: act_elem.name.clone().unwrap_or_else(|| "Action".to_string()),
                description: act_elem.description.clone(),
                data_type: "Action (trigger)".to_string(),
                full_path,
                element_path: element_path.clone(),
                constraints: vec![],
                default_value: None,
                memory_address: act_elem.offset,
            })
        }
        lcc_rs::cdi::DataElement::Blob(blob_elem) => {
            Ok(ElementDetailsResponse {
                name: blob_elem.name.clone().unwrap_or_else(|| "Blob".to_string()),
                description: blob_elem.description.clone(),
                data_type: format!("Blob ({} bytes)", blob_elem.size),
                full_path,
                element_path: element_path.clone(),
                constraints: vec![],
                default_value: None,
                memory_address: blob_elem.offset,
            })
        }
        lcc_rs::cdi::DataElement::Group(_) => {
            Err("Element is a group, not a primitive element".to_string())
        }
    }
}

/// Group instance from replication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInstance {
    pub index: u32,
    pub name: String,
    pub address: i32,
}

/// Expanded replicated group response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpandReplicatedGroupResponse {
    pub group_name: String,
    pub replication_count: u32,
    pub instances: Vec<GroupInstance>,
}

/// Expand a replicated group into individual instances
#[tauri::command]
pub async fn expand_replicated_group(
    _node_id: String,
    _group_path: Vec<String>,
    _state: tauri::State<'_, AppState>,
) -> Result<ExpandReplicatedGroupResponse, String> {
    // TODO: Implement group expansion logic
    // For now, return a stub
    Err("Group not found or not replicated".to_string())
}
#[cfg(test)]
mod tests {
    use super::*;

    // T043l-T043o: Basic struct validation tests
    // Full integration tests would require proper AppState and mocked LCC connections

    #[test]
    fn test_discovered_node_struct() {
        let node = DiscoveredNode {
            node_id: "05.01.01.01.03.01".to_string(),
            node_name: "Test Node".to_string(),
            has_cdi: true,
        };
        assert_eq!(node.node_id, "05.01.01.01.03.01");
        assert!(node.has_cdi);
    }

    #[test]
    fn test_segment_info_struct() {
        let seg = SegmentInfo {
            id: "seg-0".to_string(),
            name: Some("Config".to_string()),
            description: None,
            space: 253,
            has_groups: true,
            has_elements: true,
            metadata: None,
        };
        assert_eq!(seg.id, "seg-0");
        assert_eq!(seg.space, 253);
    }

    #[test]
    fn test_cdi_error_messages() {
        let err1 =  CdiError::CdiNotRetrieved("test_node".to_string());
        assert!(err1.to_string().contains("not yet retrieved"));
        
        let err2 = CdiError::InvalidXml("parse error".to_string());
        assert!(err2.to_string().contains("InvalidXml"));
    }
}

// ============================================================================
// Configuration Value Reading Commands (Feature 004-read-node-config)
// ============================================================================

/// Typed configuration value read from node memory (T004)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConfigValue {
    #[serde(rename = "Int")]
    Int { value: i64, size_bytes: u8 },
    
    #[serde(rename = "String")]
    String { value: String, size_bytes: u32 },
    
    #[serde(rename = "EventId")]
    EventId { value: [u8; 8] },
    
    #[serde(rename = "Float")]
    Float { value: f32 },
    
    #[serde(rename = "Invalid")]
    Invalid { error: String },
}

/// Configuration value with metadata (T005)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValueWithMetadata {
    pub value: ConfigValue,
    pub memory_address: u32,
    pub address_space: u8,
    pub element_path: Vec<String>,
    pub timestamp: String,
}

/// Progress status enum (T007)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProgressStatus {
    Starting,
    ReadingNode { node_name: String },
    NodeComplete { node_name: String, success: bool },
    Cancelled,
    Complete { success_count: usize, fail_count: usize },
}

/// Read progress update (T006)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadProgressUpdate {
    pub total_nodes: usize,
    pub current_node_index: usize,
    pub current_node_name: String,
    pub current_node_id: String,
    pub total_elements: usize,
    pub elements_read: usize,
    pub elements_failed: usize,
    pub percentage: u8,
    pub status: ProgressStatus,
}

/// Response from read_all_config_values command (T008)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadAllConfigValuesResponse {
    pub node_id: String,
    pub values: HashMap<String, ConfigValueWithMetadata>,
    pub total_elements: usize,
    pub successful_reads: usize,
    pub failed_reads: usize,
    pub duration_ms: u64,
    /// Set when reading was aborted after an unrecoverable batch failure.
    /// Contains a user-facing description of the error.
    pub abort_error: Option<String>,
}

/// Get element size in bytes from CDI element (T009)
fn get_element_size(element: &lcc_rs::cdi::DataElement) -> Result<u32, String> {
    use lcc_rs::cdi::DataElement;
    
    match element {
        DataElement::Int(e) => Ok(e.size as u32),
        DataElement::String(e) => Ok(e.size as u32),
        DataElement::EventId(_) => Ok(8),
        DataElement::Float(_) => Ok(4),
        DataElement::Group(_) => Err("Cannot read value from group element".to_string()),
        DataElement::Blob(e) => Ok(e.size as u32),
        _ => Err(format!("Unsupported element type for config reading")),
    }
}

/// Check if config reading operation has been cancelled (T013)
/// Returns error if cancellation requested
fn check_cancellation(state: &AppState) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    
    if state.config_read_cancel.load(Ordering::Relaxed) {
        Err("Operation cancelled by user".to_string())
    } else {
        Ok(())
    }
}

/// Parse raw bytes into typed ConfigValue based on element type (T032)
fn parse_config_value(element: &lcc_rs::cdi::DataElement, data: &[u8]) -> Result<ConfigValue, String> {
    use lcc_rs::cdi::DataElement;
    
    match element {
        DataElement::Int(e) => {
            let value = match e.size {
                1 => {
                    if data.len() < 1 {
                        return Err("Insufficient data for 1-byte int".to_string());
                    }
                    data[0] as i8 as i64
                }
                2 => {
                    if data.len() < 2 {
                        return Err("Insufficient data for 2-byte int".to_string());
                    }
                    i16::from_be_bytes([data[0], data[1]]) as i64
                }
                4 => {
                    if data.len() < 4 {
                        return Err("Insufficient data for 4-byte int".to_string());
                    }
                    i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as i64
                }
                8 => {
                    if data.len() < 8 {
                        return Err("Insufficient data for 8-byte int".to_string());
                    }
                    i64::from_be_bytes([
                        data[0], data[1], data[2], data[3],
                        data[4], data[5], data[6], data[7],
                    ])
                }
                _ => return Err(format!("Invalid int size: {}", e.size)),
            };
            Ok(ConfigValue::Int {
                value,
                size_bytes: e.size,
            })
        }
        DataElement::String(e) => {
            // Find the first null byte to avoid parsing padding as UTF-8
            let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
            let raw = &data[..end];
            // 0xFF is never valid UTF-8 and represents uninitialized flash on LCC
            // nodes. Filter ALL 0xFF bytes out (not just leading) then use lossy
            // conversion for any remaining non-UTF-8 bytes.
            let filtered: Vec<u8> = raw.iter().copied().filter(|&b| b != 0xFF).collect();
            let s = String::from_utf8_lossy(&filtered).into_owned();
            Ok(ConfigValue::String {
                value: s,
                size_bytes: e.size as u32,
            })
        }
        DataElement::EventId(_) => {
            if data.len() != 8 {
                return Err(format!("EventId must be 8 bytes, got {}", data.len()));
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&data[0..8]);
            Ok(ConfigValue::EventId { value: bytes })
        }
        DataElement::Float(_) => {
            if data.len() != 4 {
                return Err(format!("Float must be 4 bytes, got {}", data.len()));
            }
            let value = f32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            Ok(ConfigValue::Float { value })
        }
        _ => Err(format!("Unsupported element type for config reading")),
    }
}

/// Get the parsed CDI from cache or parse it
async fn get_cdi_from_cache(
    node_id: &str,
    app_handle: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
) -> Result<lcc_rs::cdi::Cdi, String> {
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;

    // Try the proxy first
    if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        if let Ok(Some(cdi)) = proxy.get_cdi_parsed().await {
            return Ok(cdi);
        }
    }

    // Not in cache — get CDI XML and parse it
    let cdi_response = get_cdi_xml(node_id.to_string(), app_handle.clone(), state.clone()).await?;
    
    let xml_content = cdi_response
        .xml_content
        .ok_or_else(|| CdiError::CdiNotRetrieved(node_id.to_string()))?;
    
    let parsed_cdi = lcc_rs::cdi::parser::parse_cdi(&xml_content)
        .map_err(CdiError::InvalidXml)?;
    
    // Store in proxy
    if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        let _ = proxy.set_cdi_parsed(parsed_cdi.clone()).await;
    }
    
    Ok(parsed_cdi)
}

/// Get human-readable display name for a node (T049)
/// Priority: user_name > user_description > model > node_id
fn get_node_display_name(node: &lcc_rs::DiscoveredNode) -> String {
    if let Some(snip) = &node.snip_data {
        if !snip.user_name.is_empty() {
            return snip.user_name.clone();
        }
        if !snip.user_description.is_empty() {
            return snip.user_description.clone();
        }
        if !snip.model.is_empty() {
            return snip.model.clone();
        }
    }
    
    // Fallback to node ID
    node.node_id.to_hex_string()
}

/// A configurable CDI element together with its resolved absolute memory location.
/// Produced by [`extract_all_elements_with_addresses`].
struct ElementWithAddress<'a> {
    /// Navigation path used as the cache key (e.g. `["seg:0", "elem:0#1", "elem:2"]`).
    path: Vec<String>,
    /// Byte address at which the owning segment begins (from `<segment origin=…>`).
    segment_origin: u32,
    /// Byte offset of this element from `segment_origin`, after all CDI cursor
    /// skips have been applied.
    element_offset: u32,
    /// Human-readable name extracted from the CDI `<name>` child (or a fallback).
    name: String,
    /// Borrowed reference to the parsed CDI element.
    element: &'a lcc_rs::cdi::DataElement,
    /// LCC address space byte for this element's segment (e.g. `0xFD`).
    space: u8,
}

impl<'a> ElementWithAddress<'a> {
    /// Absolute byte address to use in a `read_memory` call.
    fn absolute_address(&self) -> u32 {
        self.segment_origin + self.element_offset
    }
}

/// Recursively walk a CDI element slice, collecting every leaf (non-group) element
/// with its resolved absolute address into `results`.
///
/// Per the CDI spec each element's `offset` field is a *relative skip* from the end
/// of the previous element (not an absolute address).  `base_offset` tracks the
/// running cursor baseline at the start of this slice, and a local `cursor` grows
/// as elements are visited.
fn process_elements<'a>(
    elements: &'a [lcc_rs::cdi::DataElement],
    current_path: &mut Vec<String>,
    segment_origin: u32,
    // Absolute byte offset of the first element in this slice from `segment_origin`.
    base_offset: u32,
    segment_space: u8,
    results: &mut Vec<ElementWithAddress<'a>>,
) {
    use lcc_rs::cdi::DataElement;

    // Sequential cursor within this group/segment level.  Each element *skips*
    // cursor by `element.offset` first, then *advances* cursor by the element's size.
    let mut cursor: i32 = 0;

    for (i, element) in elements.iter().enumerate() {
        match element {
            DataElement::Group(g) => {
                let group_name = g.name.as_ref().map(|s| s.as_str()).unwrap_or("group");

                cursor += g.offset;
                let group_start = base_offset as i32 + cursor;
                let stride = g.calculate_size();

                // Guard: stride=0 with replication>1 would map all instances to the
                // same address, producing identical reads.  Clamp to 1 instance.
                let effective_replication = if stride == 0 && g.replication > 1 {
                    eprintln!(
                        "[CDI] Warning: group '{}' has replication={} but calculate_size()=0; \
                         clamping to 1 instance to avoid duplicate reads",
                        group_name, g.replication
                    );
                    1u32
                } else {
                    g.replication
                };

                for instance in 0..effective_replication {
                    if g.replication > 1 {
                        current_path.push(format!("elem:{}#{}", i, instance + 1));
                    } else {
                        current_path.push(format!("elem:{}", i));
                    }

                    let instance_base = (group_start + instance as i32 * stride) as u32;
                    process_elements(
                        &g.elements,
                        current_path,
                        segment_origin,
                        instance_base,
                        segment_space,
                        results,
                    );
                    current_path.pop();
                }

                cursor += effective_replication as i32 * stride;
            }
            DataElement::Int(e) => {
                cursor += e.offset;
                let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("int");
                current_path.push(format!("elem:{}", i));
                results.push(ElementWithAddress {
                    path: current_path.clone(),
                    segment_origin,
                    element_offset: (base_offset as i32 + cursor) as u32,
                    name: name.to_string(),
                    element,
                    space: segment_space,
                });
                current_path.pop();
                cursor += e.size as i32;
            }
            DataElement::String(e) => {
                cursor += e.offset;
                let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("string");
                current_path.push(format!("elem:{}", i));
                results.push(ElementWithAddress {
                    path: current_path.clone(),
                    segment_origin,
                    element_offset: (base_offset as i32 + cursor) as u32,
                    name: name.to_string(),
                    element,
                    space: segment_space,
                });
                current_path.pop();
                cursor += e.size as i32;
            }
            DataElement::EventId(e) => {
                cursor += e.offset;
                let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("eventid");
                current_path.push(format!("elem:{}", i));
                results.push(ElementWithAddress {
                    path: current_path.clone(),
                    segment_origin,
                    element_offset: (base_offset as i32 + cursor) as u32,
                    name: name.to_string(),
                    element,
                    space: segment_space,
                });
                current_path.pop();
                cursor += 8; // EventId is always 8 bytes
            }
            DataElement::Float(e) => {
                cursor += e.offset;
                let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("float");
                current_path.push(format!("elem:{}", i));
                results.push(ElementWithAddress {
                    path: current_path.clone(),
                    segment_origin,
                    element_offset: (base_offset as i32 + cursor) as u32,
                    name: name.to_string(),
                    element,
                    space: segment_space,
                });
                current_path.pop();
                cursor += 4; // 32-bit float
            }
            // Skip Action and Blob — they don't store readable configuration values,
            // but advance the cursor so subsequent elements get the right addresses.
            DataElement::Action(e) => { cursor += e.offset + 1; }
            DataElement::Blob(e)   => { cursor += e.offset + e.size as i32; }
        }
    }
}

/// Recursively extract all configurable elements from CDI with their absolute
/// memory addresses (T050).
fn extract_all_elements_with_addresses<'a>(
    cdi: &'a lcc_rs::cdi::Cdi,
) -> Vec<ElementWithAddress<'a>> {
    let mut results = Vec::new();
    for (seg_idx, segment) in cdi.segments.iter().enumerate() {
        let mut path = vec![format!("seg:{}", seg_idx)];
        process_elements(
            &segment.elements,
            &mut path,
            segment.origin as u32,
            0,
            segment.space,
            &mut results,
        );
    }
    results
}

/// Read a single configuration value from a node (T033)
#[tauri::command]
pub async fn read_config_value(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    node_id: String,
    element_path: Vec<String>,
    timeout_ms: Option<u64>,
) -> Result<ConfigValueWithMetadata, String> {
    let timeout = timeout_ms.unwrap_or(2000);
    
    // Parse node ID
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("Invalid node ID: {}", e))?;
    
    // Get CDI from cache
    let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await?;
    
    // Find element and compute absolute address via full CDI traversal.
    // This uses the same cursor-based logic as read_all_config_values, ensuring
    // correctness when CDI elements have relative (non-zero) offset skips.
    let all_elements = extract_all_elements_with_addresses(&cdi);
    let found = all_elements
        .iter()
        .find(|ewa| ewa.path.as_slice() == element_path.as_slice())
        .ok_or_else(|| format!("Element not found at path: {}", element_path.join("/")))?;
    let absolute_address = found.absolute_address();
    let element = found.element;
    let space = found.space;
    
    // Get connection
    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);
    
    // Get node alias from proxy
    let proxy = state.node_registry.get(&parsed_node_id).await
        .ok_or_else(|| format!("Node not found: {}", node_id))?;
    let alias = proxy.alias;
    
    // Get element size
    let size = get_element_size(element)?;
    if size > 64 {
        return Err(format!("Element size {} exceeds maximum 64 bytes", size));
    }
    
    // Read memory from node using the segment's declared address space
    let mut conn = connection.lock().await;
    let response_data = conn
        .read_memory(alias, space, absolute_address, size as u8, timeout)
        .await
        .map_err(|e| format!("Failed to read from node: {}", e))?; // T035: timeout handling
    drop(conn);
    
    // Parse typed value
    let typed_value = parse_config_value(element, &response_data)?;
    
    // Return with metadata
    Ok(ConfigValueWithMetadata {
        value: typed_value,
        memory_address: absolute_address,
        address_space: space,
        element_path,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

// ============================================================================
// Spec 007: Unified Node Tree Command
// ============================================================================

/// Return the unified configuration tree for a single node.
///
/// If a tree has already been built (e.g. after `read_all_config_values`),
/// the cached version is returned.  Otherwise a fresh tree is built from
/// the CDI parse cache and stored for future use.  Config values and event
/// roles are *not* merged here — that happens inside `read_all_config_values`.
#[tauri::command]
pub async fn get_node_tree(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    node_id: String,
) -> Result<crate::node_tree::NodeConfigTree, String> {
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;

    // Fast path: check proxy for cached tree
    if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        if let Ok(Some(tree)) = proxy.get_config_tree().await {
            return Ok(tree);
        }
    }

    // Build from CDI
    let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await?;
    let mut tree = crate::node_tree::build_node_config_tree(&node_id, &cdi);

    // Apply structure profile if available
    if let Some(identity) = &cdi.identification {
        let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
        let model = identity.model.as_deref().unwrap_or("");
        if !manufacturer.is_empty() || !model.is_empty() {
            if let Some(profile) = crate::profile::load_profile(
                manufacturer,
                model,
                &cdi,
                &app_handle,
                &state.profiles,
            ).await {
                let report = crate::profile::annotate_tree(&mut tree, &profile, &cdi);
                let shared_daughterboards = crate::profile::load_shared_daughterboards(&app_handle).await;
                let connector_profile_outcome = crate::profile::build_connector_profile_with_diagnostics(
                    &node_id,
                    &profile,
                    shared_daughterboards.as_ref(),
                    &cdi,
                );
                tree.connector_profile = connector_profile_outcome.profile;
                tree.connector_profile_warning = connector_profile_outcome.warning;
                eprintln!(
                    "[profile] {} — {} event roles, {} rules applied, {} warnings",
                    node_id,
                    report.event_roles_applied,
                    report.rules_applied,
                    report.warnings.len()
                );
            }
        }
    }

    // Store in proxy
    if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        let _ = proxy.set_config_tree(tree.clone()).await;
    }

    Ok(tree)
}

/// A single memory-read unit produced by [`build_read_plan`].
///
/// Large CDI elements (> 64 bytes) are split into 64-byte chunks; each chunk
/// becomes one `ReadItem`.
struct ReadItem {
    orig_index: usize,
    absolute_address: u32,
    size: u32,
    space: u8,
    /// Full declared size of the element (equals `size` for non-split elements).
    element_total_size: u32,
}

/// Output of [`build_read_plan`].
struct ReadPlan {
    /// Flat list of read-ready items (large elements split into 64-byte chunks).
    items: Vec<ReadItem>,
    /// Groups of `items` indices to read in a single `read_memory_timed` call.
    batches: Vec<Vec<usize>>,
    /// Original element indices for elements that were split into multiple chunks.
    multi_chunk_indices: std::collections::BTreeSet<usize>,
    /// Number of elements whose size could not be determined (already counted as errors).
    invalid_element_count: usize,
}

/// Build an optimised read plan from a list of CDI elements.
///
/// Large elements (> 64 bytes) are split into 64-byte chunks.  All items are
/// sorted by `(space, address)` and then packed into batches: elements that share
/// an address space and whose combined span fits within 64 bytes are merged into
/// a single batch read (gap bytes are read and discarded).
fn build_read_plan(all_elements: &[ElementWithAddress<'_>]) -> ReadPlan {
    let mut multi_chunk_indices = std::collections::BTreeSet::new();
    let mut items: Vec<ReadItem> = Vec::new();
    let mut invalid_element_count = 0;

    for (idx, ewa) in all_elements.iter().enumerate() {
        match get_element_size(ewa.element) {
            Ok(s) if s <= 64 => {
                items.push(ReadItem {
                    orig_index: idx,
                    absolute_address: ewa.absolute_address(),
                    size: s,
                    space: ewa.space,
                    element_total_size: s,
                });
            }
            Ok(s) => {
                multi_chunk_indices.insert(idx);
                let mut offset = 0u32;
                while offset < s {
                    let chunk_size = std::cmp::min(64, s - offset);
                    items.push(ReadItem {
                        orig_index: idx,
                        absolute_address: ewa.absolute_address() + offset,
                        size: chunk_size,
                        space: ewa.space,
                        element_total_size: s,
                    });
                    offset += chunk_size;
                }
            }
            Err(e) => {
                invalid_element_count += 1;
                eprintln!("Failed to get element size for {}: {}", ewa.name, e);
            }
        }
    }

    items.sort_by_key(|item| (item.space, item.absolute_address));

    let mut batches: Vec<Vec<usize>> = Vec::new();
    {
        let mut current_batch: Vec<usize> = Vec::new();
        let mut batch_start_addr: u32 = 0;
        let mut batch_end_addr: u32 = 0;
        let mut batch_space: u8 = 0;

        for (i, item) in items.iter().enumerate() {
            // An item fits in the current batch when it shares the same address space
            // and the span from batch start to this item's end fits in 64 bytes.
            let fits = !current_batch.is_empty()
                && item.space == batch_space
                && (item.absolute_address + item.size - batch_start_addr) <= 64;

            if fits {
                let item_end = item.absolute_address + item.size;
                if item_end > batch_end_addr {
                    batch_end_addr = item_end;
                }
                current_batch.push(i);
            } else {
                if !current_batch.is_empty() {
                    batches.push(std::mem::take(&mut current_batch));
                }
                batch_start_addr = item.absolute_address;
                batch_end_addr = item.absolute_address + item.size;
                batch_space = item.space;
                current_batch.push(i);
            }
        }
        if !current_batch.is_empty() {
            batches.push(current_batch);
        }
    }

    ReadPlan { items, batches, multi_chunk_indices, invalid_element_count }
}

/// Maximum number of continuation reads issued when a node returns fewer
/// bytes than requested (LCC spec permits short replies).
const MAX_CONTINUATIONS: u32 = 5;

/// Issue continuation reads to fill a short memory config reply.
///
/// LCC-spec-compliant nodes may return 1..N bytes when N bytes are requested.
/// Embedded nodes (e.g. RR-Cirkits TowerLCC) commonly have limited response
/// buffers and routinely return fewer bytes.  This helper issues follow-up
/// reads advancing by actual bytes received each time — matching the pattern
/// in OpenLCB_Java `MemorySpaceCache.handleReadData()`.
///
/// `read_fn` is called as `read_fn(address, size) -> Result<Vec<u8>>` for each
/// continuation chunk.  Returns the number of continuation reads issued.
async fn fill_short_reply<F, Fut>(
    data: &mut Vec<u8>,
    batch_start_addr: u32,
    batch_total_size: u32,
    mut read_fn: F,
) -> u32
where
    F: FnMut(u32, u8) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, String>>,
{
    let mut continuations = 0u32;
    while (data.len() as u32) < batch_total_size && continuations < MAX_CONTINUATIONS {
        let received = data.len() as u32;
        let remaining = batch_total_size - received;
        let cont_addr = batch_start_addr + received;
        let cont_size = std::cmp::min(remaining, 64) as u8;

        match read_fn(cont_addr, cont_size).await {
            Ok(cont_data) => {
                if cont_data.is_empty() {
                    break;
                }
                data.extend_from_slice(&cont_data);
            }
            Err(_) => {
                break;
            }
        }
        continuations += 1;
    }
    continuations
}

/// Attempt a single `read_memory_timed` call, retrying up to twice when the
/// node times out.  A 200 ms pause before each retry lets the node drain any
/// stale datagram-ack state from a previous dropped TCP frame.
///
/// Returns `(result, retry_count)`.
async fn read_memory_with_retry(
    connection: &tokio::sync::Mutex<lcc_rs::LccConnection>,
    alias: u16,
    space: u8,
    address: u32,
    size: u8,
    timeout_ms: u64,
) -> (lcc_rs::Result<(Vec<u8>, lcc_rs::MemoryReadTiming)>, u32) {
    const MAX_RETRIES: u32 = 2;
    let mut result;
    let mut retry_count = 0u32;
    loop {
        {
            let mut conn = connection.lock().await;
            result = conn
                .read_memory_timed(alias, space, address, size, timeout_ms)
                .await;
        }
        match &result {
            Ok(_) => break,
            Err(e) if retry_count < MAX_RETRIES && e.to_string().contains("Timeout") => {
                retry_count += 1;
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
            Err(_) => break,
        }
    }
    (result, retry_count)
}

/// Read all configuration values from a node with progress tracking (T051)
#[tauri::command]
pub async fn read_all_config_values(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    node_id: String,
    timeout_ms: Option<u64>,
    node_index: Option<usize>,
    total_nodes: Option<usize>,
) -> Result<ReadAllConfigValuesResponse, String> {
    use std::sync::atomic::Ordering;
    use std::time::Instant;
    
    let start_time = Instant::now();
    let timeout = timeout_ms.unwrap_or(2000); // 2 second timeout per element
    let node_idx = node_index.unwrap_or(0);
    let total_node_count = total_nodes.unwrap_or(1);
    
    // Reset cancellation flag
    state.config_read_cancel.store(false, Ordering::Relaxed);
    
    // Parse node ID
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("Invalid node ID: {}", e))?;
    
    // Get node info from proxy
    let proxy = state.node_registry.get(&parsed_node_id).await
        .ok_or_else(|| format!("Node not found: {}", node_id))?;
    let node = proxy.get_snapshot().await
        .map_err(|e| format!("Failed to get node snapshot: {}", e))?;
    
    // Get display name for progress messages (T049)
    let node_name = get_node_display_name(&node);
    
    // Get CDI from cache
    let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await?;
    
    // Extract all elements with addresses (T050)
    let all_elements = extract_all_elements_with_addresses(&cdi);
    let total_count = all_elements.len();
    
    if total_count == 0 {
        return Ok(ReadAllConfigValuesResponse {
            node_id: node_id.clone(),
            values: HashMap::new(),
            total_elements: 0,
            successful_reads: 0,
            failed_reads: 0,
            duration_ms: start_time.elapsed().as_millis() as u64,
            abort_error: None,
        });
    }
    
    // Emit starting event (T052)
    let _ = app_handle.emit("config-read-progress", ReadProgressUpdate {
        total_nodes: total_node_count,
        current_node_index: node_idx,
        current_node_name: node_name.clone(),
        current_node_id: node_id.clone(),
        total_elements: total_count,
        elements_read: 0,
        elements_failed: 0,
        percentage: 0,
        status: ProgressStatus::ReadingNode { node_name: node_name.clone() },
    });
    
    // Get connection
    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);
    
    let alias = node.alias.value();
    let mut values = HashMap::new();
    let mut success_count = 0;
    let mut error_count = 0;
    let mut abort_error: Option<String> = None;

    // Spec 007: collect raw bytes keyed by (space, absolute address) for tree merging.
    // Address-only keys can collide across spaces (e.g. LT-50 status vs macro offsets).
    let mut raw_data_by_space_address: HashMap<(u8, u32), Vec<u8>> = HashMap::new();

    // --- Build the read plan: size every element, split large ones, group into batches ---
    let plan = build_read_plan(&all_elements);
    let sized_items = plan.items;
    let batches = plan.batches;
    let multi_chunk_elements = plan.multi_chunk_indices;
    error_count += plan.invalid_element_count;

    let total_batches = batches.len();
    let total_chunks = sized_items.len();
    let logical_elements = all_elements.len();
    eprintln!(
        "[CDI] {} elements ({} chunks) grouped into {} read batches",
        logical_elements, total_chunks, total_batches
    );

    // --- Issue one read_memory per batch, slice individual element values from reply ---
    let mut elements_processed: usize = 0;
    // Tracks orig_index of any element whose chunk batch failed; used in
    // the multi-chunk assembly pass to skip partially-failed elements.
    let mut failed_orig_indices: std::collections::HashSet<usize> = Default::default();
    // Per-batch diagnostic timing records (Phase 4c instrumentation).
    let mut batch_stats: Vec<crate::diagnostics::BatchReadStat> = Vec::with_capacity(total_batches);

    // --- Pipelined batch read: subscribe once, pipeline ACK+requests tightly ---
    // Build one descriptor per batch, then execute all reads in a single call
    // that holds the broadcast subscription and sends ACK+next request back-to-back.
    let batch_descriptors: Vec<lcc_rs::BatchReadDescriptor> = batches.iter().map(|batch| {
        let first = &sized_items[batch[0]];
        let batch_start_addr = first.absolute_address;
        let batch_end_addr: u32 = batch.iter()
            .map(|&i| sized_items[i].absolute_address + sized_items[i].size)
            .max()
            .unwrap_or(batch_start_addr);
        lcc_rs::BatchReadDescriptor {
            address_space: first.space,
            address: batch_start_addr,
            count: (batch_end_addr - batch_start_addr) as u8,
        }
    }).collect();

    let mut reader = {
        let conn = connection.lock().await;
        conn.batch_reader(alias).map_err(|e| e.to_string())?
    };
    let reads_start = Instant::now();

    for (batch_idx, batch) in batches.iter().enumerate() {
        // T054: Check for cancellation between batches
        if let Err(_) = check_cancellation(&state) {
            let _ = app_handle.emit("config-read-progress", ReadProgressUpdate {
                total_nodes: total_node_count,
                current_node_index: node_idx,
                current_node_name: node_name.clone(),
                current_node_id: node_id.clone(),
                total_elements: total_count,
                elements_read: success_count,
                elements_failed: error_count,
                percentage: (elements_processed as f32 / total_count as f32 * 100.0) as u8,
                status: ProgressStatus::Cancelled,
            });
            return Err("Operation cancelled by user".to_string());
        }

        let first = &sized_items[batch[0]];
        let batch_space = first.space;
        let batch_start_addr = first.absolute_address;
        // Span from the first element's start to the last element's end.
        // This covers any gap bytes between elements (which are read and discarded).
        let batch_end_addr: u32 = batch.iter()
            .map(|&i| sized_items[i].absolute_address + sized_items[i].size)
            .max()
            .unwrap_or(batch_start_addr);
        let batch_total_size: u32 = batch_end_addr - batch_start_addr;

        elements_processed += batch.len();

        // Emit incremental progress so the bar moves during a long read
        let _ = app_handle.emit("config-read-progress", ReadProgressUpdate {
            total_nodes: total_node_count,
            current_node_index: node_idx,
            current_node_name: node_name.clone(),
            current_node_id: node_id.clone(),
            total_elements: total_count,
            elements_read: success_count,
            elements_failed: error_count,
            percentage: (elements_processed as f32 / total_count as f32 * 100.0) as u8,
            status: ProgressStatus::ReadingNode { node_name: node_name.clone() },
        });

        // Perform the read, then emit updated progress.
        let read_t_ms = reads_start.elapsed().as_millis() as u64;
        let batch_result = reader.read_next(&batch_descriptors[batch_idx], timeout).await;
        let response_data = match &batch_result.data {
            Ok(initial_data) => {
                let timing = batch_result.timing.as_ref().unwrap();
                let mut data = initial_data.clone();

                crate::bwlog!(state.inner(),
                    "[config-read] t={}ms {} @{:#010x}+{} space={:#04x}: ok latency={}ms frames={} total={}ms",
                    read_t_ms,
                    node_name, batch_start_addr, batch_total_size, batch_space,
                    timing.first_frame_latency_ms, timing.frame_count, timing.total_duration_ms);

                // LCC spec allows nodes to return fewer bytes than requested.
                // Embedded nodes (e.g. RR-Cirkits TowerLCC) commonly have limited
                // response buffers.  Issue continuation reads for any remaining
                // bytes, advancing by actual bytes received each time — matching
                // the pattern in OpenLCB_Java MemorySpaceCache.handleReadData().
                if (data.len() as u32) < batch_total_size {
                    let cont_node_name = node_name.clone();
                    let cont_state = state.inner().clone();
                    let continuations = fill_short_reply(
                        &mut data,
                        batch_start_addr,
                        batch_total_size,
                        |cont_addr, cont_size| {
                            let connection = connection.clone();
                            let node_name = cont_node_name.clone();
                            let state_ref = cont_state.clone();
                            async move {
                                let (cont_result, _) = read_memory_with_retry(
                                    &connection,
                                    alias,
                                    batch_space,
                                    cont_addr,
                                    cont_size,
                                    timeout,
                                ).await;
                                match cont_result {
                                    Ok((cont_data, _timing)) => {
                                        if !cont_data.is_empty() {
                                            crate::bwlog!(state_ref,
                                                "[config-read] {} @{:#010x}+{} space={:#04x}: \
                                                 continuation got {} bytes",
                                                node_name, cont_addr, cont_size, batch_space,
                                                cont_data.len());
                                        } else {
                                            eprintln!(
                                                "[config-read] {} @{:#010x}: continuation returned \
                                                 0 bytes, stopping",
                                                node_name, cont_addr);
                                        }
                                        Ok(cont_data)
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "[config-read] {} @{:#010x}: continuation failed: {}",
                                            node_name, cont_addr, e);
                                        Err(e.to_string())
                                    }
                                }
                            }
                        },
                    ).await;
                    if continuations > 0 {
                        crate::bwlog!(state.inner(),
                            "[config-read] {} @{:#010x}+{}: short reply filled with {} \
                             continuation read(s), final size {} bytes",
                            node_name, batch_start_addr, batch_total_size,
                            continuations, data.len());
                    }
                }

                batch_stats.push(crate::diagnostics::BatchReadStat {
                    address_space: batch_space,
                    address: batch_start_addr,
                    byte_count: batch_total_size as u8,
                    success: true,
                    error: None,
                    first_frame_latency_ms: Some(timing.first_frame_latency_ms),
                    frame_gaps_ms: timing.frame_gaps_ms.clone(),
                    frame_count: Some(timing.frame_count),
                    total_duration_ms: timing.total_duration_ms,
                });
                data
            }
            Err(err_str) => {
                crate::bwlog!(state.inner(),
                    "[config-read] {} @{:#010x}+{} space={:#04x}: FAILED {}",
                    node_name, batch_start_addr, batch_total_size, batch_space, err_str);
                batch_stats.push(crate::diagnostics::BatchReadStat {
                    address_space: batch_space,
                    address: batch_start_addr,
                    byte_count: batch_total_size as u8,
                    success: false,
                    error: Some(err_str.clone()),
                    first_frame_latency_ms: None,
                    frame_gaps_ms: vec![],
                    frame_count: None,
                    total_duration_ms: batch_result.timing.as_ref()
                        .map(|t| t.total_duration_ms).unwrap_or(0),
                });
                // Count every chunk in the batch as failed and record orig_index
                // so the assembly pass can skip partially-failed large elements.
                error_count += batch.len();
                for &i in batch {
                    failed_orig_indices.insert(sized_items[i].orig_index);
                    let ewa = &all_elements[sized_items[i].orig_index];
                    eprintln!("Failed to read element {} (batch read @{:#010x}+{}): {}",
                        ewa.name, batch_start_addr, batch_total_size, err_str);
                }
                // Abort on first unrecoverable failure — continuing would waste
                // time and produce incomplete data the UI cannot usefully display.
                abort_error = Some(format!(
                    "Configuration read failed: {err_str}. \
                     Check your connection and try again."
                ));
                break;
            }
        };

        // Slice and parse each element's bytes from the batch reply
        for &i in batch {
            let item = &sized_items[i];
            let ewa = &all_elements[item.orig_index];

            let offset_in_batch = (item.absolute_address - batch_start_addr) as usize;
            let end = offset_in_batch + item.size as usize;

            if end > response_data.len() {
                error_count += 1;
                eprintln!(
                    "Batch reply too short for element {}: need bytes [{}..{}] but reply is {} bytes",
                    ewa.name, offset_in_batch, end, response_data.len()
                );
                continue;
            }

            let item_data = &response_data[offset_in_batch..end];

            // Spec 007: record raw bytes for tree merging (also used by assembly pass).
            raw_data_by_space_address.insert((item.space, item.absolute_address), item_data.to_vec());

            // Multi-chunk elements are assembled and parsed in the pass below;
            // only parse immediately for elements that fit in a single read.
            if item.element_total_size <= 64 {
                let typed_value = match parse_config_value(ewa.element, item_data) {
                    Ok(v) => v,
                    Err(e) => {
                        error_count += 1;
                        eprintln!("Failed to parse element {}: {}", ewa.name, e);
                        continue;
                    }
                };

                let cache_key = format!("{}:{}", node_id, ewa.path.join("/"));
                values.insert(cache_key, ConfigValueWithMetadata {
                    value: typed_value,
                    memory_address: item.absolute_address,
                    address_space: item.space,
                    element_path: ewa.path.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
                success_count += 1;
            }
        }
    }

    // --- Assemble and parse multi-chunk elements ---
    // Each large element was split into 64-byte chunks above.  Now that all
    // chunks have been read into raw_data_by_space_address, concatenate them and
    // parse the complete value.
    for orig_idx in &multi_chunk_elements {
        if failed_orig_indices.contains(orig_idx) {
            // One or more chunks failed; the error was already counted in the
            // batch error handler.  Do not attempt a partial parse.
            continue;
        }
        let ewa = &all_elements[*orig_idx];
        let base_addr = ewa.absolute_address();
        let total_size = match get_element_size(ewa.element) {
            Ok(s) => s,
            Err(_) => { error_count += 1; continue; }
        };
        let mut assembled: Vec<u8> = Vec::with_capacity(total_size as usize);
        let mut ok = true;
        let mut offset = 0u32;
        while offset < total_size {
            let chunk_size = std::cmp::min(64, total_size - offset);
            let chunk_addr = base_addr + offset;
            if let Some(chunk_bytes) = raw_data_by_space_address.get(&(ewa.space, chunk_addr)) {
                assembled.extend_from_slice(chunk_bytes);
            } else {
                eprintln!("Missing chunk @{:#010x} for element {}", chunk_addr, ewa.name);
                ok = false;
                error_count += 1;
                break;
            }
            offset += chunk_size;
        }
        if ok {
            match parse_config_value(ewa.element, &assembled) {
                Ok(typed_value) => {
                    let cache_key = format!("{}:{}", node_id, ewa.path.join("/"));
                    values.insert(cache_key, ConfigValueWithMetadata {
                        value: typed_value,
                        memory_address: base_addr,
                        address_space: ewa.space,
                        element_path: ewa.path.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });
                    success_count += 1;
                }
                Err(e) => {
                    error_count += 1;
                    eprintln!("Failed to parse multi-chunk element {}: {}", ewa.name, e);
                }
            }
        }
    }
    
    // Emit completion event
    let _ = app_handle.emit("config-read-progress", ReadProgressUpdate {
        total_nodes: total_node_count,
        current_node_index: node_idx,
        current_node_name: node_name.clone(),
        current_node_id: node_id.clone(),
        total_elements: total_count,
        elements_read: success_count,
        elements_failed: error_count,
        percentage: 100,
        status: ProgressStatus::NodeComplete { 
            node_name: node_name.clone(), 
            success: error_count == 0 
        },
    });
    
    let duration = start_time.elapsed().as_millis() as u64;

    // Store EventId values in proxy
    if let Some(proxy) = state.node_registry.get(&parsed_node_id).await {
        let event_map: HashMap<String, [u8; 8]> = values.iter()
            .filter_map(|(cache_key, meta)| {
                if let ConfigValue::EventId { value: event_bytes } = meta.value {
                    let path_key = cache_key
                        .strip_prefix(&format!("{}:", node_id))
                        .unwrap_or(cache_key.as_str())
                        .to_string();
                    Some((path_key, event_bytes))
                } else {
                    None
                }
            })
            .collect();
        let event_count = event_map.len();
        let _ = proxy.merge_config_values(event_map).await;
        eprintln!("[bowties][cache] node {} — {} EventId slots cached in proxy", node_id, event_count);
    }

    // Spec 007: build (or update) the unified node tree with config values.
    {
        let proxy = state.node_registry.get(&parsed_node_id).await;
        if let Some(proxy) = &proxy {
            let mut tree = proxy.get_config_tree().await
                .ok().flatten()
                .unwrap_or_else(|| crate::node_tree::build_node_config_tree(&node_id, &cdi));
            crate::node_tree::merge_config_values_by_space(&mut tree, &raw_data_by_space_address);
            let leaf_count = crate::node_tree::count_leaves(&tree);
            eprintln!("[node_tree] node {} — tree updated with {} values ({} leaves total)",
                node_id, raw_data_by_space_address.len(), leaf_count);

            let _ = proxy.set_config_tree(tree).await;

            // Emit node-tree-updated event so the frontend can refresh.
            let _ = app_handle.emit("node-tree-updated", serde_json::json!({
                "nodeId": node_id,
                "leafCount": leaf_count,
            }));
        }
    }

    // T011: When this is the last node, run the Identify Events exchange and build the catalog.
    if node_idx + 1 == total_node_count {
        eprintln!("[bowties] Last node complete — starting Identify Events exchange");
        let build_start = std::time::Instant::now();

        // Borrow AppState as a plain reference for the async helpers.
        let state_ref: &AppState = &*state;

        let event_roles = crate::commands::bowties::query_event_roles(
            state_ref,
            125,  // ms between addressed sends
            500,  // ms collection window
        ).await;

        let nodes_snap = state.node_registry.get_all_snapshots().await;
        let node_count = nodes_snap.len();

        // Gather config values from all proxies
        let config_cache_snap: HashMap<String, HashMap<String, [u8; 8]>> = {
            let handles = state.node_registry.get_all_handles().await;
            let mut map = HashMap::new();
            for h in &handles {
                if let Ok(vals) = h.get_config_values().await {
                    if !vals.is_empty() {
                        map.insert(h.node_id.to_hex_string(), vals);
                    }
                }
            }
            map
        };

        // Spec 007: merge protocol-level event roles into every node tree via proxies.
        {
            let handles = state.node_registry.get_all_handles().await;
            for h in &handles {
                if let Ok(Some(mut tree)) = h.get_config_tree().await {
                    let nid = h.node_id.to_hex_string();
                    let path_roles = crate::node_tree::classify_leaf_roles_from_protocol(
                        &tree,
                        &event_roles,
                    );
                    if !path_roles.is_empty() {
                        crate::node_tree::merge_event_roles(&mut tree, &path_roles);
                        eprintln!("[node_tree] node {} — {} event roles merged", nid, path_roles.len());
                        let _ = h.set_config_tree(tree).await;
                    }
                }
            }
        }

        // Apply structure profiles to every node tree (must be AFTER merge_event_roles
        // so profile-declared roles take precedence over protocol-exchange heuristics).
        {
            let handles = state.node_registry.get_all_handles().await;
            for h in &handles {
                let nid = h.node_id.to_hex_string();
                let cdi = match get_cdi_from_cache(&nid, &app_handle, &state).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                if let Some(identity) = &cdi.identification {
                    let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
                    let model = identity.model.as_deref().unwrap_or("");
                    if !manufacturer.is_empty() || !model.is_empty() {
                        if let Some(profile) = crate::profile::load_profile(
                            manufacturer,
                            model,
                            &cdi,
                            &app_handle,
                            &state.profiles,
                        ).await {
                            if let Ok(Some(mut tree)) = h.get_config_tree().await {
                                let report = crate::profile::annotate_tree(&mut tree, &profile, &cdi);
                                eprintln!(
                                    "[profile] {} — {} event roles, {} rules applied, {} warnings",
                                    nid,
                                    report.event_roles_applied,
                                    report.rules_applied,
                                    report.warnings.len()
                                );
                                let _ = h.set_config_tree(tree).await;
                            }
                        }
                    }
                }
            }

            // Now that profiles are fully applied, notify the frontend to refresh every
            // tree so the annotated event roles and element names are visible immediately
            // (e.g. in ElementPicker search) without requiring the user to first expand
            // a node to trigger a lazy reload.
            for h in &handles {
                let nid = h.node_id.to_hex_string();
                let leaf_count = h.get_config_tree().await
                    .ok().flatten()
                    .map(|t| crate::node_tree::count_leaves(&t))
                    .unwrap_or(0);
                let _ = app_handle.emit("node-tree-updated", serde_json::json!({
                    "nodeId": nid,
                    "leafCount": leaf_count,
                }));
            }
        }

        // Build the bowtie catalog AFTER profile annotation so profile-declared event roles
        // feed the same-node classification step (FR-016, FR-017).
        //
        // Collect profile_group_roles: all EventId leaves with a non-Ambiguous role from the
        // now-annotated trees, keyed by "{node_id}:{element_path.join("/")}".
        let profile_group_roles: std::collections::HashMap<String, lcc_rs::EventRole> = {
            let handles = state.node_registry.get_all_handles().await;
            let mut map = std::collections::HashMap::new();
            for h in &handles {
                let nid = h.node_id.to_hex_string();
                if let Ok(Some(tree)) = h.get_config_tree().await {
                    for leaf in crate::node_tree::collect_event_id_leaves(&tree) {
                        if let Some(role) = leaf.event_role {
                            if role != lcc_rs::EventRole::Ambiguous {
                                let key = format!("{}:{}", nid, leaf.path.join("/"));
                                map.insert(key, role);
                            }
                        }
                    }
                }
            }
            map
        };

        let catalog = crate::commands::bowties::build_bowtie_catalog(
            &nodes_snap,
            &event_roles,
            &config_cache_snap,
            Some(&profile_group_roles),
        );

        eprintln!(
            "[bowties] Catalog built in {} ms: {} bowties from {} nodes",
            build_start.elapsed().as_millis(),
            catalog.bowties.len(),
            node_count,
        );

        // Store catalog in AppState.
        *state.bowties_catalog.write().await = Some(catalog.clone());

        // Emit `cdi-read-complete` event to the frontend.
        let _ = app_handle.emit(
            "cdi-read-complete",
            crate::commands::bowties::CdiReadCompletePayload { catalog, node_count },
        );
    }

    // Phase 4c: Record NodeConfigReadStats in diagnostics.
    {
        let snip_name = node.snip_data.as_ref().and_then(|s| {
            if !s.user_name.is_empty() { Some(s.user_name.clone()) }
            else if !s.model.is_empty() { Some(s.model.clone()) }
            else { None }
        });
        let snip = node.snip_data.as_ref().map(|s| crate::diagnostics::SnipInfo {
            manufacturer: s.manufacturer.clone(),
            model: s.model.clone(),
            hardware_version: s.hardware_version.clone(),
            software_version: s.software_version.clone(),
            user_name: s.user_name.clone(),
            user_description: s.user_description.clone(),
        });
        let successful_batches = batch_stats.iter().filter(|b| b.success).count();
        let failed_batches = batch_stats.iter().filter(|b| !b.success).count();
        let stats_entry = crate::diagnostics::NodeConfigReadStats {
            node_id: node_id.clone(),
            snip_name,
            snip,
            total_batches: batch_stats.len(),
            successful_batches,
            failed_batches,
            total_elements: total_count,
            successful_elements: success_count,
            failed_elements: error_count,
            total_duration_ms: duration,
            batch_stats,
        };
        crate::bwlog!(state.inner(),
            "[config-read] {} complete: {}/{} elements ok, {}/{} batches ok, {}ms clock",
            node_id, success_count, total_count,
            stats_entry.successful_batches, stats_entry.total_batches, duration);
        state.diag_stats.write().await.config_reads.insert(node_id.clone(), stats_entry);
    }

    Ok(ReadAllConfigValuesResponse {
        node_id: node_id.clone(),
        values,
        total_elements: total_count,
        successful_reads: success_count,
        failed_reads: error_count,
        duration_ms: duration,
        abort_error,
    })
}

/// Cancel ongoing configuration reading operation
#[tauri::command]
pub async fn cancel_config_reading(state: tauri::State<'_, AppState>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.config_read_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

/// Cancel an in-progress CDI download
#[tauri::command]
pub async fn cancel_cdi_download(state: tauri::State<'_, AppState>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.cdi_download_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

// ============================================================================
// get_card_elements Command (Feature 005-config-sidebar-view)
// ============================================================================

/// Leaf configuration field within a card (part of CardElementTree).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardField {
    pub element_path: Vec<String>,
    pub name: String,
    pub description: Option<String>,
    pub data_type: String,
    pub memory_address: u32,
    pub size_bytes: u32,
    pub default_value: Option<String>,
    pub address_space: u8,
}

/// Sub-group within a card, rendered inline and fully expanded per FR-011.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSubGroup {
    pub name: String,
    pub description: Option<String>,
    pub group_path: Vec<String>,
    pub fields: Vec<CardField>,
    pub sub_groups: Vec<CardSubGroup>,
    /// Original CDI replication count. > 1 means this is one instance of a
    /// replicated group and should be rendered as a collapsible accordion.
    /// == 1 means a non-replicated group — render inline (always visible).
    pub replication: u32,
}

/// Full recursive element tree returned by get_card_elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardElementTree {
    pub group_name: Option<String>,
    pub group_description: Option<String>,
    pub fields: Vec<CardField>,
    pub sub_groups: Vec<CardSubGroup>,
}

/// Parse an "elem:N" or "elem:N#I" path step.
/// Returns (element_index, optional_instance_1based).
fn parse_elem_path_step(step: &str) -> Result<(usize, Option<u32>), String> {
    let index_part = step
        .strip_prefix("elem:")
        .ok_or_else(|| format!("InvalidPath: expected 'elem:N' or 'elem:N#I', got '{}'", step))?;
    if let Some(hash_pos) = index_part.find('#') {
        let idx_str = &index_part[..hash_pos];
        let instance_str = &index_part[hash_pos + 1..];
        let idx = idx_str
            .parse::<usize>()
            .map_err(|_| format!("InvalidPath: bad element index in '{}'", step))?;
        let instance = instance_str
            .parse::<u32>()
            .map_err(|_| format!("InvalidPath: bad instance number in '{}'", step))?;
        Ok((idx, Some(instance)))
    } else {
        let idx = index_part
            .parse::<usize>()
            .map_err(|_| format!("InvalidPath: bad element index in '{}'", step))?;
        Ok((idx, None))
    }
}

/// Total byte footprint of a DataElement (offset skip + its content size).
/// This is the number of cursor bytes the element occupies in its parent context.
fn card_element_footprint(element: &lcc_rs::cdi::DataElement) -> i32 {
    use lcc_rs::cdi::DataElement;
    match element {
        DataElement::Group(g) => g.offset + g.calculate_size() * g.replication as i32,
        DataElement::Int(e) => e.offset + e.size as i32,
        DataElement::String(s) => s.offset + s.size as i32,
        DataElement::EventId(e) => e.offset + 8,
        DataElement::Float(e) => e.offset + 4,
        DataElement::Action(e) => e.offset + 1,
        DataElement::Blob(b) => b.offset + b.size as i32,
    }
}

/// Navigate element slice following `path`, tracking absolute address.
/// `base_offset`: absolute address of the beginning of this elements slice.
/// Returns (&Group, absolute_base_address_of_that_group_instance).
fn navigate_elements_to_group<'a>(
    elements: &'a [lcc_rs::cdi::DataElement],
    path: &[String],
    base_offset: i32,
) -> Result<(&'a lcc_rs::cdi::Group, i32), String> {
    use lcc_rs::cdi::DataElement;

    if path.is_empty() {
        return Err("InvalidPath: unexpected end of path".to_string());
    }

    let (elem_idx, opt_instance) = parse_elem_path_step(&path[0])?;

    if elem_idx >= elements.len() {
        return Err(format!(
            "InvalidPath: element index {} out of range (len={})",
            elem_idx,
            elements.len()
        ));
    }

    // Accumulate cursor to element elem_idx by summing footprints of preceding elements
    let cursor_before: i32 = elements[..elem_idx]
        .iter()
        .map(card_element_footprint)
        .sum();

    let element = &elements[elem_idx];
    let group = match element {
        DataElement::Group(g) => g,
        _ => {
            return Err(format!(
                "InvalidPath: element at index {} is not a group",
                elem_idx
            ))
        }
    };

    // Group content start: base_offset + cursor_before + group.offset (skip before group)
    let group_content_start = base_offset + cursor_before + group.offset;

    // Apply replication instance offset (1-based input → 0-based math)
    let stride = group.calculate_size();
    let instance_0based = opt_instance.map(|i| (i as i32) - 1).unwrap_or(0);
    let instance_base = group_content_start + instance_0based * stride;

    // If this is the last step in the path, we found the target group
    if path.len() == 1 {
        return Ok((group, instance_base));
    }

    // Otherwise recurse into this group's elements
    navigate_elements_to_group(&group.elements, &path[1..], instance_base)
}

/// Recursively collect CardField and CardSubGroup entries from an element slice.
/// `base_address`: absolute address at which the first element of this slice starts.
fn collect_fields_and_subgroups(
    elements: &[lcc_rs::cdi::DataElement],
    base_address: i32,
    address_space: u8,
    parent_path: &[String],
) -> (Vec<CardField>, Vec<CardSubGroup>) {
    use lcc_rs::cdi::DataElement;

    let mut fields = Vec::new();
    let mut sub_groups = Vec::new();
    let mut cursor: i32 = 0;

    for (i, element) in elements.iter().enumerate() {
        match element {
            DataElement::Group(g) => {
                cursor += g.offset;
                let sub_base = base_address + cursor;
                let stride = g.calculate_size();

                // FR-011: sub-groups within a card are rendered inline, fully expanded.
                // Each replication instance becomes a separate CardSubGroup.
                for inst in 0..g.replication {
                    let inst_base = sub_base + inst as i32 * stride;
                    let inst_path: Vec<String> = if g.replication > 1 {
                        let mut p = parent_path.to_vec();
                        p.push(format!("elem:{}#{}", i, inst + 1));
                        p
                    } else {
                        let mut p = parent_path.to_vec();
                        p.push(format!("elem:{}", i));
                        p
                    };
                    let (sub_fields, deeper_sub_groups) =
                        collect_fields_and_subgroups(&g.elements, inst_base, address_space, &inst_path);
                    sub_groups.push(CardSubGroup {
                        name: g.name.clone().unwrap_or_else(|| format!("Group {}", i)),
                        description: g.description.clone(),
                        group_path: inst_path,
                        fields: sub_fields,
                        sub_groups: deeper_sub_groups,
                        replication: g.replication,
                    });
                }
                cursor += g.calculate_size() * g.replication as i32;
            }
            DataElement::Int(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                cursor += e.size as i32;
                let mut elem_path = parent_path.to_vec();
                elem_path.push(format!("elem:{}", i));
                fields.push(CardField {
                    element_path: elem_path,
                    name: e.name.clone().unwrap_or_else(|| format!("Int {}", i)),
                    description: e.description.clone(),
                    data_type: "int".to_string(),
                    memory_address: addr,
                    size_bytes: e.size as u32,
                    default_value: e.default.map(|v| v.to_string()),
                    address_space,
                });
            }
            DataElement::String(s) => {
                cursor += s.offset;
                let addr = (base_address + cursor) as u32;
                cursor += s.size as i32;
                let mut elem_path = parent_path.to_vec();
                elem_path.push(format!("elem:{}", i));
                fields.push(CardField {
                    element_path: elem_path,
                    name: s.name.clone().unwrap_or_else(|| format!("String {}", i)),
                    description: s.description.clone(),
                    data_type: "string".to_string(),
                    memory_address: addr,
                    size_bytes: s.size as u32,
                    default_value: None,
                    address_space,
                });
            }
            DataElement::EventId(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                cursor += 8;
                let mut elem_path = parent_path.to_vec();
                elem_path.push(format!("elem:{}", i));
                fields.push(CardField {
                    element_path: elem_path,
                    name: e.name.clone().unwrap_or_else(|| format!("EventId {}", i)),
                    description: e.description.clone(),
                    data_type: "eventid".to_string(),
                    memory_address: addr,
                    size_bytes: 8,
                    default_value: None,
                    address_space,
                });
            }
            DataElement::Float(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                cursor += 4;
                let mut elem_path = parent_path.to_vec();
                elem_path.push(format!("elem:{}", i));
                fields.push(CardField {
                    element_path: elem_path,
                    name: e.name.clone().unwrap_or_else(|| format!("Float {}", i)),
                    description: e.description.clone(),
                    data_type: "float".to_string(),
                    memory_address: addr,
                    size_bytes: 4,
                    default_value: None,
                    address_space,
                });
            }
            DataElement::Action(e) => {
                cursor += e.offset;
                let addr = (base_address + cursor) as u32;
                cursor += 1;
                let mut elem_path = parent_path.to_vec();
                elem_path.push(format!("elem:{}", i));
                fields.push(CardField {
                    element_path: elem_path,
                    name: e.name.clone().unwrap_or_else(|| format!("Action {}", i)),
                    description: e.description.clone(),
                    data_type: "action".to_string(),
                    memory_address: addr,
                    size_bytes: 1,
                    default_value: None,
                    address_space,
                });
            }
            DataElement::Blob(b) => {
                cursor += b.offset;
                let addr = (base_address + cursor) as u32;
                cursor += b.size as i32;
                let mut elem_path = parent_path.to_vec();
                elem_path.push(format!("elem:{}", i));
                fields.push(CardField {
                    element_path: elem_path,
                    name: b.name.clone().unwrap_or_else(|| format!("Blob {}", i)),
                    description: b.description.clone(),
                    data_type: "blob".to_string(),
                    memory_address: addr,
                    size_bytes: b.size as u32,
                    default_value: None,
                    address_space,
                });
            }
        }
    }

    (fields, sub_groups)
}

/// Navigate CDI to the group at `group_path` and build a CardElementTree.
/// This is the pure, testable core of get_card_elements.
fn navigate_and_build_card_tree(
    cdi: &lcc_rs::cdi::Cdi,
    group_path: &[String],
) -> Result<CardElementTree, String> {
    if group_path.len() < 2 {
        return Err(
            "InvalidPath: group_path must have at least a segment step and one element step"
                .to_string(),
        );
    }

    // Parse segment index from "seg:N"
    let seg_id = &group_path[0];
    let seg_idx = seg_id
        .strip_prefix("seg:")
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or_else(|| format!("InvalidPath: segment id must be 'seg:N', got '{}'", seg_id))?;

    let segment = cdi
        .segments
        .get(seg_idx)
        .ok_or_else(|| format!("InvalidPath: segment index {} out of range", seg_idx))?;

    let base_offset = segment.origin as i32;
    let address_space = segment.space;

    let (group, group_base) =
        navigate_elements_to_group(&segment.elements, &group_path[1..], base_offset)?;

    let (fields, sub_groups) =
        collect_fields_and_subgroups(&group.elements, group_base, address_space, group_path);

    Ok(CardElementTree {
        group_name: group.name.clone(),
        group_description: group.description.clone(),
        fields,
        sub_groups,
    })
}

/// Return the full recursive element tree for a top-level CDI group.
///
/// Used by ElementCard to render the card body with all leaf fields and sub-groups
/// inline (FR-011). Replaces multiple sequential get_column_items calls (SC-002).
///
/// # Errors
/// - `NodeNotFound: ...` — node not in discovered list
/// - `CdiNotRetrieved: ...` — CDI not yet fetched for node
/// - `InvalidPath: ...` — group_path does not resolve to a group in the CDI
/// - `ParseError: ...` — CDI XML could not be parsed
#[tauri::command]
pub async fn get_card_elements(
    node_id: String,
    group_path: Vec<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<CardElementTree, String> {
    let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await?;
    navigate_and_build_card_tree(&cdi, &group_path)
}

// ============================================================================
// Spec 007: Write Configuration Value Commands
// ============================================================================

/// Response returned by `write_config_value`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResponse {
    pub address: u32,
    pub space: u8,
    pub success: bool,
    pub error_code: Option<u16>,
    pub error_message: Option<String>,
    pub retry_count: u32,
}

/// Write a raw byte array to a node's configuration memory.
///
/// # Arguments
/// * `node_id`  — dotted-hex node identifier (e.g. `"02.01.57.00.00.01"`)
/// * `address`  — absolute memory address within the address space
/// * `space`    — address space byte (e.g. `0xFD` for Configuration)
/// * `data`     — raw bytes to write (1–64 bytes per call)
#[tauri::command]
pub async fn write_config_value(
    state: tauri::State<'_, AppState>,
    node_id: String,
    address: u32,
    space: u8,
    data: Vec<u8>,
) -> Result<WriteResponse, String> {
    // Parse node ID
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("Invalid node ID: {}", e))?;

    // Get connection arc
    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);

    // Get node alias from proxy
    let proxy = state.node_registry.get(&parsed_node_id).await
        .ok_or_else(|| format!("Node not found: {}", node_id))?;
    let alias = proxy.alias;

    // Perform write
    let mut conn = connection.lock().await;
    match conn.write_memory(alias, space, address, &data).await {
        Ok(()) => Ok(WriteResponse {
            address,
            space,
            success: true,
            error_code: None,
            error_message: None,
            retry_count: 0,
        }),
        Err(e) => Ok(WriteResponse {
            address,
            space,
            success: false,
            error_code: None,
            error_message: Some(e.to_string()),
            retry_count: 0,
        }),
    }
}

/// Send an Update Complete datagram to a node after writing configuration.
///
/// Per OpenLCB S-9.7.4.2 §4.23 — nodes use this as a signal to reload
/// their configuration from memory and apply changes.
#[tauri::command]
pub async fn send_update_complete(
    state: tauri::State<'_, AppState>,
    node_id: String,
) -> Result<(), String> {
    // Parse node ID
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("Invalid node ID: {}", e))?;

    // Get connection arc
    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);

    // Get node alias from proxy
    let proxy = state.node_registry.get(&parsed_node_id).await
        .ok_or_else(|| format!("Node not found: {}", node_id))?;
    let alias = proxy.alias;

    // Send update complete
    let mut conn = connection.lock().await;
    conn.send_update_complete(alias)
        .await
        .map_err(|e| format!("Failed to send update complete: {}", e))
}

// ── Modified value commands ───────────────────────────────────────────────────

/// Set a modified (pending) value on a leaf in the in-memory tree.
///
/// The modification is stored alongside the committed value so both are
/// available for display and catalog building.  If the new value matches
/// the committed value the modification is automatically cleared (revert).
///
/// Emits `node-tree-updated` so the frontend reactively picks up the change.
#[tauri::command]
pub async fn set_modified_value(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    node_id: String,
    address: u32,
    space: u8,
    value: crate::node_tree::ConfigValue,
) -> Result<bool, String> {
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;
    let proxy = state.node_registry.get(&parsed_node_id).await
        .ok_or_else(|| format!("No proxy for node {}", node_id))?;

    // Get the tree from the proxy, falling back to building from CDI cache.
    // This handles the "online with layout" case where layout trees exist only
    // in the frontend nodeTreeStore — the proxy has no tree until the CDI is
    // explicitly re-read from the live node.
    let mut tree = match proxy.get_config_tree().await {
        Ok(Some(t)) => t,
        _ => {
            let cdi = get_cdi_from_cache(&node_id, &app_handle, &state).await
                .map_err(|e| format!(
                    "No tree loaded and CDI not available for node {}: {}",
                    node_id, e
                ))?;
            let mut t = crate::node_tree::build_node_config_tree(&node_id, &cdi);
            if let Some(identity) = &cdi.identification {
                let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
                let model = identity.model.as_deref().unwrap_or("");
                if !manufacturer.is_empty() || !model.is_empty() {
                    if let Some(profile) = crate::profile::load_profile(
                        manufacturer, model, &cdi, &app_handle, &state.profiles,
                    ).await {
                        crate::profile::annotate_tree(&mut t, &profile, &cdi);
                    }
                }
            }
            // Populate config values from the layout snapshot so that other
            // segments don't show empty values after the frontend refreshes
            // from node-tree-updated.  This mirrors what build_offline_node_tree
            // does during layout hydration.
            if let Some(context) = state.active_layout.read().await.as_ref() {
                let base_file = std::path::Path::new(&context.root_path);
                if let Ok(companion_dir) =
                    crate::layout::io::derive_companion_dir_path(base_file)
                {
                    let nodes_dir = companion_dir.join("nodes");
                    let canonical = node_id.replace('.', "").to_uppercase();
                    let snap_path =
                        crate::layout::io::derive_node_file_path(&nodes_dir, &canonical);
                    if let Ok(snapshot) = crate::layout::io::read_yaml_file::<
                        crate::layout::node_snapshot::NodeSnapshot,
                    >(&snap_path)
                    {
                        crate::node_tree::merge_snapshot_path_values(
                            &mut t,
                            &snapshot.config,
                        );
                    }
                }
            }
            let _ = proxy.set_config_tree(t.clone()).await;
            t
        }
    };

    let found = crate::node_tree::set_modified_value(&mut tree, space, address, value);
    if !found {
        return Err(format!(
            "Leaf not found at space={}, address={} in node {}",
            space, address, node_id
        ));
    }

    // Also update config values in proxy for EventId modifications so the
    // bowtie catalog builder sees them.
    for leaf_info in crate::node_tree::collect_event_id_leaves(&tree) {
        if leaf_info.address == address && leaf_info.space == space {
            if let Some(bytes) = leaf_info.value {
                let mut update = HashMap::new();
                update.insert(leaf_info.path.join("/"), bytes);
                let _ = proxy.merge_config_values(update).await;
            }
            break;
        }
    }

    // Store updated tree back to proxy
    let _ = proxy.set_config_tree(tree).await;

    let _ = app_handle.emit(
        "node-tree-updated",
        serde_json::json!({ "nodeId": node_id }),
    );

    Ok(found)
}

/// Discard all modified values across all loaded trees (or for a specific node).
///
/// Emits `node-tree-updated` for each affected node.
#[tauri::command]
pub async fn discard_modified_values(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    node_id: Option<String>,
) -> Result<u32, String> {
    let mut count = 0u32;

    if let Some(nid) = node_id {
        // Discard for a specific node
        let parsed = lcc_rs::NodeID::from_hex_string(&nid)
            .map_err(|e| format!("InvalidNodeId: {}", e))?;
        if let Some(proxy) = state.node_registry.get(&parsed).await {
            if let Ok(Some(mut tree)) = proxy.get_config_tree().await {
                if crate::node_tree::has_modified_values(&tree) {
                    crate::node_tree::discard_all_modified(&mut tree);
                    count += 1;

                    // Rebuild config values from committed values
                    let rebuilt: HashMap<String, [u8; 8]> = crate::node_tree::collect_event_id_leaves(&tree)
                        .into_iter().filter_map(|l| l.value.map(|v| (l.path.join("/"), v)))
                        .collect();
                    let _ = proxy.set_config_values(rebuilt).await;
                    let _ = proxy.set_config_tree(tree).await;

                    let _ = app_handle.emit(
                        "node-tree-updated",
                        serde_json::json!({ "nodeId": nid }),
                    );
                }
            }
        }
    } else {
        // Discard across all nodes
        let handles = state.node_registry.get_all_handles().await;
        for h in &handles {
            if let Ok(Some(mut tree)) = h.get_config_tree().await {
                if crate::node_tree::has_modified_values(&tree) {
                    crate::node_tree::discard_all_modified(&mut tree);
                    count += 1;

                    // Rebuild config values from committed values
                    let rebuilt: HashMap<String, [u8; 8]> = crate::node_tree::collect_event_id_leaves(&tree)
                        .into_iter().filter_map(|l| l.value.map(|v| (l.path.join("/"), v)))
                        .collect();
                    let _ = h.set_config_values(rebuilt).await;
                    let _ = h.set_config_tree(tree).await;

                    let nid = h.node_id.to_hex_string();
                    let _ = app_handle.emit(
                        "node-tree-updated",
                        serde_json::json!({ "nodeId": nid }),
                    );
                }
            }
        }
    }

    Ok(count)
}

/// Write all pending modifications to their respective nodes.
///
/// Iterates every tree, collects leaves with `write_state == Dirty | Error`,
/// writes each to the network, updates states, and emits events.
/// Returns the number of successful + failed writes.
#[tauri::command]
pub async fn write_modified_values(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<WriteModifiedResult, String> {
    // Collect all modifications across all nodes from proxies
    let mut work: Vec<(String, crate::node_tree::ModifiedLeafInfo)> = Vec::new();
    {
        let handles = state.node_registry.get_all_handles().await;
        for h in &handles {
            if let Ok(Some(tree)) = h.get_config_tree().await {
                let nid = h.node_id.to_hex_string();
                for leaf in crate::node_tree::collect_modified_leaves(&tree) {
                    work.push((nid.clone(), leaf));
                }
            }
        }
    }

    if work.is_empty() {
        return Ok(WriteModifiedResult {
            total: 0,
            succeeded: 0,
            failed: 0,
            read_only_rejected: 0,
        });
    }

    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);

    let mut succeeded = 0u32;
    let mut failed = 0u32;
    let mut read_only_rejected = 0u32;
    let mut success_node_ids = std::collections::HashSet::new();
    // Track successfully written leaves for layout snapshot updates
    let mut written_leaves: Vec<(String, u8, u32, String)> = Vec::new(); // (nodeId, space, address, value_string)

    for (node_id, leaf_info) in &work {
        let parsed_nid = lcc_rs::NodeID::from_hex_string(node_id)
            .map_err(|e| format!("Invalid node ID: {}", e))?;
        let proxy = state.node_registry.get(&parsed_nid).await
            .ok_or_else(|| format!("Node not found: {}", node_id))?;
        let alias = proxy.alias;

        // Mark as writing via proxy
        if let Ok(Some(mut tree)) = proxy.get_config_tree().await {
            crate::node_tree::set_leaf_write_state(
                &mut tree,
                leaf_info.space,
                leaf_info.address,
                crate::node_tree::WriteState::Writing,
                None,
            );
            let _ = proxy.set_config_tree(tree).await;
        }

        // Serialize and write
        let bytes = serialize_config_value(&leaf_info.value, leaf_info.element_type, leaf_info.size);
        let mut conn = connection.lock().await;
        let result = conn.write_memory(alias, leaf_info.space, leaf_info.address, &bytes).await;
        drop(conn);

        if let Ok(Some(mut tree)) = proxy.get_config_tree().await {
            match result {
                Ok(()) => {
                    // Commit: promote modified_value → value
                    crate::node_tree::commit_leaf_value(&mut tree, leaf_info.space, leaf_info.address);
                    succeeded += 1;
                    success_node_ids.insert(node_id.clone());
                    written_leaves.push((
                        node_id.clone(),
                        leaf_info.space,
                        leaf_info.address,
                        leaf_info.value.to_snapshot_string(),
                    ));
                }
                Err(e) => {
                    let err_str = e.to_string();
                    // Error 0x1083 = OpenLCB "address is read-only / cannot be written".
                    // Revert the modification silently and mark the leaf read-only so the
                    // control is disabled for the rest of the session.
                    if err_str.contains("1083") {
                        crate::node_tree::revert_and_mark_leaf_read_only(
                            &mut tree,
                            leaf_info.space,
                            leaf_info.address,
                        );
                        read_only_rejected += 1;
                        eprintln!(
                            "[write] {} @{:#010x}: read-only rejection (0x1083), reverting '{}'",
                            node_id, leaf_info.address, leaf_info.name
                        );
                    } else {
                        crate::node_tree::set_leaf_write_state(
                            &mut tree,
                            leaf_info.space,
                            leaf_info.address,
                            crate::node_tree::WriteState::Error,
                            Some(err_str),
                        );
                        failed += 1;
                    }
                }
            }
            let _ = proxy.set_config_tree(tree).await;
        }
    }

    // Send Update Complete to each node that had successful writes
    for nid in &success_node_ids {
        let parsed_nid = match lcc_rs::NodeID::from_hex_string(nid) {
            Ok(id) => id,
            Err(_) => continue,
        };
        if let Some(proxy) = state.node_registry.get(&parsed_nid).await {
            let alias = proxy.alias;
            let mut conn = connection.lock().await;
            let _ = conn.send_update_complete(alias).await;
        }
    }

    // Emit tree updates for all affected nodes
    let affected_nodes: std::collections::HashSet<&String> = work.iter().map(|(nid, _)| nid).collect();
    for nid in affected_nodes {
        let _ = app_handle.emit(
            "node-tree-updated",
            serde_json::json!({ "nodeId": nid }),
        );
    }

    // Update node snapshot YAML files in the layout directory so the saved
    // layout reflects the values just written to hardware.
    if !written_leaves.is_empty() {
        if let Some(context) = state.active_layout.read().await.as_ref() {
            let base_file = std::path::Path::new(&context.root_path);
            if let Ok(companion_dir) = crate::layout::io::derive_companion_dir_path(base_file) {
                let nodes_dir = companion_dir.join("nodes");
                if nodes_dir.exists() {
                    let mut snapshot_cache: std::collections::HashMap<
                        String,
                        crate::layout::node_snapshot::NodeSnapshot,
                    > = std::collections::HashMap::new();

                    for (node_id, space, address, value_str) in &written_leaves {
                        let canonical = node_id.replace('.', "").to_uppercase();
                        let snapshot = snapshot_cache.entry(canonical.clone()).or_insert_with(|| {
                            let path = crate::layout::io::derive_node_file_path(&nodes_dir, &canonical);
                            crate::layout::io::read_yaml_file(&path).unwrap_or_else(|_| {
                                // Node not in layout — use a dummy that won't be saved
                                crate::layout::node_snapshot::NodeSnapshot {
                                    node_id: lcc_rs::NodeID::new([0; 6]),
                                    captured_at: String::new(),
                                    capture_status: crate::layout::node_snapshot::CaptureStatus::Complete,
                                    missing: Vec::new(),
                                    snip: crate::layout::node_snapshot::SnipSnapshot::default(),
                                    cdi_ref: crate::layout::node_snapshot::CdiReference {
                                        cache_key: String::new(),
                                        version: String::new(),
                                        fingerprint: String::new(),
                                    },
                                    config: std::collections::BTreeMap::new(),
                                    producer_identified_events: Vec::new(),
                                }
                            })
                        });

                        let offset_hex = format!("0x{:08X}", address);
                        crate::layout::node_snapshot::update_snapshot_baseline(
                            &mut snapshot.config,
                            *space,
                            &offset_hex,
                            value_str,
                        );
                    }

                    for (canonical, snapshot) in &snapshot_cache {
                        if snapshot.node_id == lcc_rs::NodeID::new([0; 6]) {
                            continue; // Skip nodes not in layout
                        }
                        let path = crate::layout::io::derive_node_file_path(&nodes_dir, canonical);
                        if let Err(e) = crate::layout::io::write_yaml_file(&path, snapshot) {
                            eprintln!("[write] Failed to update snapshot for {}: {}", canonical, e);
                        }
                    }
                }
            }
        }
    }

    Ok(WriteModifiedResult {
        total: work.len() as u32,
        succeeded,
        failed,
        read_only_rejected,
    })
}

/// Result of a `write_modified_values` operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteModifiedResult {
    pub total: u32,
    pub succeeded: u32,
    pub failed: u32,
    /// Number of writes silently reverted because the device returned 0x1083
    /// (address is read-only).  Not counted in `failed`.
    pub read_only_rejected: u32,
}

/// Check whether any loaded tree has pending modifications.
#[tauri::command]
pub async fn has_modified_values(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let handles = state.node_registry.get_all_handles().await;
    for h in &handles {
        if let Ok(Some(tree)) = h.get_config_tree().await {
            if crate::node_tree::has_modified_values(&tree) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Write an action element's trigger value to a node's memory space.
/// This is a fire-once write that bypasses the modified-value pipeline.
#[tauri::command]
pub async fn trigger_action(
    state: tauri::State<'_, AppState>,
    node_id: String,
    space: u8,
    address: u32,
    size: u32,
    value: i64,
) -> Result<(), String> {
    let parsed_nid = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("Invalid node ID: {}", e))?;
    let proxy = state.node_registry.get(&parsed_nid).await
        .ok_or_else(|| format!("Node not found: {}", node_id))?;
    let alias = proxy.alias;

    let bytes: Vec<u8> = match size {
        1 => vec![value as u8],
        2 => (value as i16).to_be_bytes().to_vec(),
        4 => (value as i32).to_be_bytes().to_vec(),
        8 => value.to_be_bytes().to_vec(),
        _ => vec![value as u8],
    };

    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);

    let mut conn = connection.lock().await;
    conn.write_memory(alias, space, address, &bytes).await
        .map_err(|e| e.to_string())
}

/// Serialize a ConfigValue to raw bytes for writing to a node.
fn f64_to_f16_bytes(v: f64) -> [u8; 2] {
    // Encode f64 as IEEE 754 half-precision (binary16) big-endian
    let bits = if v.is_nan() {
        0x7E00u16 // quiet NaN
    } else if v.is_infinite() {
        if v > 0.0 { 0x7C00u16 } else { 0xFC00u16 }
    } else if v == 0.0 {
        if v.is_sign_negative() { 0x8000u16 } else { 0x0000u16 }
    } else {
        let sign: u16 = if v < 0.0 { 1 } else { 0 };
        let abs = v.abs();
        let exp = abs.log2().floor() as i32;
        if exp > 15 {
            // overflow → infinity
            (sign << 15) | 0x7C00
        } else if exp < -24 {
            // underflow → zero
            sign << 15
        } else if exp < -14 {
            // subnormal
            let mantissa = (abs / 2.0f64.powi(-24)).round() as u16;
            (sign << 15) | mantissa
        } else {
            let biased_exp = (exp + 15) as u16;
            let mantissa = ((abs / 2.0f64.powi(exp) - 1.0) * 1024.0).round() as u16;
            (sign << 15) | (biased_exp << 10) | (mantissa & 0x3FF)
        }
    };
    bits.to_be_bytes()
}

pub(crate) fn serialize_config_value(
    value: &crate::node_tree::ConfigValue,
    _element_type: crate::node_tree::LeafType,
    size: u32,
) -> Vec<u8> {
    match value {
        crate::node_tree::ConfigValue::Int { value: v } => {
            match size {
                1 => vec![*v as u8],
                2 => (*v as i16).to_be_bytes().to_vec(),
                4 => (*v as i32).to_be_bytes().to_vec(),
                8 => v.to_be_bytes().to_vec(),
                _ => vec![*v as u8],
            }
        }
        crate::node_tree::ConfigValue::String { value: s } => {
            let mut bytes: Vec<u8> = s.bytes().take(size as usize - 1).collect();
            // NUL-terminate, pad to full size
            bytes.push(0);
            while bytes.len() < size as usize {
                bytes.push(0);
            }
            bytes
        }
        crate::node_tree::ConfigValue::EventId { bytes, .. } => bytes.to_vec(),
        crate::node_tree::ConfigValue::Float { value: v } => {
            match size {
                2 => f64_to_f16_bytes(*v).to_vec(),
                8 => v.to_be_bytes().to_vec(),
                _ => (*v as f32).to_be_bytes().to_vec(), // default: 4-byte f32
            }
        }
    }
}

// ============================================================================
// T009: Unit tests for get_card_elements (navigate_and_build_card_tree)
// ============================================================================

#[cfg(test)]
mod get_card_elements_tests {
    use super::*;

    /// CDI with a replicated "Line" group (3 instances):
    ///   Segment "Port I/O" (space=253, origin=0)
    ///     Group "Line" offset=0, replication=3, stride=24
    ///       String "User Name" size=16, offset=0
    ///       EventId "Set Event" offset=0  (always 8 bytes)
    fn make_replicated_line_cdi() -> lcc_rs::cdi::Cdi {
        lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![lcc_rs::cdi::Segment {
                name: Some("Port I/O".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: vec![lcc_rs::cdi::DataElement::Group(lcc_rs::cdi::Group {
                    name: Some("Line".to_string()),
                    description: Some("Config for one line".to_string()),
                    offset: 0,
                    replication: 3,
                    repname: vec!["Line".to_string()],
                    elements: vec![
                        lcc_rs::cdi::DataElement::String(lcc_rs::cdi::StringElement {
                            name: Some("User Name".to_string()),
                            description: Some("User-assigned label".to_string()),
                            size: 16,
                            offset: 0,
                        }),
                        lcc_rs::cdi::DataElement::EventId(lcc_rs::cdi::EventIdElement {
                            name: Some("Set Event".to_string()),
                            description: Some("Event that activates this line".to_string()),
                            offset: 0,
                        }),
                    ],
                    hints: None,
                })],
            }],
        }
    }

    #[test]
    fn test_card_tree_basic_build() {
        let cdi = make_replicated_line_cdi();
        let path = vec!["seg:0".to_string(), "elem:0#1".to_string()];
        let result = navigate_and_build_card_tree(&cdi, &path);
        assert!(result.is_ok(), "Should build tree for instance 1: {:?}", result);
        let tree = result.unwrap();

        assert_eq!(tree.group_name.as_deref(), Some("Line"));
        assert_eq!(tree.group_description.as_deref(), Some("Config for one line"));
        assert_eq!(tree.fields.len(), 2, "Expected 2 fields");
        assert!(tree.sub_groups.is_empty(), "Expected no sub-groups");

        // Instance 1 (1-based = index 0): base_address = 0 + 0*24 = 0
        assert_eq!(tree.fields[0].name, "User Name");
        assert_eq!(tree.fields[0].data_type, "string");
        assert_eq!(tree.fields[0].memory_address, 0);
        assert_eq!(tree.fields[0].size_bytes, 16);

        assert_eq!(tree.fields[1].name, "Set Event");
        assert_eq!(tree.fields[1].data_type, "eventid");
        assert_eq!(tree.fields[1].memory_address, 16); // After 16-byte string
        assert_eq!(tree.fields[1].size_bytes, 8);
    }

    #[test]
    fn test_replicated_group_instance_address() {
        let cdi = make_replicated_line_cdi();
        // Instance 3 (1-based, stride=24): base = 0 + (3-1)*24 = 48
        let path = vec!["seg:0".to_string(), "elem:0#3".to_string()];
        let result = navigate_and_build_card_tree(&cdi, &path);
        assert!(result.is_ok(), "Should build tree for instance 3: {:?}", result);
        let tree = result.unwrap();

        assert_eq!(tree.fields[0].name, "User Name");
        assert_eq!(tree.fields[0].memory_address, 48);
        assert_eq!(tree.fields[1].name, "Set Event");
        assert_eq!(tree.fields[1].memory_address, 64); // 48 + 16
    }

    #[test]
    fn test_invalid_segment_returns_invalidpath_error() {
        let cdi = make_replicated_line_cdi();
        let path = vec!["seg:5".to_string(), "elem:0".to_string()];
        let result = navigate_and_build_card_tree(&cdi, &path);
        assert!(result.is_err(), "Should error for out-of-range segment");
        assert!(
            result.unwrap_err().contains("InvalidPath"),
            "Error should contain InvalidPath"
        );
    }

    #[test]
    fn test_invalid_element_index_returns_error() {
        let cdi = make_replicated_line_cdi();
        let path = vec!["seg:0".to_string(), "elem:99".to_string()];
        let result = navigate_and_build_card_tree(&cdi, &path);
        assert!(result.is_err(), "Should error for out-of-range element index");
        assert!(result.unwrap_err().contains("InvalidPath"));
    }

    #[test]
    fn test_path_to_non_group_returns_error() {
        // elem:0#1 is the Group "Line"; elem:0 inside it is String "User Name" (not a group)
        let cdi = make_replicated_line_cdi();
        let path = vec![
            "seg:0".to_string(),
            "elem:0#1".to_string(),
            "elem:0".to_string(),
        ];
        let result = navigate_and_build_card_tree(&cdi, &path);
        assert!(result.is_err(), "Path pointing to a leaf element should be an error");
        assert!(result.unwrap_err().contains("InvalidPath"));
    }

    #[test]
    fn test_empty_group_returns_empty_fields() {
        let cdi = lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![lcc_rs::cdi::Segment {
                name: Some("Empty".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: vec![lcc_rs::cdi::DataElement::Group(lcc_rs::cdi::Group {
                    name: Some("EmptyGroup".to_string()),
                    description: None,
                    offset: 0,
                    replication: 1,
                    repname: vec![],
                    elements: vec![],
                    hints: None,
                })],
            }],
        };
        let path = vec!["seg:0".to_string(), "elem:0".to_string()];
        let result = navigate_and_build_card_tree(&cdi, &path);
        assert!(result.is_ok(), "Should succeed for empty group");
        let tree = result.unwrap();
        assert!(tree.fields.is_empty(), "Empty group should have no fields");
        assert!(tree.sub_groups.is_empty(), "Empty group should have no sub-groups");
    }

    #[test]
    fn test_nested_subgroup_addresses() {
        // Segment origin=0, space=253
        //   Group "Advanced" offset=0, replication=1
        //     Int "Mode" size=1, offset=0  → address=0
        //     Group "Timing" offset=0, replication=1
        //       Int "Delay" size=2, offset=0  → address = 0+1+0 = 1
        let cdi = lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![lcc_rs::cdi::Segment {
                name: Some("Settings".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: vec![lcc_rs::cdi::DataElement::Group(lcc_rs::cdi::Group {
                    name: Some("Advanced".to_string()),
                    description: None,
                    offset: 0,
                    replication: 1,
                    repname: vec![],
                    elements: vec![
                        lcc_rs::cdi::DataElement::Int(lcc_rs::cdi::IntElement {
                            name: Some("Mode".to_string()),
                            description: None,
                            size: 1,
                            offset: 0,
                            min: None,
                            max: None,
                            default: Some(0),
                            map: None,
                            hints: None,
                        }),
                        lcc_rs::cdi::DataElement::Group(lcc_rs::cdi::Group {
                            name: Some("Timing".to_string()),
                            description: Some("Timing parameters".to_string()),
                            offset: 0,
                            replication: 1,
                            repname: vec![],
                            elements: vec![lcc_rs::cdi::DataElement::Int(
                                lcc_rs::cdi::IntElement {
                                    name: Some("Delay".to_string()),
                                    description: None,
                                    size: 2,
                                    offset: 0,
                                    min: Some(0),
                                    max: Some(1000),
                                    default: None,
                                    map: None,
                                    hints: None,
                                },
                            )],
                            hints: None,
                        }),
                    ],
                    hints: None,
                })],
            }],
        };
        let path = vec!["seg:0".to_string(), "elem:0".to_string()];
        let result = navigate_and_build_card_tree(&cdi, &path);
        assert!(result.is_ok(), "Should succeed for nested group: {:?}", result);
        let tree = result.unwrap();

        assert_eq!(tree.fields.len(), 1, "Should have 1 direct field (Mode)");
        assert_eq!(tree.fields[0].name, "Mode");
        assert_eq!(tree.fields[0].memory_address, 0);

        assert_eq!(tree.sub_groups.len(), 1, "Should have 1 sub-group (Timing)");
        assert_eq!(tree.sub_groups[0].name, "Timing");
        assert_eq!(tree.sub_groups[0].fields.len(), 1);
        assert_eq!(tree.sub_groups[0].fields[0].name, "Delay");
        assert_eq!(tree.sub_groups[0].fields[0].memory_address, 1); // Mode(0+1) + Timing.offset(0)
    }

    #[test]
    fn test_element_paths_include_full_group_path() {
        let cdi = make_replicated_line_cdi();
        let path = vec!["seg:0".to_string(), "elem:0#1".to_string()];
        let tree = navigate_and_build_card_tree(&cdi, &path).unwrap();

        // Field paths should have the group_path as prefix
        assert_eq!(
            tree.fields[0].element_path,
            vec!["seg:0", "elem:0#1", "elem:0"]
        );
        assert_eq!(
            tree.fields[1].element_path,
            vec!["seg:0", "elem:0#1", "elem:1"]
        );
    }

    #[test]
    fn test_address_space_propagated_to_all_fields() {
        let cdi = make_replicated_line_cdi();
        let path = vec!["seg:0".to_string(), "elem:0#1".to_string()];
        let tree = navigate_and_build_card_tree(&cdi, &path).unwrap();

        for field in &tree.fields {
            assert_eq!(field.address_space, 253, "All fields should carry address_space=253");
        }
    }
}

// ============================================================================
// Tests for parse_config_value (T022-T028)
// ============================================================================
#[cfg(test)]
mod parse_config_value_tests {
    use super::{parse_config_value, ConfigValue};
    use lcc_rs::cdi::{DataElement, IntElement, StringElement, EventIdElement, FloatElement};

    fn make_int(size: u8) -> DataElement {
        DataElement::Int(IntElement {
            name: None,
            description: None,
            size,
            offset: 0,
            min: None,
            max: None,
            default: None,
            map: None,
            hints: None,
        })
    }

    fn make_string(size: usize) -> DataElement {
        DataElement::String(StringElement {
            name: None,
            description: None,
            size,
            offset: 0,
        })
    }

    fn make_eventid() -> DataElement {
        DataElement::EventId(EventIdElement {
            name: None,
            description: None,
            offset: 0,
        })
    }

    fn make_float() -> DataElement {
        DataElement::Float(FloatElement {
            name: None,
            description: None,
            offset: 0,
            size: 4,
            min: None,
            max: None,
            default: None,
        })
    }

    // T022 — 1-byte int
    #[test]
    fn test_parse_int_value_1_byte() {
        let elem = make_int(1);
        let result = parse_config_value(&elem, &[42u8]).unwrap();
        assert!(matches!(result, ConfigValue::Int { value: 42, size_bytes: 1 }));
    }

    // T022 — 1-byte int, negative (sign-extended from i8)
    #[test]
    fn test_parse_int_value_1_byte_negative() {
        let elem = make_int(1);
        // 0xFF as i8 = -1
        let result = parse_config_value(&elem, &[0xFFu8]).unwrap();
        assert!(matches!(result, ConfigValue::Int { value: -1, size_bytes: 1 }));
    }

    // T022 — 2-byte int (big-endian)
    #[test]
    fn test_parse_int_value_2_bytes() {
        let elem = make_int(2);
        let result = parse_config_value(&elem, &[0x01u8, 0x00u8]).unwrap();
        assert!(matches!(result, ConfigValue::Int { value: 256, size_bytes: 2 }));
    }

    // T023 — 4-byte int (big-endian)
    #[test]
    fn test_parse_int_value_4_bytes() {
        let elem = make_int(4);
        let data = 1_000_000i32.to_be_bytes();
        let result = parse_config_value(&elem, &data).unwrap();
        assert!(matches!(result, ConfigValue::Int { value: 1_000_000, size_bytes: 4 }));
    }

    // T024 — 8-byte int (big-endian)
    #[test]
    fn test_parse_int_value_8_bytes() {
        let elem = make_int(8);
        let data = 0x0102030405060708i64.to_be_bytes();
        let result = parse_config_value(&elem, &data).unwrap();
        assert!(matches!(result, ConfigValue::Int { value: 0x0102030405060708, size_bytes: 8 }));
    }

    // T022 — insufficient data returns Err
    #[test]
    fn test_parse_int_insufficient_data() {
        let elem = make_int(4);
        let result = parse_config_value(&elem, &[0x00u8, 0x01u8]); // only 2 bytes for a 4-byte int
        assert!(result.is_err());
    }

    // T025 — plain ASCII string (null-terminated)
    #[test]
    fn test_parse_string_value() {
        let elem = make_string(32);
        let mut data = [0u8; 32];
        data[..5].copy_from_slice(b"hello");
        let result = parse_config_value(&elem, &data).unwrap();
        match result {
            ConfigValue::String { value, size_bytes: 32 } => assert_eq!(value, "hello"),
            other => panic!("Expected String, got {:?}", other),
        }
    }

    // T025 — string with no null terminator uses full buffer
    #[test]
    fn test_parse_string_no_null() {
        let elem = make_string(4);
        let result = parse_config_value(&elem, b"abcd").unwrap();
        match result {
            ConfigValue::String { value, .. } => assert_eq!(value, "abcd"),
            other => panic!("Expected String, got {:?}", other),
        }
    }

    // T028 — 0xFF bytes (uninitialized flash) are filtered out
    #[test]
    fn test_parse_string_filters_0xff() {
        let elem = make_string(8);
        // Device returns all 0xFF (factory-erased flash)
        let result = parse_config_value(&elem, &[0xFFu8; 8]).unwrap();
        match result {
            ConfigValue::String { value, .. } => assert_eq!(value, ""),
            other => panic!("Expected String, got {:?}", other),
        }
    }

    // T028 — mixed valid UTF-8 and 0xFF bytes
    #[test]
    fn test_parse_string_mixed_ff_bytes() {
        let elem = make_string(8);
        let data = [b'h', b'i', 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00];
        let result = parse_config_value(&elem, &data).unwrap();
        match result {
            ConfigValue::String { value, .. } => assert_eq!(value, "hi"),
            other => panic!("Expected String, got {:?}", other),
        }
    }

    // T026 — EventId: 8 bytes round-trip
    #[test]
    fn test_parse_eventid_value() {
        let elem = make_eventid();
        let bytes: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let result = parse_config_value(&elem, &bytes).unwrap();
        match result {
            ConfigValue::EventId { value } => assert_eq!(value, bytes),
            other => panic!("Expected EventId, got {:?}", other),
        }
    }

    // T026 — EventId wrong length returns Err
    #[test]
    fn test_parse_eventid_wrong_length() {
        let elem = make_eventid();
        let result = parse_config_value(&elem, &[0x01u8; 4]);
        assert!(result.is_err());
    }

    // T027 — Float: known IEEE 754 big-endian value
    #[test]
    fn test_parse_float_value() {
        let elem = make_float();
        let data = 1.5f32.to_be_bytes();
        let result = parse_config_value(&elem, &data).unwrap();
        match result {
            ConfigValue::Float { value } => assert!((value - 1.5f32).abs() < f32::EPSILON),
            other => panic!("Expected Float, got {:?}", other),
        }
    }

    // T027 — Float wrong length returns Err
    #[test]
    fn test_parse_float_wrong_length() {
        let elem = make_float();
        let result = parse_config_value(&elem, &[0x00u8; 2]);
        assert!(result.is_err());
    }

    // Float f32 wrong length (3 bytes for size=4) → Err
    #[test]
    fn test_parse_float_f32_wrong_length() {
        let result = parse_config_value(&make_float(), &[0x00u8; 3]);
        assert!(result.is_err(), "3 bytes for f32 should fail");
    }
}

// ============================================================================
// Tests for serialize_config_value (float encoding: f16/f32/f64)
// ============================================================================
#[cfg(test)]
mod serialize_config_value_tests {
    use super::{serialize_config_value, f64_to_f16_bytes};
    use crate::node_tree::{ConfigValue, LeafType};

    // ── Float serialization ─────────────────────────────────────────────────

    #[test]
    fn test_serialize_float_f32() {
        let val = ConfigValue::Float { value: 1.5f64 };
        let bytes = serialize_config_value(&val, LeafType::Float, 4);
        assert_eq!(bytes, 1.5f32.to_be_bytes().to_vec());
    }

    #[test]
    fn test_serialize_float_f64() {
        let val = ConfigValue::Float { value: 1.5f64 };
        let bytes = serialize_config_value(&val, LeafType::Float, 8);
        assert_eq!(bytes, 1.5f64.to_be_bytes().to_vec());
    }

    #[test]
    fn test_serialize_float_f16_one() {
        // 1.0 in f16 big-endian = [0x3C, 0x00]
        let val = ConfigValue::Float { value: 1.0f64 };
        let bytes = serialize_config_value(&val, LeafType::Float, 2);
        assert_eq!(bytes, vec![0x3Cu8, 0x00u8]);
    }

    #[test]
    fn test_serialize_float_f16_zero() {
        let val = ConfigValue::Float { value: 0.0f64 };
        let bytes = serialize_config_value(&val, LeafType::Float, 2);
        assert_eq!(bytes, vec![0x00u8, 0x00u8]);
    }

    #[test]
    fn test_serialize_float_f16_negative_one() {
        // -1.0 in f16 = [0xBC, 0x00]
        let val = ConfigValue::Float { value: -1.0f64 };
        let bytes = serialize_config_value(&val, LeafType::Float, 2);
        assert_eq!(bytes, vec![0xBCu8, 0x00u8]);
    }

    // ── f64_to_f16_bytes known values ───────────────────────────────────────

    #[test]
    fn test_f64_to_f16_bytes_one() {
        assert_eq!(f64_to_f16_bytes(1.0f64), [0x3Cu8, 0x00u8]);
    }

    #[test]
    fn test_f64_to_f16_bytes_zero() {
        assert_eq!(f64_to_f16_bytes(0.0f64), [0x00u8, 0x00u8]);
    }

    #[test]
    fn test_f64_to_f16_bytes_pos_inf() {
        // +inf in f16 = 0x7C00
        assert_eq!(f64_to_f16_bytes(f64::INFINITY), [0x7Cu8, 0x00u8]);
    }

    #[test]
    fn test_f64_to_f16_bytes_neg_inf() {
        // -inf in f16 = 0xFC00
        assert_eq!(f64_to_f16_bytes(f64::NEG_INFINITY), [0xFCu8, 0x00u8]);
    }

    #[test]
    fn test_f64_to_f16_bytes_nan() {
        // quiet NaN in f16 = 0x7E00
        assert_eq!(f64_to_f16_bytes(f64::NAN), [0x7Eu8, 0x00u8]);
    }

    // ── Int serialization ───────────────────────────────────────────────────

    #[test]
    fn test_serialize_int_1_byte() {
        let val = ConfigValue::Int { value: 42 };
        assert_eq!(serialize_config_value(&val, LeafType::Int, 1), vec![42u8]);
    }

    #[test]
    fn test_serialize_int_2_bytes() {
        let val = ConfigValue::Int { value: 0x0102 };
        assert_eq!(serialize_config_value(&val, LeafType::Int, 2), vec![0x01u8, 0x02u8]);
    }

    #[test]
    fn test_serialize_int_4_bytes() {
        let val = ConfigValue::Int { value: 0x01020304 };
        assert_eq!(serialize_config_value(&val, LeafType::Int, 4), vec![0x01u8, 0x02, 0x03, 0x04]);
    }
}

// ============================================================================
// Tests for extract_all_elements_with_addresses and build_read_plan
// ============================================================================
#[cfg(test)]
mod read_plan_tests {
    use super::*;

    /// One segment, three consecutive Int elements (no offset skips).
    /// Expected addresses: 0, 1, 3  (sizes 1, 2, 4 → cursor 0, 1, 3)
    fn make_flat_cdi() -> lcc_rs::cdi::Cdi {
        use lcc_rs::cdi::{IntElement, DataElement};
        lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![lcc_rs::cdi::Segment {
                name: Some("Config".to_string()),
                description: None,
                space: 0xFD,
                origin: 0,
                elements: vec![
                    DataElement::Int(IntElement { name: Some("A".to_string()), description: None, size: 1, offset: 0, min: None, max: None, default: None, map: None, hints: None }),
                    DataElement::Int(IntElement { name: Some("B".to_string()), description: None, size: 2, offset: 0, min: None, max: None, default: None, map: None, hints: None }),
                    DataElement::Int(IntElement { name: Some("C".to_string()), description: None, size: 4, offset: 0, min: None, max: None, default: None, map: None, hints: None }),
                ],
            }],
        }
    }

    /// Segment with a non-zero origin and an element that has an offset skip.
    fn make_offset_cdi() -> lcc_rs::cdi::Cdi {
        use lcc_rs::cdi::{IntElement, DataElement};
        lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![lcc_rs::cdi::Segment {
                name: None,
                description: None,
                space: 0xFD,
                origin: 100,
                elements: vec![
                    // 2-byte int at offset 0 → absolute address 100
                    DataElement::Int(IntElement { name: Some("X".to_string()), description: None, size: 2, offset: 0, min: None, max: None, default: None, map: None, hints: None }),
                    // 1-byte int with a 3-byte skip → absolute address 100+2+3=105
                    DataElement::Int(IntElement { name: Some("Y".to_string()), description: None, size: 1, offset: 3, min: None, max: None, default: None, map: None, hints: None }),
                ],
            }],
        }
    }

    // ---- extract_all_elements_with_addresses tests ----

    #[test]
    fn test_extract_flat_addresses() {
        let cdi = make_flat_cdi();
        let elems = extract_all_elements_with_addresses(&cdi);
        assert_eq!(elems.len(), 3);

        assert_eq!(elems[0].name, "A");
        assert_eq!(elems[0].absolute_address(), 0);
        assert_eq!(elems[0].space, 0xFD);

        assert_eq!(elems[1].name, "B");
        assert_eq!(elems[1].absolute_address(), 1);

        assert_eq!(elems[2].name, "C");
        assert_eq!(elems[2].absolute_address(), 3);
    }

    #[test]
    fn test_extract_segment_origin_and_element_offset() {
        let cdi = make_offset_cdi();
        let elems = extract_all_elements_with_addresses(&cdi);
        assert_eq!(elems.len(), 2);
        assert_eq!(elems[0].absolute_address(), 100); // origin 100 + offset 0
        assert_eq!(elems[1].absolute_address(), 105); // origin 100 + cursor(2) + skip(3)
    }

    #[test]
    fn test_extract_empty_cdi_returns_empty() {
        let cdi = lcc_rs::cdi::Cdi { identification: None, acdi: None, segments: vec![] };
        let elems = extract_all_elements_with_addresses(&cdi);
        assert!(elems.is_empty());
    }

    #[test]
    fn test_extract_replicated_group_addresses() {
        use lcc_rs::cdi::{IntElement, DataElement, Group};
        // Group with replication=2, stride = 4 (one 4-byte Int)
        let cdi = lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![lcc_rs::cdi::Segment {
                name: None, description: None, space: 253, origin: 0,
                elements: vec![DataElement::Group(Group {
                    name: Some("G".to_string()),
                    description: None,
                    offset: 0,
                    replication: 2,
                    repname: vec!["G".to_string()],
                    elements: vec![DataElement::Int(IntElement {
                        name: Some("V".to_string()), description: None,
                        size: 4, offset: 0, min: None, max: None, default: None, map: None, hints: None,
                    })],
                    hints: None,
                })],
            }],
        };
        let elems = extract_all_elements_with_addresses(&cdi);
        assert_eq!(elems.len(), 2, "Two instances → two elements");
        assert_eq!(elems[0].absolute_address(), 0);  // instance 1
        assert_eq!(elems[1].absolute_address(), 4);  // instance 2 (stride = 4)
        // The group instance step (path[1]) carries the '#' instance marker
        assert!(elems[0].path[1].contains('#'), "Group path step should contain instance marker #");
        assert!(elems[1].path[1].contains('#'));
    }

    #[test]
    fn test_extract_path_structure() {
        let cdi = make_flat_cdi();
        let elems = extract_all_elements_with_addresses(&cdi);
        // All elements are in seg:0, so paths should start with "seg:0"
        for e in &elems {
            assert_eq!(e.path[0], "seg:0");
            assert!(e.path[1].starts_with("elem:"), "Second step should be elem:N");
        }
    }

    // ---- build_read_plan tests ----

    #[test]
    fn test_build_read_plan_single_element() {
        let cdi = make_flat_cdi();
        let elems = extract_all_elements_with_addresses(&cdi);
        // Just use the first element
        let single = &elems[..1];
        let plan = build_read_plan(single);
        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.batches.len(), 1);
        assert!(plan.multi_chunk_indices.is_empty());
        assert_eq!(plan.invalid_element_count, 0);
    }

    #[test]
    fn test_build_read_plan_consecutive_elements_grouped() {
        let cdi = make_flat_cdi();
        let elems = extract_all_elements_with_addresses(&cdi);
        // A(1) at 0, B(2) at 1, C(4) at 3 → span 0..7 = 7 bytes, fits in 64
        let plan = build_read_plan(&elems);
        assert_eq!(plan.items.len(), 3);
        assert_eq!(plan.batches.len(), 1, "All 3 elements fit in one batch");
    }

    #[test]
    fn test_build_read_plan_different_spaces_split() {
        use lcc_rs::cdi::{IntElement, DataElement};
        let cdi = lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![
                lcc_rs::cdi::Segment {
                    name: None, description: None, space: 0xFD, origin: 0,
                    elements: vec![DataElement::Int(IntElement {
                        name: Some("P".to_string()), description: None, size: 1, offset: 0,
                        min: None, max: None, default: None, map: None, hints: None,
                    })],
                },
                lcc_rs::cdi::Segment {
                    name: None, description: None, space: 0xFE, origin: 0,
                    elements: vec![DataElement::Int(IntElement {
                        name: Some("Q".to_string()), description: None, size: 1, offset: 0,
                        min: None, max: None, default: None, map: None, hints: None,
                    })],
                },
            ],
        };
        let elems = extract_all_elements_with_addresses(&cdi);
        let plan = build_read_plan(&elems);
        assert_eq!(plan.batches.len(), 2, "Different spaces must be separate batches");
    }

    #[test]
    fn test_build_read_plan_large_element_chunked() {
        use lcc_rs::cdi::{StringElement, DataElement};
        // String of 130 bytes → ceil(130/64) = 3 chunks
        let cdi = lcc_rs::cdi::Cdi {
            identification: None,
            acdi: None,
            segments: vec![lcc_rs::cdi::Segment {
                name: None, description: None, space: 0xFD, origin: 0,
                elements: vec![DataElement::String(StringElement {
                    name: Some("Long".to_string()), description: None, size: 130, offset: 0,
                })],
            }],
        };
        let elems = extract_all_elements_with_addresses(&cdi);
        let plan = build_read_plan(&elems);
        assert_eq!(plan.items.len(), 3, "130 bytes → 3 chunks of 64, 64, 2");
        assert!(plan.multi_chunk_indices.contains(&0), "Element 0 flagged as multi-chunk");
        assert_eq!(plan.items[0].size, 64);
        assert_eq!(plan.items[1].size, 64);
        assert_eq!(plan.items[2].size, 2);
    }

    #[test]
    fn test_build_read_plan_gap_elements_same_batch_if_span_fits() {
        let cdi = make_offset_cdi();
        // X(2) at 100, Y(1) at 105 → span = 105+1-100 = 6 ≤ 64 → same batch
        let elems = extract_all_elements_with_addresses(&cdi);
        let plan = build_read_plan(&elems);
        assert_eq!(plan.batches.len(), 1, "Elements with a gap but span ≤ 64 share a batch");
    }
}

// ============================================================================
// Tests for fill_short_reply (short memory config reply continuation)
// ============================================================================
#[cfg(test)]
mod fill_short_reply_tests {
    use super::*;

    /// Node returns 40 of 58 requested bytes.  Continuation should read the
    /// remaining 18 bytes at the correct address and append them.
    #[tokio::test]
    async fn continuation_fills_remaining_bytes() {
        let mut data: Vec<u8> = (0..40u8).collect();
        let batch_start = 0x0C5Cu32;
        let batch_total = 58u32;
        let remaining_bytes: Vec<u8> = (40..58u8).collect();

        let mut call_count = 0u32;
        let continuations = fill_short_reply(
            &mut data,
            batch_start,
            batch_total,
            |addr, size| {
                call_count += 1;
                let rb = remaining_bytes.clone();
                async move {
                    // First (and only) continuation: should request at addr 0x0C5C+40=0x0C84, size 18
                    assert_eq!(addr, 0x0C5C + 40, "continuation address = batch_start + received");
                    assert_eq!(size, 18, "continuation size = remaining bytes");
                    Ok(rb)
                }
            },
        ).await;

        assert_eq!(continuations, 1, "Exactly one continuation read needed");
        assert_eq!(call_count, 1);
        assert_eq!(data.len(), 58, "Data fully filled");
        let expected: Vec<u8> = (0..58u8).collect();
        assert_eq!(data, expected, "Data bytes match original + continuation");
    }

    /// Multiple continuations needed when node returns small chunks.
    #[tokio::test]
    async fn multiple_continuations_for_small_chunks() {
        let mut data: Vec<u8> = vec![0; 10]; // initial 10 of 64
        let batch_start = 0u32;
        let batch_total = 64u32;

        let mut calls = Vec::new();
        let continuations = fill_short_reply(
            &mut data,
            batch_start,
            batch_total,
            |addr, size| {
                calls.push((addr, size));
                async move {
                    // Return 20 bytes each time
                    Ok(vec![0xAA; 20])
                }
            },
        ).await;

        // 10 + 20 + 20 + 20 = 70 ≥ 64 → but capped: 10+20=30, 30+20=50, 50+20=70
        // Actually data fills to 70 which is > 64, so loop stops after continuation 3
        // because 30 < 64, 50 < 64, 70 ≥ 64
        assert_eq!(continuations, 3);
        assert_eq!(data.len(), 70); // overread is fine; slicing happens in caller
        // Verify addresses advanced correctly: 10, 30, 50
        assert_eq!(calls[0].0, 10);
        assert_eq!(calls[1].0, 30);
        assert_eq!(calls[2].0, 50);
    }

    /// Zero-byte continuation reply terminates the loop (node has no more data).
    #[tokio::test]
    async fn zero_byte_reply_stops_continuation() {
        let mut data: Vec<u8> = vec![0; 20]; // 20 of 64
        let batch_start = 100u32;
        let batch_total = 64u32;

        let continuations = fill_short_reply(
            &mut data,
            batch_start,
            batch_total,
            |_addr, _size| async move {
                Ok(vec![]) // node returns empty
            },
        ).await;

        assert_eq!(continuations, 0, "Zero-byte reply breaks before incrementing");
        assert_eq!(data.len(), 20, "Data unchanged — no bytes appended");
    }

    /// Error on continuation terminates the loop gracefully.
    #[tokio::test]
    async fn error_stops_continuation() {
        let mut data: Vec<u8> = vec![0; 30]; // 30 of 58
        let batch_start = 0u32;
        let batch_total = 58u32;

        let continuations = fill_short_reply(
            &mut data,
            batch_start,
            batch_total,
            |_addr, _size| async move {
                Err("Timeout waiting for datagram reply".to_string())
            },
        ).await;

        assert_eq!(continuations, 0, "Error breaks before incrementing");
        assert_eq!(data.len(), 30, "Data unchanged on error");
    }

    /// If initial data already meets batch_total_size, no continuations issued.
    #[tokio::test]
    async fn no_continuation_when_full() {
        let mut data: Vec<u8> = vec![0; 64];
        let batch_start = 0u32;
        let batch_total = 64u32;

        let continuations = fill_short_reply(
            &mut data,
            batch_start,
            batch_total,
            |_addr, _size| async move {
                panic!("Should not be called");
            },
        ).await;

        assert_eq!(continuations, 0);
        assert_eq!(data.len(), 64);
    }

    /// Continuation is capped at MAX_CONTINUATIONS even if data never fills.
    #[tokio::test]
    async fn max_continuations_cap() {
        let mut data: Vec<u8> = vec![0; 10];
        let batch_start = 0u32;
        let batch_total = 200u32; // impossibly large for a single batch, but tests the cap

        let continuations = fill_short_reply(
            &mut data,
            batch_start,
            batch_total,
            |_addr, _size| async move {
                Ok(vec![0xFF; 1]) // return 1 byte at a time — very slow node
            },
        ).await;

        assert_eq!(continuations, MAX_CONTINUATIONS, "Capped at MAX_CONTINUATIONS");
        assert_eq!(data.len(), 10 + MAX_CONTINUATIONS as usize, "Got 1 byte per continuation");
    }

    /// Continuation read sizes are capped at 64 bytes per read.
    #[tokio::test]
    async fn continuation_size_capped_at_64() {
        let mut data: Vec<u8> = vec![0; 10];
        let batch_start = 0u32;
        let batch_total = 200u32;

        let mut requested_sizes = Vec::new();
        let _continuations = fill_short_reply(
            &mut data,
            batch_start,
            batch_total,
            |_addr, size| {
                requested_sizes.push(size);
                async move {
                    // Return exactly what was requested to advance normally
                    Ok(vec![0; size as usize])
                }
            },
        ).await;

        for &sz in &requested_sizes {
            assert!(sz <= 64, "Continuation size {} exceeds 64-byte CAN datagram limit", sz);
        }
    }
}
