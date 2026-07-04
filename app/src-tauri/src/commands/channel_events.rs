//! IPC command for resolving channel event IDs from cached config trees.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::state::AppState;
use lcc_rs::cdi::EventRole;

/// Binding-shape discriminator for `ChannelResolutionRequest` — mirrors the
/// frontend `ChannelBinding` discriminated union, restricted to fields the
/// resolver needs.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChannelResolutionBinding {
    #[serde(rename_all = "camelCase")]
    ConnectorInput { connector: String, input: u32 },
    #[serde(rename_all = "camelCase")]
    LampRow { row_ordinal: u32 },
}

/// Role the resolver should match against the CDI tree (`producer` for
/// hardware-input channels, `consumer` for output channels driving the bus).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionRole {
    Producer,
    Consumer,
}

impl From<ResolutionRole> for EventRole {
    fn from(role: ResolutionRole) -> Self {
        match role {
            ResolutionRole::Producer => EventRole::Producer,
            ResolutionRole::Consumer => EventRole::Consumer,
        }
    }
}

/// Request payload for batch channel event ID resolution.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelResolutionRequest {
    pub channel_id: String,
    pub node_key: String,
    pub binding: ChannelResolutionBinding,
    pub role: ResolutionRole,
    /// State name → leaf ordinal within the role-filtered leaves under the
    /// binding's resolved path prefix (e.g. `producerLeafIndex` for
    /// connectorInput / Producer, `consumerLeafIndex` for lampRow / Consumer).
    pub leaf_index_map: HashMap<String, u32>,
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
/// Dispatches on `binding.kind`:
/// - `connectorInput` — uses the connector profile's `resolved_affected_paths`
///   to find the Line group prefix, then collects leaves matching `role`.
/// - `lampRow` — walks the `Direct Lamp Control` segment for `Lamp#N`, then
///   collects leaves matching `role`.
///
/// Channels whose trees are not yet available — or whose binding does not
/// resolve to a CDI path — return empty `event_ids` maps.
#[tauri::command]
pub async fn resolve_channel_event_ids(
    requests: Vec<ChannelResolutionRequest>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ChannelResolutionResult>, String> {
    let mut results = Vec::with_capacity(requests.len());

    for req in requests {
        let parsed_key = crate::node_key::NodeKey::parse(&req.node_key)
            .map_err(|e| format!("InvalidNodeKey: {}", e))?;

        let event_ids = {
            let layout_guard = state.layout_state.read().await;
            let tree_opt = layout_guard.as_ref().and_then(|ls| ls.config_tree(&parsed_key));
            if let Some(tree) = tree_opt {
                let path_prefix = match &req.binding {
                    ChannelResolutionBinding::ConnectorInput { connector, input } => {
                        bowties_core::channel_events::resolve_connector_input_path_prefix(
                            tree, connector, *input,
                        )
                    }
                    ChannelResolutionBinding::LampRow { row_ordinal } => {
                        bowties_core::channel_events::resolve_lamp_row_path_prefix(
                            tree,
                            *row_ordinal,
                        )
                    }
                };

                match path_prefix {
                    Some(prefix) => bowties_core::channel_events::resolve_event_ids(
                        tree,
                        &prefix,
                        req.role.into(),
                        &req.leaf_index_map,
                    ),
                    None => HashMap::new(),
                }
            } else {
                HashMap::new()
            }
        };

        results.push(ChannelResolutionResult {
            channel_id: req.channel_id,
            event_ids,
        });
    }

    Ok(results)
}
