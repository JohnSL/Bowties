# Rust Command Signatures

**Feature**: 001-cdi-xml-viewer  
**Date**: February 16, 2026

## Overview

This document provides the Rust function signatures for the Tauri commands defined in this feature. These are reference signatures to guide implementation.

---

## Command: `get_cdi_xml`

### Location
`app/src-tauri/src/commands/cdi.rs`

### Signature

```rust
#[tauri::command]
pub async fn get_cdi_xml(
    node_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<GetCdiXmlResponse, String> {
    // Implementation retrieves CDI from node cache
    // Returns formatted response or error
}
```

### Request Type

```rust
// Inline parameter (String) - Tauri automatically deserializes from JSON
// Frontend passes: { nodeId: "01.02.03.04.05.06" }
// Rust receives: node_id as String
```

### Response Type

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCdiXmlResponse {
    /// Raw CDI XML content as string (null if not available)
    pub xml_content: Option<String>,
    
    /// Size of XML content in bytes (null if xml_content is null)
    pub size_bytes: Option<usize>,
    
    /// Timestamp when CDI was retrieved (ISO 8601 format)
    pub retrieved_at: Option<String>,
}
```

### Error Type

```rust
// Errors returned as String (Tauri converts to Promise rejection)
// Error messages should include error type prefix:
// - "CdiNotRetrieved: CDI not yet retrieved for node {node_id}"
// - "CdiUnavailable: Node {node_id} does not provide CDI"
// - "NodeNotFound: Node {node_id} not found"
// - "RetrievalFailed: {underlying error details}"
// - "InvalidXml: {parse error details}"

// Alternative: Use custom error enum with thiserror
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
}

// Convert CdiError to String for Tauri (implements Display via thiserror)
impl From<CdiError> for String {
    fn from(err: CdiError) -> String {
        err.to_string()
    }
}
```

---

## Implementation Pseudocode

```rust
#[tauri::command]
pub async fn get_cdi_xml(
    node_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<GetCdiXmlResponse, String> {
    // 1. Parse node ID
    let node_id = parse_node_id(&node_id)
        .map_err(|e| format!("InvalidNodeId: {}", e))?;
    
    // 2. Access node cache
    let nodes = state.nodes.read().await;
    
    // 3. Find node
    let node = nodes.get(&node_id)
        .ok_or_else(|| format!("NodeNotFound: Node {} not found", node_id))?;
    
    // 4. Check if CDI exists
    let cdi = node.cdi.as_ref()
        .ok_or_else(|| format!("CdiNotRetrieved: CDI not yet retrieved for node {}", node_id))?;
    
    // 5. Build response
    Ok(GetCdiXmlResponse {
        xml_content: Some(cdi.xml_content.clone()),
        size_bytes: Some(cdi.xml_content.len()),
        retrieved_at: Some(cdi.retrieved_at.to_rfc3339()),
    })
}

// Helper function to parse node ID
fn parse_node_id(s: &str) -> Result<NodeId, String> {
    // Accept formats: "01.02.03.04.05.06" or "010203040506"
    // Return NodeId struct or error
}
```

---

## State Structure

The command assumes the following state structure exists (or will be created):

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Application state (existing or to be created)
pub struct AppState {
    /// Cache of discovered nodes
    pub nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    // ... other state fields
}

/// Node structure (existing or to be enhanced)
pub struct Node {
    pub id: NodeId,
    pub snip: Option<SnipData>,
    pub cdi: Option<CdiData>, // New field for this feature (if not exists)
    // ... other fields
}

/// CDI data structure (new for this feature, if not exists)
#[derive(Debug, Clone)]
pub struct CdiData {
    /// Raw CDI XML content
    pub xml_content: String,
    
    /// Timestamp when CDI was retrieved
    pub retrieved_at: DateTime<Utc>,
}
```

---

## Registration

Command must be registered in Tauri builder:

```rust
// app/src-tauri/src/lib.rs or main.rs

use crate::commands::cdi::get_cdi_xml;

fn run() {
    tauri::Builder::default()
        // ... other setup
        .invoke_handler(tauri::generate_handler![
            // ... existing commands
            get_cdi_xml, // Add this
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## Testing

### Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    #[tokio::test]
    async fn test_get_cdi_xml_success() {
        // Setup mock state with node containing CDI
        let state = create_mock_state_with_cdi();
        
        let response = get_cdi_xml(
            "01.02.03.04.05.06".to_string(),
            state,
        ).await.unwrap();
        
        assert!(response.xml_content.is_some());
        assert!(response.size_bytes.is_some());
        assert!(response.retrieved_at.is_some());
    }
    
    #[tokio::test]
    async fn test_get_cdi_xml_node_not_found() {
        let state = create_mock_empty_state();
        
        let result = get_cdi_xml(
            "99.99.99.99.99.99".to_string(),
            state,
        ).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("NodeNotFound"));
    }
    
    #[tokio::test]
    async fn test_get_cdi_xml_not_retrieved() {
        // Node exists but CDI field is None
        let state = create_mock_state_without_cdi();
        
        let result = get_cdi_xml(
            "01.02.03.04.05.06".to_string(),
            state,
        ).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("CdiNotRetrieved"));
    }
}
```

---

## Dependencies

No new dependencies required:
- `serde` - Already in project (JSON serialization)
- `tauri` - Already in project (command framework)
- `tokio` - Already in project (async runtime)
- `chrono` - Already in project (timestamps)
- `thiserror` - Already in project (error types)

---

## Performance Considerations

- **XML Size**: For large CDI (>1MB), consider streaming or chunking (future enhancement)
- **Caching**: CDI already cached in node state (no re-fetch needed)
- **Async**: Command is async to avoid blocking Tauri event loop
- **Read Lock**: Uses read lock (not write) for concurrent access

---

## Security Considerations

- **Input Validation**: Node ID must be validated (prevent injection)
- **Size Limits**: Enforce max CDI size (10MB per spec)
- **Read-Only**: Command only reads data, doesn't modify state
- **Error Messages**: Don't leak sensitive information in errors

---

## Summary

The `get_cdi_xml` command provides a simple, type-safe interface to retrieve cached CDI XML from the backend node state. It follows Tauri command conventions and integrates seamlessly with the existing application architecture.

**Implementation Checklist**:
- [ ] Create `commands/cdi.rs` module
- [ ] Define `GetCdiXmlResponse` struct
- [ ] Define `CdiError` enum (optional, or use String)
- [ ] Implement `get_cdi_xml` command function
- [ ] Add `cdi: Option<CdiData>` field to `Node` struct (if not exists)
- [ ] Register command in Tauri builder
- [ ] Write unit tests for success and error cases
- [ ] Export command in `lib.rs`
