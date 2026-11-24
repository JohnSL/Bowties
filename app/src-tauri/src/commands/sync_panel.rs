//! Offline sync panel commands.
//!
//! Implements layout-match scoring, sync session building, and selective apply.
//! Pure domain logic (scoring, classification, CDI field resolution, change helpers)
//! lives in `bowties_core::sync`. This module owns only the Tauri command wrappers
//! and AppState coordination.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tauri::Manager;
use crate::layout::pending_changes::{PendingChange, PendingChangeKind, PendingChangeStatus};
use crate::state::{AppState, ActiveLayoutMode, SyncMode};

use bowties_core::sync::writer::{
    self, BaselineUpdate, BusNodeError, LayoutNodeResources, LeafUpdate, MemoryPort,
};
use bowties_core::sync::classifier::compute_layout_match;

// Re-export types used in IPC contracts from bowties-core.
pub use bowties_core::sync::classifier::{
    LayoutMatchStatus, SyncSession,
    ApplySyncResult,
};

// ── CDI / layout resolution (Tauri-dependent, stays here) ───────────────────

fn load_layout_snapshot(
    canonical_node_id: &str,
    root_path: &str,
) -> Result<crate::layout::node_snapshot::NodeSnapshot, String> {
    let base_file = std::path::Path::new(root_path);
    crate::layout::read_node_snapshot(base_file, canonical_node_id)
}

fn resolve_layout_cdi_from_snapshot(
    snapshot: &crate::layout::node_snapshot::NodeSnapshot,
    root_path: &str,
    app: &tauri::AppHandle,
) -> Result<lcc_rs::cdi::Cdi, String> {
    let base_file = std::path::Path::new(root_path);

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {}", e))?;

    let xml = crate::layout::resolve_cdi_xml_for_snapshot(base_file, snapshot, &app_data_dir)
        .map_err(|e| format!("CDI not found for node {}: {}", snapshot.node_key, e))?;

    lcc_rs::cdi::parser::parse_cdi(&xml)
        .map_err(|e| format!("Cannot parse CDI for {}: {}", snapshot.node_key, e))
}

fn load_layout_node_resources(
    canonical_node_id: &str,
    root_path: &str,
    app: &tauri::AppHandle,
) -> Result<LayoutNodeResources, String> {
    let snapshot = load_layout_snapshot(canonical_node_id, root_path)?;
    let cdi = resolve_layout_cdi_from_snapshot(&snapshot, root_path, app)?;
    Ok(LayoutNodeResources { snapshot, cdi })
}

/// Adapts the live connection (peer sessions, node registry) and the active
/// layout's files to the writer's `MemoryPort`.
struct LivePort<'a> {
    state: &'a AppState,
    root_path: &'a str,
    app: &'a tauri::AppHandle,
}

impl LivePort<'_> {
    /// Resolve the peer session for a node. Placeholder (synthesized) nodes
    /// have no node ID and are not on the bus.
    async fn session(&self, node_key: &str) -> Result<lcc_rs::PeerSessionHandle, String> {
        let nk = crate::node_key::NodeKey::parse(node_key)
            .map_err(|_| format!("Invalid node key: {}", node_key))?;
        let node_id = self
            .state
            .node_registry
            .get_by_node_key(&nk)
            .await
            .and_then(|p| p.node_id())
            .ok_or_else(|| format!("Node not on bus: {}", node_key))?;
        self.state.peer_session(node_id).await
    }
}

impl MemoryPort for LivePort<'_> {
    fn load_layout_resources(&self, node_key: &str) -> Result<LayoutNodeResources, String> {
        load_layout_node_resources(node_key, self.root_path, self.app)
    }

    async fn resolve_bus_node(&self, node_key: &str) -> Result<(), BusNodeError> {
        let nk = crate::node_key::NodeKey::parse(node_key).map_err(|_| BusNodeError::InvalidNodeKey)?;
        match self.state.node_registry.get_by_node_key(&nk).await {
            Some(p) if p.node_id().is_some() => Ok(()),
            _ => Err(BusNodeError::NotOnBus),
        }
    }

    async fn peer_session_ready(&self, node_key: &str) -> Result<(), String> {
        self.session(node_key).await.map(|_| ())
    }

    async fn read_memory(
        &self,
        node_key: &str,
        space: u8,
        address: u32,
        size: u8,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, String> {
        let session = self.session(node_key).await?;
        session
            .read_memory(space, address, size, timeout_ms)
            .await
            .map(|(data, _timing)| data)
            .map_err(|e| e.to_string())
    }

    async fn write_memory(
        &self,
        node_key: &str,
        space: u8,
        address: u32,
        data: Vec<u8>,
        timeout_ms: u64,
    ) -> Result<(), String> {
        let session = self.session(node_key).await?;
        session
            .write_memory(space, address, data, timeout_ms)
            .await
            .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingChangeInput {
    pub kind: PendingChangeKind,
    pub node_id: Option<String>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingChangeRow {
    pub change_id: String,
    pub kind: PendingChangeKind,
    pub node_id: Option<String>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub status: PendingChangeStatus,
    pub error: Option<String>,
    pub updated_at: String,
}



#[tauri::command]
pub async fn set_pending_change(
    change: PendingChangeInput,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Verify active offline layout
    {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?;
    }

    // Generate unique change ID (UUID + timestamp)
    let change_id = format!(
        "{}-{}",
        Uuid::new_v4(),
        chrono::Utc::now().timestamp_millis()
    );

    // Create the pending change record
    let pending_change = PendingChange {
        change_id: change_id.clone(),
        kind: change.kind,
        node_key: change.node_id,
        space: change.space,
        offset: change.offset,
        baseline_value: change.baseline_value,
        planned_value: change.planned_value,
        status: PendingChangeStatus::Pending,
        error: None,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    // Validate the change
    pending_change.validate()?;

    // In-memory only pre-save path: upsert on same target.
    let (change_id_out, row_count) = {
        let mut guard = state.layout_state.write().await;
        let layout = guard.as_mut().ok_or_else(|| "No layout is open".to_string())?;
        let id = layout.upsert_pending_change(pending_change);
        (id, layout.pending_changes().len())
    };

    // Update pending count in active layout context
    {
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_change_count = row_count;
        }
    }

    Ok(change_id_out)
}

#[tauri::command]
pub async fn revert_pending_change(
    change_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    // Verify active offline layout
    {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?;
    }

    // Remove from in-memory cache
    let (removed, row_count) = {
        let mut guard = state.layout_state.write().await;
        match guard.as_mut() {
            Some(layout) => {
                let ids = std::collections::HashSet::from([change_id.clone()]);
                (layout.remove_pending_changes(&ids) > 0, layout.pending_changes().len())
            }
            None => (false, 0),
        }
    };

    if !removed {
        return Err(format!("Change not found: {}", change_id));
    }

    // Update pending count in active layout context
    {
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_change_count = row_count;
        }
    }

    Ok(true)
}

#[tauri::command]
pub async fn list_pending_changes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PendingChangeRow>, String> {
    // Verify active offline layout
    let _context = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?
    };

    let layout_guard = state.layout_state.read().await;
    let changes = layout_guard
        .as_ref()
        .map(|layout| layout.pending_changes())
        .unwrap_or(&[]);
    let rows = changes
        .iter()
        .map(|c| PendingChangeRow {
            change_id: c.change_id.clone(),
            kind: c.kind.clone(),
            node_id: c.node_key.clone(),
            space: c.space,
            offset: c.offset.clone(),
            baseline_value: c.baseline_value.clone(),
            planned_value: c.planned_value.clone(),
            status: c.status.clone(),
            error: c.error.clone(),
            updated_at: c.updated_at.clone(),
        })
        .collect();

    Ok(rows)
}

#[tauri::command]
pub async fn replace_pending_changes(
    changes: Vec<PendingChangeInput>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    // Verify active offline layout
    {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut rows = Vec::with_capacity(changes.len());
    for change in changes {
        let row = PendingChange {
            change_id: format!("{}-{}", Uuid::new_v4(), chrono::Utc::now().timestamp_millis()),
            kind: change.kind,
            node_key: change.node_id,
            space: change.space,
            offset: change.offset,
            baseline_value: change.baseline_value,
            planned_value: change.planned_value,
            status: PendingChangeStatus::Pending,
            error: None,
            updated_at: now.clone(),
        };
        row.validate()?;
        rows.push(row);
    }

    let count = rows.len();
    {
        let mut guard = state.layout_state.write().await;
        let layout = guard.as_mut().ok_or_else(|| "No layout is open".to_string())?;
        layout.set_pending_changes(rows);
    }

    let mut guard = state.active_layout.write().await;
    if let Some(ctx) = &mut *guard {
        ctx.pending_change_count = count;
    }

    Ok(count)
}

#[tauri::command]
pub async fn compute_layout_match_status(
    discovered_node_ids: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<LayoutMatchStatus, String> {
    let context = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "No layout is active".to_string())?
    };

    let layout_ids: std::collections::HashSet<String> = context
        .layout_node_keys
        .iter()
        .map(|nk| nk.to_string())
        .collect();

    let discovered_ids: std::collections::HashSet<String> = discovered_node_ids
        .iter()
        .map(|id| id.replace('.', "").to_uppercase())
        .collect();

    Ok(compute_layout_match(&layout_ids, &discovered_ids))
}

#[tauri::command]
pub async fn build_sync_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncSession, String> {
    // Must have an active layout and be connected
    let context = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "No layout is active".to_string())?
    };

    if !state.is_connected().await {
        return Err("Not connected to network".to_string());
    }

    let changes = current_pending_changes(&state).await;
    let port = LivePort { state: &state, root_path: &context.root_path, app: &app };
    let outcome = writer::classify_session(&port, &changes).await;
    let already_applied_ids = outcome.already_applied_ids;

    if !already_applied_ids.is_empty() {
        let cache_snapshot = {
            let mut guard = state.layout_state.write().await;
            match guard.as_mut() {
                Some(layout) => {
                    layout.remove_pending_changes(&already_applied_ids);
                    layout.pending_changes().to_vec()
                }
                None => Vec::new(),
            }
        };

        {
            let mut guard = state.active_layout.write().await;
            if let Some(ctx) = &mut *guard {
                ctx.pending_change_count = cache_snapshot.len();
            }
        }

        if let Err(e) = crate::layout::update_pending_changes(
            std::path::Path::new(&context.root_path),
            &cache_snapshot,
        ) {
            eprintln!("[sync] Failed to persist offline-changes after already-applied clear: {}", e);
        }
    }

    Ok(outcome.session)
}

#[cfg(test)]
mod tests {
    use bowties_core::sync::changes::remove_changes_by_id;
    use bowties_core::sync::field_meta::{find_field_meta_in_cdi, find_snapshot_field_label};
    use crate::layout::node_snapshot::{NodeSnapshot, SnapshotLeafValue};
    use crate::layout::pending_changes::{PendingChange, PendingChangeKind, PendingChangeStatus};
    use crate::layout::node_snapshot::{CdiReference, CaptureStatus, SnipSnapshot};
    use lcc_rs::NodeID;
    use lcc_rs::cdi::{Cdi, DataElement, EventIdElement, Group, Segment};
    use std::collections::HashSet;

    fn make_change(change_id: &str) -> PendingChange {
        PendingChange {
            change_id: change_id.to_string(),
            kind: PendingChangeKind::Config,
            node_key: Some(NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00]).to_canonical()),
            space: Some(253),
            offset: Some("0x00000010".to_string()),
            baseline_value: "10".to_string(),
            planned_value: "20".to_string(),
            status: PendingChangeStatus::Pending,
            error: None,
            updated_at: "2026-04-25T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn removes_only_cleared_change_ids_from_cache() {
        let mut cache = vec![make_change("row-1"), make_change("row-2"), make_change("row-3")];
        let cleared_ids = HashSet::from(["row-2".to_string(), "row-3".to_string()]);

        let removed = remove_changes_by_id(&mut cache, &cleared_ids);

        assert_eq!(removed, 2);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].change_id, "row-1");
    }

    #[test]
    fn does_not_change_cache_when_no_ids_match() {
        let mut cache = vec![make_change("row-1")];
        let cleared_ids = HashSet::from(["row-9".to_string()]);

        let removed = remove_changes_by_id(&mut cache, &cleared_ids);

        assert_eq!(removed, 0);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache[0].change_id, "row-1");
    }

    #[test]
    fn finds_snapshot_field_label_from_saved_path_tree() {
        let mut snapshot = NodeSnapshot {
            node_key: NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9]).to_canonical(),
            node_id: Some(NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9])),
            profile_stem: None,
            lifecycle: crate::layout::node_snapshot::NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-04-26T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot::default(),
            cdi_ref: CdiReference {
                cache_key: "cache".to_string(),
                version: "1.0".to_string(),
                fingerprint: "fp".to_string(),
            },
            config: Default::default(),
            producer_identified_events: Vec::new(),
        };

        snapshot.add_config_leaf(
            &[
                "Port I/O".to_string(),
                "Line(2)".to_string(),
                "Event(0)".to_string(),
                "Indicator".to_string(),
            ],
            SnapshotLeafValue {
                value: "02.01.57.00.02.D9.02.66".to_string(),
                space: Some(253),
                offset: Some("0x00000010".to_string()),
            },
        );

        assert_eq!(
            find_snapshot_field_label(&snapshot, 253, "0x00000010").as_deref(),
            Some("Port I/O.Line(2).Event(0).Indicator")
        );
    }

    #[test]
    fn finds_cdi_field_label_for_replicated_event_path() {
        let cdi = Cdi {
            identification: None,
            acdi: None,
            segments: vec![Segment {
                name: Some("Port I/O".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: vec![DataElement::Group(Group {
                    name: Some("Line".to_string()),
                    description: None,
                    offset: 0,
                    replication: 3,
                    repname: Vec::new(),
                    elements: vec![DataElement::Group(Group {
                        name: Some("Event".to_string()),
                        description: None,
                        offset: 0,
                        replication: 2,
                        repname: Vec::new(),
                        elements: vec![DataElement::EventId(EventIdElement {
                            name: Some("Indicator".to_string()),
                            description: None,
                            offset: 0,
                        })],
                        hints: None,
                    })],
                    hints: None,
                })],
            }],
        };

        let meta = find_field_meta_in_cdi(&cdi, 253, 32).expect("expected field metadata");

        assert_eq!(meta.field_label, "Port I/O.Line(2).Event(0).Indicator");
    }

    #[test]
    fn pending_change_kind_serializes_to_camel_case() {
        // PendingChangeKind has serde(rename_all = "camelCase"), so serde
        // serialization must produce "bowtieMetadata" not "bowtiemetadata".
        // The kind round-trips through list → replace via serde, so this
        // validates the enum variants produce correct IPC strings.
        let json = serde_json::to_string(&PendingChangeKind::BowtieMetadata).unwrap();
        assert_eq!(json, r#""bowtieMetadata""#);

        let json = serde_json::to_string(&PendingChangeKind::BowtieEvent).unwrap();
        assert_eq!(json, r#""bowtieEvent""#);

        let json = serde_json::to_string(&PendingChangeKind::Config).unwrap();
        assert_eq!(json, r#""config""#);
    }

    #[test]
    fn pending_change_kind_round_trips_through_serde_json() {
        // Deserializing a camelCase string must produce the correct enum variant.
        let kind: PendingChangeKind = serde_json::from_str(r#""bowtieMetadata""#).unwrap();
        assert_eq!(kind, PendingChangeKind::BowtieMetadata);

        let kind: PendingChangeKind = serde_json::from_str(r#""bowtieEvent""#).unwrap();
        assert_eq!(kind, PendingChangeKind::BowtieEvent);

        let kind: PendingChangeKind = serde_json::from_str(r#""config""#).unwrap();
        assert_eq!(kind, PendingChangeKind::Config);
    }

    /// S8.11-T1: a placeholder NodeKey must round-trip through PendingChange
    /// without crashing. Previously, `parse_opt_node_id` called
    /// `NodeID::from_hex_string` which rejects `placeholder:*` keys.
    #[test]
    fn pending_change_accepts_placeholder_node_key() {
        let placeholder_key = "placeholder:a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string();
        let change = PendingChange {
            change_id: "test-placeholder".to_string(),
            kind: PendingChangeKind::Config,
            node_key: Some(placeholder_key.clone()),
            space: Some(253),
            offset: Some("0x00000010".to_string()),
            baseline_value: "0".to_string(),
            planned_value: "1".to_string(),
            status: PendingChangeStatus::Pending,
            error: None,
            updated_at: "2026-05-26T00:00:00Z".to_string(),
        };
        // Round-trip through serde_yaml (disk format)
        let yaml = serde_yaml_ng::to_string(&change).unwrap();
        let restored: PendingChange = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(restored.node_key.as_deref(), Some(placeholder_key.as_str()));
    }

    /// S8.11-T1: backward compat — existing changes with canonical hex NodeIDs
    /// still round-trip correctly after the type change.
    #[test]
    fn pending_change_round_trips_hex_node_id() {
        let change = make_change("compat-test");
        let yaml = serde_yaml_ng::to_string(&change).unwrap();
        let restored: PendingChange = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(restored.node_key, change.node_key);
    }
}

#[tauri::command]
pub async fn set_sync_mode(
    mode: SyncMode,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mode_str = match &mode {
        SyncMode::TargetLayoutBus => "target_layout_bus",
        SyncMode::BenchOtherBus => "bench_other_bus",
    };
    *state.sync_mode.write().await = Some(mode);
    Ok(mode_str.to_string())
}

/// Snapshot of the open layout's Pending Changes (empty when no layout is open).
async fn current_pending_changes(state: &AppState) -> Vec<PendingChange> {
    state
        .layout_state
        .read()
        .await
        .as_ref()
        .map(|layout| layout.pending_changes().to_vec())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn apply_sync_changes(
    apply_change_ids: Vec<String>,
    skip_change_ids: Vec<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ApplySyncResult, String> {
    // Must have an active layout
    let context = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "No layout is active".to_string())?
    };

    // Must be connected
    let connection = state.connection_arc().await
        .ok_or("Not connected to network")?;

    let changes = current_pending_changes(&state).await;
    let port = LivePort { state: &state, root_path: &context.root_path, app: &app };
    let outcome = writer::apply_changes(&port, &changes, &apply_change_ids, &skip_change_ids).await;

    // Commit values / read-only marks in the LayoutState tree
    if !outcome.leaf_updates.is_empty() {
        let mut layout_guard = state.layout_state.write().await;
        if let Some(ls) = layout_guard.as_mut() {
            for update in &outcome.leaf_updates {
                let node_key = match update {
                    LeafUpdate::Commit { node_key, .. } | LeafUpdate::MarkReadOnly { node_key, .. } => node_key,
                };
                let Ok(nk) = crate::node_key::NodeKey::parse(node_key) else { continue };
                let Some(tree) = ls.config_tree_mut(&nk) else { continue };
                match update {
                    LeafUpdate::Commit { space, address, value, .. } => {
                        crate::node_tree::update_leaf_value(tree, *space, *address, value.clone());
                    }
                    LeafUpdate::MarkReadOnly { space, address, .. } => {
                        crate::node_tree::revert_and_mark_leaf_read_only(tree, *space, *address);
                    }
                }
            }
        }
    }

    // Send Update Complete to each node that had successful writes
    for node_key in &outcome.nodes_written {
        let nk = match crate::node_key::NodeKey::parse(node_key) {
            Ok(nk) => nk,
            Err(_) => continue,
        };
        if let Some(proxy) = state.node_registry.get_by_node_key(&nk).await {
            let mut conn = connection.lock().await;
            if let Err(e) = conn.send_update_complete(proxy.alias()).await {
                eprintln!("[sync] Update Complete failed for {}: {}", node_key, e);
            }
        }
    }

    // Remove applied and read-only-cleared rows from pending changes cache
    {
        let all_cleared = outcome.cleared_ids();

        let row_count = {
            let mut guard = state.layout_state.write().await;
            match guard.as_mut() {
                Some(layout) => {
                    layout.remove_pending_changes(&all_cleared);
                    layout.pending_changes().len()
                }
                None => 0,
            }
        };

        // Update pending count
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_change_count = row_count;
        }
    }

    // Persist pending changes and update node snapshot baselines on disk
    if !outcome.cleared_ids().is_empty() {
        let base_file = std::path::Path::new(&context.root_path);

        // Write updated pending changes
        let cache_snapshot = current_pending_changes(&state).await;
        if let Err(e) = crate::layout::update_pending_changes(base_file, &cache_snapshot) {
            eprintln!("[sync] Failed to persist offline-changes: {}", e);
        }

        // Update node snapshot baselines for successfully applied config changes
        let mut updated_snapshots: std::collections::HashMap<String, crate::layout::node_snapshot::NodeSnapshot> =
            std::collections::HashMap::new();
        let applied_at = chrono::Utc::now().to_rfc3339();

        for BaselineUpdate { node_key: canonical_nid, space, offset: offset_hex, planned_value } in &outcome.baseline_updates {
            let snapshot = updated_snapshots.entry(canonical_nid.clone()).or_insert_with(|| {
                match crate::layout::read_node_snapshot(base_file, canonical_nid) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[sync] Failed to read snapshot for {}: {}", canonical_nid, e);
                        // Return a dummy that won't be saved (sentinel: empty node_key)
                        crate::layout::node_snapshot::NodeSnapshot {
                            node_key: String::new(),
                            node_id: None,
                            profile_stem: None,
                            lifecycle: crate::layout::node_snapshot::NodeSnapshotLifecycle::Persisted,
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
                    }
                }
            });

            crate::layout::node_snapshot::update_snapshot_baseline_and_capture_time(
                snapshot,
                *space,
                offset_hex,
                planned_value,
                &applied_at,
            );
        }

        // Write back modified snapshots in one partial-update call
        let to_write: Vec<crate::layout::node_snapshot::NodeSnapshot> = updated_snapshots
            .into_values()
            .filter(|s| !s.node_key.is_empty())
            .collect();
        if !to_write.is_empty() {
            if let Err(e) = crate::layout::update_node_snapshots(base_file, &to_write) {
                eprintln!("[sync] Failed to persist snapshot updates: {}", e);
            }
        }
    }

    Ok(outcome.result)
}
