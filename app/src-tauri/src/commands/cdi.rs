//! CDI (Configuration Description Information) XML viewer commands

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::Utc;
use tauri::{Manager, Emitter};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

// T104: CDI parsing cache (parsed Cdi structs by node ID)
lazy_static::lazy_static! {
    static ref CDI_PARSE_CACHE: Arc<RwLock<HashMap<String, lcc_rs::cdi::Cdi>>> = 
        Arc::new(RwLock::new(HashMap::new()));
}

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

    // Get connection reference
    let connection_arc = {
        let conn_guard = state.connection.read().await;
        match conn_guard.as_ref() {
            Some(conn) => conn.clone(),
            None => return Err(CdiError::RetrievalFailed("Not connected to LCC network".to_string()).into()),
        }
    };

    println!("[CDI] Starting CDI download from alias 0x{:03X}...", alias);
    
    // Download CDI from node (5 second timeout per chunk to accommodate slower nodes)
    let xml_content = {
        let mut connection = connection_arc.lock().await;
        connection
            .read_cdi(alias, 5000)
            .await
            .map_err(|e| {
                println!("[CDI] Download failed: {}", e);
                CdiError::RetrievalFailed(format!("CDI download failed: {}", e))
            })?
    };

    println!("[CDI] Download complete, size: {} bytes", xml_content.len());
    
    let retrieved_at = Utc::now();

    // Create CdiData
    let cdi_data = lcc_rs::CdiData {
        xml_content: xml_content.clone(),
        retrieved_at,
    };

    // Update node cache with CDI
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
    let nodes = state.nodes.read().await;
    
    let mut discovered_nodes = Vec::new();
    
    for node in nodes.iter() {
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
    // T104: Check parse cache first
    let cache = CDI_PARSE_CACHE.read().await;
    let cached_cdi = cache.get(&node_id).cloned();
    drop(cache);

    let cdi = if let Some(cached) = cached_cdi {
        cached
    } else {
        // Get CDI XML from cache
        let cdi_response = get_cdi_xml(node_id.clone(), app_handle, state.clone()).await?;
        
        let xml_content = cdi_response
            .xml_content
            .ok_or_else(|| CdiError::CdiNotRetrieved(node_id.clone()))?;
        
        // Parse CDI XML
        let parsed_cdi = lcc_rs::cdi::parser::parse_cdi(&xml_content)
            .map_err(CdiError::InvalidXml)?;
        
        // T104: Cache the parsed CDI
        let mut cache = CDI_PARSE_CACHE.write().await;
        cache.insert(node_id.clone(), parsed_cdi.clone());
        drop(cache);
        
        parsed_cdi
    };
    
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
    
    // Get node name
    let nodes = state.nodes.read().await;
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;
    
    let node_name = nodes
        .iter()
        .find(|n| n.node_id == parsed_node_id)
        .and_then(|n| n.snip_data.as_ref())
        .map(|s| s.user_name.clone())
        .unwrap_or_else(|| format!("Node {}", node_id));
    
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
    
    // T104: Check parse cache first
    let cache = CDI_PARSE_CACHE.read().await;
    let cached_cdi = cache.get(&node_id).cloned();
    drop(cache);

    let cdi = if let Some(cached) = cached_cdi {
        cached
    } else {
        // Get CDI XML from cache
        let cdi_response = get_cdi_xml(node_id.clone(), app_handle, state.clone()).await?;
        
        let xml_content = cdi_response
            .xml_content
            .ok_or_else(|| CdiError::CdiNotRetrieved(node_id.clone()))?;
        
        // Parse CDI XML
        let parsed_cdi = lcc_rs::cdi::parser::parse_cdi(&xml_content)
            .map_err(CdiError::InvalidXml)?;
        
        // T104: Cache the parsed CDI
        let mut cache = CDI_PARSE_CACHE.write().await;
        cache.insert(node_id.clone(), parsed_cdi.clone());
        drop(cache);
        
        parsed_cdi
    };
    
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
    
    // Get node name for breadcrumb
    let nodes = state.nodes.read().await;
    let parsed_node_id = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;
    
    let node_name = nodes
        .iter()
        .find(|n| n.node_id == parsed_node_id)
        .and_then(|n| n.snip_data.as_ref())
        .map(|s| s.user_name.clone())
        .unwrap_or_else(|| format!("Node {}", node_id));
    
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

/// Extract memory address information from element (T010)
/// Returns (segment_origin, element_offset)
fn extract_address_info(segment: &lcc_rs::cdi::Segment, element: &lcc_rs::cdi::DataElement) -> Result<(u32, u32), String> {
    use lcc_rs::cdi::DataElement;
    
    let element_offset = match element {
        DataElement::Int(e) => e.offset,
        DataElement::String(e) => e.offset,
        DataElement::EventId(e) => e.offset,
        DataElement::Float(e) => e.offset,
        DataElement::Blob(e) => e.offset,
        _ => return Err("Element does not have address offset".to_string()),
    };
    
    // Convert i32 to u32 (addresses are unsigned in protocol)
    let origin = segment.origin as u32;
    let offset = element_offset as u32;
    
    Ok((origin, offset))
}

/// Navigate to an element in the CDI structure using path (T011)
/// Returns (segment, element) tuple
///
/// Accepts index-based paths generated by the frontend:
///   path[0] = "seg:N"       — segment by 0-based index
///   path[1..] = "elem:N"    — element by 0-based index within parent elements slice
///              "elem:N#I"   — replicated group instance (N = element index, I = instance, 1-based)
///                             The template element at index N is used; replication offset is
///                             applied during address calculation, not here.
fn navigate_to_element<'a>(
    cdi: &'a lcc_rs::cdi::Cdi,
    path: &[String],
) -> Result<(&'a lcc_rs::cdi::Segment, &'a lcc_rs::cdi::DataElement), String> {
    use lcc_rs::cdi::DataElement;

    if path.is_empty() {
        return Err("Path cannot be empty".to_string());
    }

    // --- Segment lookup: parse "seg:N" index format ---
    let segment_id = &path[0];
    let segment = if let Some(index_str) = segment_id.strip_prefix("seg:") {
        let index = index_str
            .parse::<usize>()
            .map_err(|_| format!("Invalid segment index in path: {}", segment_id))?;
        cdi.segments
            .get(index)
            .ok_or_else(|| format!("Segment index out of range: {}", index))?
    } else {
        // Fallback: name-based lookup for any callers using the old format
        cdi.segments
            .iter()
            .find(|s| s.name.as_deref() == Some(segment_id.as_str()))
            .ok_or_else(|| format!("Segment not found: {}", segment_id))?
    };

    // --- Element navigation: parse "elem:N" or "elem:N#I" index format ---
    let mut current_elements: &[DataElement] = &segment.elements;
    let mut target_element: Option<&DataElement> = None;

    for (i, elem_id) in path.iter().skip(1).enumerate() {
        let is_last = i == path.len() - 2;

        // Parse element index from "elem:N" or "elem:N#I"
        let element = if let Some(index_part) = elem_id.strip_prefix("elem:") {
            // Strip optional "#instance" suffix to get the element index
            let index_str = index_part
                .split('#')
                .next()
                .unwrap_or(index_part);
            let index = index_str
                .parse::<usize>()
                .map_err(|_| format!("Invalid element index in path: {}", elem_id))?;
            current_elements
                .get(index)
                .ok_or_else(|| format!("Element index out of range: {} (len={})", index, current_elements.len()))?
        } else {
            // Fallback: name-based lookup
            current_elements
                .iter()
                .find(|e| {
                    let elem_name = match e {
                        DataElement::Group(g) => g.name.as_deref(),
                        DataElement::Int(e) => e.name.as_deref(),
                        DataElement::String(e) => e.name.as_deref(),
                        DataElement::EventId(e) => e.name.as_deref(),
                        DataElement::Float(e) => e.name.as_deref(),
                        DataElement::Blob(e) => e.name.as_deref(),
                        _ => None,
                    };
                    elem_name == Some(elem_id.as_str())
                })
                .ok_or_else(|| format!("Element not found in path: {}", elem_id))?
        };

        if is_last {
            target_element = Some(element);
            break;
        }

        // Navigate into group for non-terminal path segments
        match element {
            DataElement::Group(g) => {
                current_elements = &g.elements;
            }
            _ => return Err(format!("Cannot navigate through non-group element: {}", elem_id)),
        }
    }

    let element = target_element.ok_or_else(|| "Failed to navigate to element".to_string())?;
    Ok((segment, element))
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
            // Only parse up to the null byte
            let s = String::from_utf8(data[..end].to_vec())
                .map_err(|e| format!("Invalid UTF-8: {}", e))?;
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
    // Check parse cache first
    let cache = CDI_PARSE_CACHE.read().await;
    if let Some(cdi) = cache.get(node_id) {
        return Ok(cdi.clone());
    }
    drop(cache);
    
    // Not in parse cache - try to get CDI XML and parse it
    let cdi_response = get_cdi_xml(node_id.to_string(), app_handle.clone(), state.clone()).await?;
    
    let xml_content = cdi_response
        .xml_content
        .ok_or_else(|| CdiError::CdiNotRetrieved(node_id.to_string()))?;
    
    // Parse CDI XML
    let parsed_cdi = lcc_rs::cdi::parser::parse_cdi(&xml_content)
        .map_err(CdiError::InvalidXml)?;
    
    // Cache the parsed CDI for future use
    let mut cache = CDI_PARSE_CACHE.write().await;
    cache.insert(node_id.to_string(), parsed_cdi.clone());
    drop(cache);
    
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

/// Recursively extract all configurable elements from CDI with their absolute memory addresses (T050)
/// Returns Vec<(element_path, segment_origin, element_offset, element_name, element_ref)>
fn extract_all_elements_with_addresses<'a>(
    cdi: &'a lcc_rs::cdi::Cdi,
) -> Vec<(Vec<String>, u32, u32, String, &'a lcc_rs::cdi::DataElement, u8)> {
    use lcc_rs::cdi::DataElement;
    
    let mut results = Vec::new();
    
    // Process each segment
    for (seg_idx, segment) in cdi.segments.iter().enumerate() {
        let segment_origin = segment.origin as u32;
        let _segment_name = segment.name.as_ref().map(|s| s.as_str()).unwrap_or("config");
        let segment_space = segment.space;
        
        // Helper to recursively process elements within a group/segment.
        // `instance_offset` accumulates the byte offset contributed by replicated
        // group instances at every nesting level, so the final element address is
        // segment_origin + element.offset + instance_offset.
        fn process_elements<'a>(
            elements: &'a [DataElement],
            current_path: &mut Vec<String>,
            segment_origin: u32,
            // Absolute byte offset of this group's start from segment_origin.
            // Per CDI spec, elements use *relative* offsets (skips from the current
            // sequential position), so we maintain a running cursor here.
            base_offset: u32,
            segment_space: u8,
            results: &mut Vec<(Vec<String>, u32, u32, String, &'a DataElement, u8)>,
        ) {
            // Sequential cursor within the current group/segment level.
            // Starts at 0 (= base_offset in absolute terms).
            // Each element SKIPS cursor by element.offset first, then ADVANCES cursor
            // by the element's size.  This implements the CDI spec rule that `offset`
            // is a relative skip from the previous element's end, not an absolute address.
            let mut cursor: i32 = 0;

            for (i, element) in elements.iter().enumerate() {
                match element {
                    DataElement::Group(g) => {
                        let group_name = g.name.as_ref().map(|s| s.as_str()).unwrap_or("group");

                        // Apply this group's own offset skip before placing it.
                        cursor += g.offset;
                        let group_start = base_offset as i32 + cursor;

                        // Size of one group instance = stride between replications.
                        let stride = g.calculate_size();

                        // Guard: stride=0 with replication>1 means all instances would map
                        // to the same address → identical reads.  Clamp to 1 instance.
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

                        // Advance cursor past all instances of this group.
                        cursor += effective_replication as i32 * stride;
                    }
                    DataElement::Int(e) => {
                        cursor += e.offset; // explicit skip before this element
                        let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("int");
                        current_path.push(format!("elem:{}", i));
                        let element_offset = (base_offset as i32 + cursor) as u32;
                        results.push((
                            current_path.clone(),
                            segment_origin,
                            element_offset,
                            name.to_string(),
                            element,
                            segment_space,
                        ));
                        current_path.pop();
                        cursor += e.size as i32;
                    }
                    DataElement::String(e) => {
                        cursor += e.offset;
                        let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("string");
                        current_path.push(format!("elem:{}", i));
                        let element_offset = (base_offset as i32 + cursor) as u32;
                        results.push((
                            current_path.clone(),
                            segment_origin,
                            element_offset,
                            name.to_string(),
                            element,
                            segment_space,
                        ));
                        current_path.pop();
                        cursor += e.size as i32;
                    }
                    DataElement::EventId(e) => {
                        cursor += e.offset;
                        let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("eventid");
                        current_path.push(format!("elem:{}", i));
                        let element_offset = (base_offset as i32 + cursor) as u32;
                        results.push((
                            current_path.clone(),
                            segment_origin,
                            element_offset,
                            name.to_string(),
                            element,
                            segment_space,
                        ));
                        current_path.pop();
                        cursor += 8; // EventId is always 8 bytes
                    }
                    DataElement::Float(e) => {
                        cursor += e.offset;
                        let name = e.name.as_ref().map(|s| s.as_str()).unwrap_or("float");
                        current_path.push(format!("elem:{}", i));
                        let element_offset = (base_offset as i32 + cursor) as u32;
                        results.push((
                            current_path.clone(),
                            segment_origin,
                            element_offset,
                            name.to_string(),
                            element,
                            segment_space,
                        ));
                        current_path.pop();
                        cursor += 4; // 32-bit float
                    }
                    // Skip Action and Blob - they don't store readable configuration values;
                    // but still advance the cursor past them so subsequent elements
                    // get the right addresses.
                    DataElement::Action(e) => { cursor += e.offset + 1; }
                    DataElement::Blob(e)   => { cursor += e.offset + e.size as i32; }
                }
            }
        }
        
        let mut path = vec![format!("seg:{}", seg_idx)];
        process_elements(&segment.elements, &mut path, segment_origin, 0, segment_space, &mut results);
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
        .find(|(path, ..)| path.as_slice() == element_path.as_slice())
        .ok_or_else(|| format!("Element not found at path: {}", element_path.join("/")))?;
    let absolute_address = found.1 + found.2;
    let element = found.4;
    let space = found.5;
    
    // Get connection
    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);
    
    // Get node alias
    let nodes = state.nodes.read().await;
    let node = nodes
        .iter()
        .find(|n| n.node_id == parsed_node_id)
        .ok_or_else(|| format!("Node not found: {}", node_id))?;
    let alias = node.alias.value();
    drop(nodes);
    
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
    
    // Get node info
    let nodes = state.nodes.read().await;
    let node = nodes
        .iter()
        .find(|n| n.node_id == parsed_node_id)
        .ok_or_else(|| format!("Node not found: {}", node_id))?
        .clone();
    drop(nodes);
    
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

    // --- Build a flat list of (orig_index, absolute_address, size, space) ---
    // Items whose size is invalid or >64 are counted as errors immediately and
    // excluded from the batch plan (same behaviour as before, just up-front).
    struct ReadItem {
        orig_index: usize,
        absolute_address: u32,
        size: u32,
        space: u8,
    }

    let mut sized_items: Vec<ReadItem> = Vec::new();
    for (idx, (_, segment_origin, element_offset, element_name, element, segment_space))
        in all_elements.iter().enumerate()
    {
        match get_element_size(element) {
            Ok(s) if s <= 64 => {
                sized_items.push(ReadItem {
                    orig_index: idx,
                    absolute_address: segment_origin + element_offset,
                    size: s,
                    space: *segment_space,
                });
            }
            Ok(s) => {
                error_count += 1;
                eprintln!("Element {} size {} exceeds 64 bytes, skipping", element_name, s);
            }
            Err(e) => {
                error_count += 1;
                eprintln!("Failed to get element size for {}: {}", element_name, e);
            }
        }
    }

    // --- Sort by (space, absolute_address) to enable consecutive grouping ---
    sized_items.sort_by_key(|item| (item.space, item.absolute_address));

    // --- Group into batches of same-space elements fitting within a 64-byte window ---
    // Elements that are not consecutive (have gaps between them) are still batched
    // together as long as the span from the first element's start to the last element's
    // end is <= 64 bytes.  Gap bytes are read from the node but discarded during slicing.
    // This matches JMRI's behaviour and eliminates the "one read per element" phase that
    // occurs when CDI elements have small non-zero offsets between them.
    //
    // A batch is a Vec of indices into `sized_items`.
    let mut batches: Vec<Vec<usize>> = Vec::new();
    {
        let mut current_batch: Vec<usize> = Vec::new();
        let mut batch_start_addr: u32 = 0;
        let mut batch_end_addr: u32 = 0;  // end of the last element added
        let mut batch_space: u8 = 0;

        for (i, item) in sized_items.iter().enumerate() {
            // An item fits in the current batch when:
            //   - same address space
            //   - the span from our batch start to this item's end fits in 64 bytes
            // Items are sorted by address so item.absolute_address >= batch_start_addr always.
            let fits = !current_batch.is_empty()
                && item.space == batch_space
                && (item.absolute_address + item.size - batch_start_addr) <= 64;

            if fits {
                // Extend the window end if this element reaches further.
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

    let total_batches = batches.len();
    eprintln!(
        "[CDI] {} elements grouped into {} read batches (was {} round-trips)",
        sized_items.len(), total_batches, sized_items.len()
    );

    // --- Issue one read_memory per batch, slice individual element values from reply ---
    let mut elements_processed: usize = 0;

    for batch in batches.iter() {
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

        // T056: Single read covering all elements in this batch
        let mut conn = connection.lock().await;
        let response_data = match conn
            .read_memory(alias, batch_space, batch_start_addr, batch_total_size as u8, timeout)
            .await
        {
            Ok(data) => { drop(conn); data }
            Err(e) => {
                drop(conn);
                // Count every element in the batch as failed
                error_count += batch.len();
                for &i in batch {
                    let (_, _, _, element_name, _, _) = &all_elements[sized_items[i].orig_index];
                    eprintln!("Failed to read element {} (batch read @{:#010x}+{}): {}",
                        element_name, batch_start_addr, batch_total_size, e);
                }
                continue;
            }
        };

        // Slice and parse each element's bytes from the batch reply
        for &i in batch {
            let item = &sized_items[i];
            let (element_path, _, _, element_name, element, _) =
                &all_elements[item.orig_index];

            let offset_in_batch = (item.absolute_address - batch_start_addr) as usize;
            let end = offset_in_batch + item.size as usize;

            if end > response_data.len() {
                error_count += 1;
                eprintln!(
                    "Batch reply too short for element {}: need bytes [{}..{}] but reply is {} bytes",
                    element_name, offset_in_batch, end, response_data.len()
                );
                continue;
            }

            let item_data = &response_data[offset_in_batch..end];

            let typed_value = match parse_config_value(element, item_data) {
                Ok(v) => v,
                Err(e) => {
                    error_count += 1;
                    eprintln!("Failed to parse element {}: {}", element_name, e);
                    continue;
                }
            };

            let cache_key = format!("{}:{}", node_id, element_path.join("/"));
            values.insert(cache_key, ConfigValueWithMetadata {
                value: typed_value,
                memory_address: item.absolute_address,
                address_space: item.space,
                element_path: element_path.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
            success_count += 1;
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
    
    Ok(ReadAllConfigValuesResponse {
        node_id,
        values,
        total_elements: total_count,
        successful_reads: success_count,
        failed_reads: error_count,
        duration_ms: duration,
    })
}

/// Cancel ongoing configuration reading operation
#[tauri::command]
pub async fn cancel_config_reading(state: tauri::State<'_, AppState>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.config_read_cancel.store(true, Ordering::Relaxed);
    Ok(())
}
