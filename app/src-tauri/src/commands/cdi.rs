//! CDI (Configuration Description Information) XML viewer commands

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::Utc;
use tauri::Manager;
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
    // Get CDI XML from cache
    let cdi_response = get_cdi_xml(node_id.clone(), app_handle, state.clone()).await?;
    
    let xml_content = cdi_response
        .xml_content
        .ok_or_else(|| CdiError::CdiNotRetrieved(node_id.clone()))?;
    
    // Parse CDI XML
    let cdi = lcc_rs::cdi::parser::parse_cdi(&xml_content)
        .map_err(CdiError::InvalidXml)?;
    
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
