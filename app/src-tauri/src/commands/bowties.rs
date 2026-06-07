//! Bowtie catalog: discovery, building, and query commands
//!
//! A "bowtie" is a shared LCC event ID that has at least one producer slot
//! and at least one consumer slot across the discovered nodes on the network.
//!
//! ## Data flow
//! 1. `read_all_config_values` completes for all nodes.
//! 2. `query_event_roles` sends `IdentifyEventsAddressed` to each node (125 ms apart)
//!    and collects `ProducerIdentified` / `ConsumerIdentified` replies for 500 ms.
//! 3. `build_bowtie_catalog` groups the resulting `NodeRoles` map into `BowtieCard`s.
//! 4. The catalog is stored in `AppState.bowties_catalog` and emitted as `cdi-read-complete`.
//! 5. `get_bowties` Tauri command lets the frontend retrieve the catalog on demand.
//!
//! Pure catalog-building logic lives in `bowties_core::bowtie::catalog`. This module
//! owns Tauri-specific orchestration: AppState reads, protocol queries, and IPC commands.

use std::collections::HashMap;
use tauri::{Emitter, Manager};

use crate::state::{AppState, BowtieCatalog, NodeRoles};
use crate::node_key::NodeKey;

// Re-export pure catalog functions so existing `crate::commands::bowties::*` call sites work.
pub use bowties_core::bowtie::catalog::{
    build_bowtie_catalog,
    merge_layout_metadata,
    extract_catalog_role_classifications,
    parse_event_id_hex,
    CdiReadCompletePayload,
};

// ── Protocol query ────────────────────────────────────────────────────────────

/// Send `IdentifyEventsAddressed` (MTI 0x0488) to each node and collect
/// `ProducerIdentified` / `ConsumerIdentified` replies.
///
/// **Timing** (per spec contract and JMRI reference):
/// - 125 ms between successive addressed sends.
/// - 500 ms collection window starting from the last send.
///
/// Returns a map of `event_id_bytes → NodeRoles` where `NodeRoles` records
/// which node IDs claimed to produce / consume each event.
///
/// If the connection or transport handle is unavailable the function returns an empty map.
pub async fn query_event_roles(
    state: &AppState,
    send_delay_ms: u64,
    collect_window_ms: u64,
) -> HashMap<[u8; 8], NodeRoles> {
    use lcc_rs::protocol::{GridConnectFrame, MTI};
    use lcc_rs::TransportHandle;
    use tokio::sync::broadcast;
    use tokio::time::{sleep, Duration};
    use std::time::Instant as StdInstant;

    // Grab connection + transport handle + own alias.
    let (_connection, handle, our_alias) = {
        let conn_lock = state.connection.read().await;
        let conn_opt = match conn_lock.as_ref() {
            Some(c) => c.clone(),
            None => {
                eprintln!("[bowties] query_event_roles: no connection");
                return HashMap::new();
            }
        };
        let (our_alias, handle) = {
            let c = conn_opt.lock().await;
            let alias = c.our_alias().value();
            let h = c.transport_handle().cloned();
            (alias, h)
        };
        let handle: TransportHandle = match handle {
            Some(h) => h,
            None => {
                eprintln!("[bowties] query_event_roles: no transport handle");
                return HashMap::new();
            }
        };
        (conn_opt, handle, our_alias)
    };

    // Read current node list from proxy registry
    let nodes = state.node_registry.get_all_snapshots().await;
    if nodes.is_empty() {
        return HashMap::new();
    }

    let exchange_start = StdInstant::now();
    let started_at = chrono::Utc::now();
    let nodes_queried = nodes.len();
    crate::bwlog!(state, "[bowties] query_event_roles: sending to {} nodes", nodes_queried);

    // Subscribe to all broadcast traffic so we catch the six relevant MTIs.
    let mut rx = handle.subscribe_all();

    // Send IdentifyEventsAddressed to each node, 125 ms apart.
    let mut events_sent: usize = 0;
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            sleep(Duration::from_millis(send_delay_ms)).await;
        }

        let dest_alias = node.alias.value();
        match GridConnectFrame::from_addressed_mti(
            MTI::IdentifyEventsAddressed,
            our_alias,
            dest_alias,
            vec![],
        ) {
            Ok(frame) => {
                if let Err(e) = handle.send(&frame).await {
                    eprintln!(
                        "[bowties] IdentifyEventsAddressed send error to {:?}: {}",
                        node.node_id, e
                    );
                } else {
                    events_sent += 1;
                }
            }
            Err(e) => {
                eprintln!(
                    "[bowties] frame build error for {:?}: {}",
                    node.node_id, e
                );
            }
        }
    }

    // Build alias → NodeKey lookup for fast resolution.
    let alias_to_node_key: HashMap<u16, NodeKey> = nodes
        .iter()
        .map(|n| (n.alias.value(), NodeKey::from_node_id(n.node_id)))
        .collect();

    // Collect replies for `collect_window_ms` ms.
    let mut roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();
    let mut responses_received: usize = 0;

    let collect_deadline = tokio::time::Instant::now()
        + Duration::from_millis(collect_window_ms);

    loop {
        let remaining = collect_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        let recv_result = tokio::time::timeout(remaining, rx.recv()).await;

        match recv_result {
            Ok(Ok(msg)) => {
                let frame = &msg.frame;
                // Extract (mti_value, source_alias) from header.
                let mti_value = (frame.header >> 12) & 0x1FFFF;
                let source_alias = (frame.header & 0xFFF) as u16;

                // Check for one of the six event-identified MTIs.
                // ProducerIdentified: Valid=0x19544, Invalid=0x19545, Unknown=0x19547
                // ConsumerIdentified: Valid=0x194C4, Invalid=0x194C5, Unknown=0x194C7
                let is_producer = mti_value == 0x19544
                    || mti_value == 0x19545
                    || mti_value == 0x19547;
                let is_consumer = mti_value == 0x194C4
                    || mti_value == 0x194C5
                    || mti_value == 0x194C7;

                if !is_producer && !is_consumer {
                    continue;
                }

                // Frame data = 8-byte event ID.
                if frame.data.len() < 8 {
                    continue;
                }
                let event_id: [u8; 8] = frame.data[..8].try_into().unwrap_or([0u8; 8]);

                // Resolve source_alias → NodeKey.
                let node_key = match alias_to_node_key.get(&source_alias) {
                    Some(nk) => *nk,
                    None => continue, // unknown alias — not one of our nodes
                };

                let entry = roles.entry(event_id).or_default();
                if is_producer {
                    entry.producers.insert(node_key);
                } else {
                    entry.consumers.insert(node_key);
                }
                responses_received += 1;
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                // Missed some frames — continue collecting
                continue;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                break;
            }
            Err(_) => {
                // Timeout — collection window ended
                break;
            }
        }
    }

    let duration_ms = exchange_start.elapsed().as_millis() as u64;
    crate::bwlog!(state,
        "[bowties] query_event_roles complete: {} event IDs, {} responses, {} nodes, {}ms",
        roles.len(), responses_received, nodes_queried, duration_ms);

    // Record EventRoleExchangeStats in diagnostics.
    {
        let stats = crate::diagnostics::EventRoleExchangeStats {
            started_at,
            nodes_queried,
            events_sent,
            responses_received,
            duration_ms,
        };
        state.diag_stats.write().await.event_role_exchange = Some(stats);
    }

    roles
}

// ── Layout file Tauri commands ────────────────────────────────────────────────

/// Load a YAML layout file from disk.
///
/// Validates the schema and emits a `layout-loaded` event on success.
#[tauri::command]
pub async fn load_layout(
    path: String,
    app: tauri::AppHandle,
) -> Result<crate::layout::types::LayoutFile, String> {
    let layout = crate::layout::io::load_file(std::path::Path::new(&path))?;

    // Emit layout-loaded event
    let _ = app.emit("layout-loaded", serde_json::json!({
        "path": path,
        "bowtieCount": layout.bowties.len(),
        "classificationCount": layout.role_classifications.len(),
    }));

    Ok(layout)
}

/// Save bowtie metadata and role classifications to a YAML layout file.
///
/// Uses atomic write (temp → flush → rename). Emits `layout-save-error` on failure.
#[tauri::command]
pub async fn save_layout(
    path: String,
    layout: crate::layout::types::LayoutFile,
    app: tauri::AppHandle,
) -> Result<(), String> {
    match crate::layout::io::save_file(std::path::Path::new(&path), &layout) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = app.emit("layout-save-error", serde_json::json!({
                "path": path,
                "error": e,
            }));
            Err(e)
        }
    }
}

/// Retrieve the most recently opened layout file path from app data dir.
#[tauri::command]
pub async fn get_recent_layout(
    app: tauri::AppHandle,
) -> Result<Option<crate::layout::types::RecentLayout>, String> {
    let app_data = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let recent_path = app_data.join("recent-layout.json");
    if !recent_path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&recent_path)
        .map_err(|e| format!("Failed to read recent layout file: {}", e))?;

    let recent: crate::layout::types::RecentLayout = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse recent layout data: {}", e))?;

    // Verify the referenced file still exists
    if !std::path::Path::new(&recent.path).exists() {
        return Ok(None);
    }

    Ok(Some(recent))
}

/// Store the most recently opened layout file path in app data dir.
#[tauri::command]
pub async fn set_recent_layout(
    path: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let app_data = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    std::fs::create_dir_all(&app_data)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    let recent = crate::layout::types::RecentLayout {
        path,
        last_opened: chrono::Utc::now().to_rfc3339(),
    };

    let json = serde_json::to_string_pretty(&recent)
        .map_err(|e| format!("Failed to serialize recent layout: {}", e))?;

    let recent_path = app_data.join("recent-layout.json");
    std::fs::write(&recent_path, json)
        .map_err(|e| format!("Failed to write recent layout: {}", e))?;

    Ok(())
}

/// Clear the persisted startup layout marker from app data.
#[tauri::command]
pub async fn clear_recent_layout(
    app: tauri::AppHandle,
) -> Result<(), String> {
    let app_data = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let recent_path = app_data.join("recent-layout.json");
    if !recent_path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&recent_path)
        .map_err(|e| format!("Failed to clear recent layout file: {}", e))?;

    Ok(())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Rebuild the bowtie catalog, optionally merging layout file metadata.
///
/// Uses the current AppState (discovered nodes, event roles, config cache)
/// to build a fresh catalog, then merges layout metadata if provided.
/// Falls back to offline bowtie data (populated by `build_offline_node_tree`)
/// when the node_registry is empty (offline layout mode).
/// The result is stored in AppState and emitted via `cdi-read-complete`.
#[tauri::command]
pub async fn build_bowtie_catalog_command(
    layout_metadata: Option<crate::layout::types::LayoutFile>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BowtieCatalog, String> {
    let nodes_snap = state.node_registry.get_all_snapshots().await;
    let use_offline = nodes_snap.is_empty();

    // Gather config values: from live proxies (online) or offline cache
    let mut config_cache_snap: HashMap<NodeKey, HashMap<String, [u8; 8]>> = if !use_offline {
        let handles = state.node_registry.get_all_handles().await;
        let mut map = HashMap::new();
        for h in &handles {
            if let Ok(vals) = h.get_config_values().await {
                if !vals.is_empty() {
                    if let Some(nid) = h.node_id() {
                        map.insert(NodeKey::from_node_id(nid), vals);
                    }
                }
            }
        }
        map
    } else {
        state.offline_bowtie_data.read().await.config_values.clone()
    };

    // Merge pending offline config changes into snapshot-based config values
    // so the catalog reflects the user's pending event-ID assignments.
    if use_offline {
        let changes = state.offline_changes_cache.read().await;
        let pending_config: Vec<_> = changes.iter()
            .filter(|c| c.kind == crate::layout::offline_changes::OfflineChangeKind::Config
                && c.status == crate::layout::offline_changes::OfflineChangeStatus::Pending
                && c.node_key.is_some()
                && c.space.is_some()
                && c.offset.is_some())
            .collect();

        if !pending_config.is_empty() {
            let offline_data = state.offline_bowtie_data.read().await;

            // Build (space, address) → element-path maps from CDI XML,
            // only for nodes that have pending changes.
            let affected_nodes: std::collections::HashSet<String> = pending_config.iter()
                .filter_map(|c| c.node_key.clone())
                .collect();

            let mut address_to_path: HashMap<String, HashMap<(u8, u32), String>> = HashMap::new();
            for node_key_str in &affected_nodes {
                let nk = match NodeKey::parse(node_key_str) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
                if let Some(xml) = offline_data.cdi_xml.get(&nk) {
                    if let Ok(cdi) = lcc_rs::cdi::parser::parse_cdi(xml) {
                        let tree = crate::node_tree::build_node_config_tree(&nk.to_string(), &cdi);
                        let mut map = HashMap::new();
                        for leaf in crate::node_tree::collect_event_id_leaves(&tree) {
                            map.insert((leaf.space, leaf.address), leaf.path.join("/"));
                        }
                        address_to_path.insert(node_key_str.clone(), map);
                    }
                }
            }

            // Overlay each pending event-ID change onto config_cache_snap.
            for change in &pending_config {
                let node_key_str = change.node_key.as_ref().unwrap().clone();
                let nk = match NodeKey::parse(&node_key_str) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
                let space = change.space.unwrap();
                let address = u32::from_str_radix(
                    change.offset.as_ref().unwrap()
                        .trim_start_matches("0x").trim_start_matches("0X"),
                    16,
                ).unwrap_or(0);

                if let Some(addr_map) = address_to_path.get(&node_key_str) {
                    if let Some(path) = addr_map.get(&(space, address)) {
                        if let Some(bytes) = parse_event_id_hex(&change.planned_value) {
                            config_cache_snap.entry(nk)
                                .or_default()
                                .insert(path.clone(), bytes);
                        }
                    }
                }
            }
        }
    }

    // Gather profile group roles: from live proxy trees or offline cache
    let profile_group_roles: HashMap<String, lcc_rs::EventRole> = if !use_offline {
        let handles = state.node_registry.get_all_handles().await;
        let mut map = HashMap::new();
        for h in &handles {
            let nk = match h.node_id() {
                Some(id) => NodeKey::from_node_id(id),
                None => continue, // skip synthesized placeholders
            };
            if let Ok(Some(tree)) = h.get_config_tree().await {
                for leaf in crate::node_tree::collect_event_id_leaves(&tree).into_iter() {
                    if let Some(role) = leaf.event_role {
                        if role != lcc_rs::EventRole::Ambiguous {
                            let key = format!("{}:{}", nk, leaf.path.join("/"));
                            map.insert(key, role);
                        }
                    }
                }
            }
        }
        map
    } else {
        state.offline_bowtie_data.read().await.profile_roles.clone()
    };

    // Build synthetic DiscoveredNode list for offline (CDI-only, for slot walking)
    let offline_nodes: Vec<lcc_rs::DiscoveredNode>;
    let nodes_for_catalog: &[lcc_rs::DiscoveredNode] = if !use_offline {
        &nodes_snap
    } else {
        let offline_data = state.offline_bowtie_data.read().await;
        offline_nodes = offline_data.cdi_xml.iter().enumerate().map(|(i, (node_key, xml))| {
            let node_id = node_key.as_node_id()
                .unwrap_or_else(|| lcc_rs::NodeID::new([0; 6]));
            lcc_rs::DiscoveredNode {
                node_id,
                alias: lcc_rs::NodeAlias::new((0x100 + i) as u16)
                    .unwrap_or_else(|_| lcc_rs::NodeAlias::new(1).unwrap()),
                snip_data: None,
                snip_status: lcc_rs::types::SNIPStatus::Unknown,
                connection_status: lcc_rs::types::ConnectionStatus::Unknown,
                last_verified: None,
                last_seen: chrono::Utc::now(),
                cdi: Some(lcc_rs::types::CdiData {
                    xml_content: xml.clone(),
                    retrieved_at: chrono::Utc::now(),
                }),
                pip_flags: None,
                pip_status: lcc_rs::types::PIPStatus::Unknown,
            }
        }).collect();
        &offline_nodes
    };

    // Event roles: query from protocol (online) or empty (offline — no protocol exchange)
    let event_roles: HashMap<[u8; 8], NodeRoles> = if !use_offline {
        let existing = state.bowties_catalog.read().await;
        if existing.is_some() {
            drop(existing);
            query_event_roles(&state, 125, 500).await
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    let mut catalog = build_bowtie_catalog(
        nodes_for_catalog,
        &event_roles,
        &config_cache_snap,
        Some(&profile_group_roles),
    );

    // Merge layout metadata if provided
    if let Some(layout) = &layout_metadata {
        merge_layout_metadata(&mut catalog, layout);
    }

    // Store in AppState
    let node_count = nodes_for_catalog.len();
    *state.bowties_catalog.write().await = Some(catalog.clone());

    // Emit to frontend
    let _ = app.emit(
        "cdi-read-complete",
        CdiReadCompletePayload { catalog: catalog.clone(), node_count },
    );

    Ok(catalog)
}

/// Return the current `BowtieCatalog` from AppState.
///
/// Returns `null` (serialized as `None`) if no catalog has been built yet —
/// i.e., CDI reads have not completed or no nodes were present.
#[tauri::command]
pub async fn get_bowties(
    state: tauri::State<'_, AppState>,
) -> Result<Option<BowtieCatalog>, String> {
    let catalog = state.bowties_catalog.read().await;
    Ok(catalog.clone())
}

/// T034a: Set bowtie name/tags (emits offline change when offline, saves layout when online).
/// Returns a change ID if offline, empty string if online.
#[tauri::command]
pub async fn set_bowtie_metadata(
    event_id_hex: String,
    name: Option<String>,
    tags: Vec<String>,
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Check if offline mode is active
    let is_offline = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .map(|c| c.mode == crate::state::ActiveLayoutMode::OfflineFile)
            .unwrap_or(false)
    };

    if is_offline {
        // Offline mode: emit an offline change
        // For bowtie metadata, we use a JSON representation of the new state as planned_value
        let planned_value = serde_json::json!({
            "name": name,
            "tags": tags,
        }).to_string();

        let offline_change = crate::layout::offline_changes::OfflineChange {
            change_id: format!(
                "{}-{}",
                uuid::Uuid::new_v4(),
                chrono::Utc::now().timestamp_millis()
            ),
            kind: crate::layout::offline_changes::OfflineChangeKind::BowtieMetadata,
            node_key: None,
            space: None,
            offset: None,
            baseline_value: format!("event:{}", event_id_hex),
            planned_value,
            status: crate::layout::offline_changes::OfflineChangeStatus::Pending,
            error: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        offline_change.validate()?;

        // In-memory only pre-save path: upsert same bowtie metadata target.
        let mut change_id_out = offline_change.change_id.clone();
        {
            let mut cache = state.offline_changes_cache.write().await;
            if let Some(existing) = cache.iter_mut().find(|c|
                c.kind == crate::layout::offline_changes::OfflineChangeKind::BowtieMetadata
                    && c.baseline_value == offline_change.baseline_value
            ) {
                existing.planned_value = offline_change.planned_value.clone();
                existing.updated_at = offline_change.updated_at.clone();
                existing.status = crate::layout::offline_changes::OfflineChangeStatus::Pending;
                existing.error = None;
                change_id_out = existing.change_id.clone();
            } else {
                cache.push(offline_change);
            }
        }

        // Update pending count
        {
            let mut guard = state.active_layout.write().await;
            if let Some(ctx) = &mut *guard {
                ctx.pending_offline_change_count = state.offline_changes_cache.read().await.len();
            }
        }

        Ok(change_id_out)
    } else {
        // Online mode: save to layout file (Feature 009 style)
        // For now, just update the local catalog
        if let Some(catalog) = state.bowties_catalog.write().await.as_mut() {
            if let Some(card) = catalog.bowties.iter_mut().find(|c| c.event_id_hex == event_id_hex) {
                card.name = name;
                card.tags = tags;
            }
        }
        Ok(String::new())
    }
}

// Tests for the pure catalog-building functions (build_bowtie_catalog,
// merge_layout_metadata, extract_catalog_role_classifications, node_display_name,
// walk_cdi_slots, best_slot, slot_for_event_id) now live in
// bowties_core::bowtie::catalog::tests.


