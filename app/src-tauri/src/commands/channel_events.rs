//! IPC command for resolving channel event IDs from cached config trees.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::state::AppState;

/// Request payload for batch channel event ID resolution.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelResolutionRequest {
    pub channel_id: String,
    pub node_key: String,
    pub connector: String,
    pub input: u32,
    /// State name → producerLeafIndex
    pub event_mapping: HashMap<String, u32>,
}

/// Response payload for a single channel's resolved event IDs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelResolutionResult {
    pub channel_id: String,
    /// State name → event ID hex string (16 chars uppercase)
    pub event_ids: HashMap<String, String>,
}

/// Resolve event IDs for a batch of channels using their cached config trees.
///
/// For each channel, looks up the tree in the node registry's proxy cache,
/// then uses the bowties-core resolution logic to extract producer event IDs
/// at the positions declared by the channel's event mapping.
///
/// Channels whose trees are not yet available return empty event_ids maps.
#[tauri::command]
pub async fn resolve_channel_event_ids(
    requests: Vec<ChannelResolutionRequest>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ChannelResolutionResult>, String> {
    let mut results = Vec::with_capacity(requests.len());

    for req in requests {
        let parsed_key = crate::node_key::NodeKey::parse(&req.node_key)
            .map_err(|e| format!("InvalidNodeKey: {}", e))?;

        let event_ids = if let Some(proxy) = state.node_registry.get_by_node_key(&parsed_key).await {
            if let Ok(Some(tree)) = proxy.get_config_tree().await {
                bowties_core::channel_events::resolve_channel_event_ids(
                    &tree,
                    &req.connector,
                    req.input,
                    &req.event_mapping,
                )
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        results.push(ChannelResolutionResult {
            channel_id: req.channel_id,
            event_ids,
        });
    }

    Ok(results)
}
