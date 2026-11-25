//! Sync Panel writer — classifies Pending Change rows against the bus and
//! applies chosen rows, behind a [`MemoryPort`].
//!
//! Extracted from `app/src-tauri/src/commands/sync_panel.rs` so the decision
//! logic (which rows are already applied, clean, conflicting or missing; which
//! rows a write clears; which leaf and baseline updates follow) is testable
//! without a bus or Tauri. The Tauri commands adapt the live connection to the
//! port, call this module, and own disk persistence and event emission.

use std::collections::{HashMap, HashSet};

use lcc_rs::cdi::Cdi;

use crate::layout::node_snapshot::NodeSnapshot;
use crate::layout::pending_changes::{PendingChange, PendingChangeKind, PendingChangeStatus};
use crate::node_tree::ConfigValue;

use super::classifier::{
    build_sync_row, classify_sync_row, ApplySyncFailure, ApplySyncResult, SyncClassification,
    SyncRow, SyncSession,
};
use super::field_meta::{
    fallback_field_label, field_meta_to_leaf, find_field_meta_in_cdi, find_snapshot_field_label,
    parse_offset, raw_bytes_to_value_string, resolve_snapshot_node_name, serialize_config_value,
    string_to_config_value,
};

/// Timeout for each targeted read and each write in the Sync Panel.
pub const SYNC_MEMORY_TIMEOUT_MS: u64 = 3000;

/// Largest single read issued for one row.
pub const MAX_ROW_READ_BYTES: u32 = 64;

/// Why a node key could not be resolved to a node on the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusNodeError {
    /// The key does not parse as a node key.
    InvalidNodeKey,
    /// No real (non-placeholder) node with this key is known on the bus.
    NotOnBus,
}

/// Saved snapshot and parsed CDI for a node in the active layout.
#[derive(Debug, Clone)]
pub struct LayoutNodeResources {
    pub snapshot: NodeSnapshot,
    pub cdi: Cdi,
}

/// Boundary to the live network and the layout files. The app adapts its peer
/// sessions, node registry and layout directory to this.
#[allow(async_fn_in_trait)]
pub trait MemoryPort {
    /// Load the saved snapshot and CDI for `node_key` from the active layout.
    fn load_layout_resources(&self, node_key: &str) -> Result<LayoutNodeResources, String>;

    /// Confirm `node_key` is a node currently on the bus.
    async fn resolve_bus_node(&self, node_key: &str) -> Result<(), BusNodeError>;

    /// Confirm a peer session is available for the node.
    async fn peer_session_ready(&self, node_key: &str) -> Result<(), String>;

    async fn read_memory(
        &self,
        node_key: &str,
        space: u8,
        address: u32,
        size: u8,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, String>;

    async fn write_memory(
        &self,
        node_key: &str,
        space: u8,
        address: u32,
        data: Vec<u8>,
        timeout_ms: u64,
    ) -> Result<(), String>;
}

/// Result of classifying the pending rows against the bus.
#[derive(Debug, Clone)]
pub struct SessionOutcome {
    pub session: SyncSession,
    /// Rows whose bus value already equals the planned value; the caller
    /// removes them from the pending rows and persists.
    pub already_applied_ids: HashSet<String>,
    /// Leaf commits for the already-applied rows: the board holds the planned
    /// value, so the tree shows it (same shape a write produces).
    pub leaf_updates: Vec<LeafUpdate>,
    /// Snapshot baselines for the already-applied rows, as a write would move them.
    pub baseline_updates: Vec<BaselineUpdate>,
}

fn sync_row(
    change: &PendingChange,
    node_name: Option<String>,
    field_label: Option<String>,
    bus_value: Option<String>,
    error: Option<String>,
) -> SyncRow {
    build_sync_row(
        &change.change_id,
        change.node_key.as_deref(),
        node_name,
        field_label,
        &change.baseline_value,
        &change.planned_value,
        bus_value,
        error,
    )
}

/// Read each pending row's field from the bus and classify it.
pub async fn classify_session<P: MemoryPort>(
    port: &P,
    changes: &[PendingChange],
) -> SessionOutcome {
    let mut conflict_rows = Vec::new();
    let mut clean_rows = Vec::new();
    let mut already_applied_count: usize = 0;
    let mut already_applied_ids = HashSet::new();
    let mut leaf_updates = Vec::new();
    let mut baseline_updates = Vec::new();
    let mut node_missing_rows = Vec::new();

    // Cache snapshot + parsed CDI per node to avoid reloading for each change.
    let mut resources_cache: HashMap<String, LayoutNodeResources> = HashMap::new();

    for change in changes.iter().filter(|c| c.status.is_waiting()) {
        // Only config changes go through bus comparison; bowtie metadata
        // changes are always clean (they don't live on the bus).
        if change.kind != PendingChangeKind::Config {
            clean_rows.push(sync_row(change, None, None, None, None));
            continue;
        }

        let node_key = match &change.node_key {
            Some(key) => key.clone(),
            None => {
                node_missing_rows.push(sync_row(
                    change,
                    None,
                    fallback_field_label(change.space, change.offset.as_deref()),
                    None,
                    Some("No node ID".to_string()),
                ));
                continue;
            }
        };

        if !resources_cache.contains_key(&node_key) {
            if let Ok(resources) = port.load_layout_resources(&node_key) {
                resources_cache.insert(node_key.clone(), resources);
            }
        }
        let resources = resources_cache.get(&node_key);
        let node_name = resources
            .map(|res| resolve_snapshot_node_name(&res.snapshot, &node_key))
            .or_else(|| Some(node_key.clone()));
        let field_label = if let (Some(space), Some(offset)) = (change.space, change.offset.as_deref()) {
            resources
                .and_then(|res| {
                    find_snapshot_field_label(&res.snapshot, space, offset).or_else(|| {
                        parse_offset(offset).and_then(|address| {
                            find_field_meta_in_cdi(&res.cdi, space, address).map(|meta| meta.field_label)
                        })
                    })
                })
                .or_else(|| fallback_field_label(change.space, change.offset.as_deref()))
        } else {
            fallback_field_label(change.space, change.offset.as_deref())
        };

        // Synthesized (placeholder) nodes are not on the bus — treat as missing.
        if port.resolve_bus_node(&node_key).await.is_err() {
            node_missing_rows.push(sync_row(change, node_name, field_label, None, None));
            continue;
        }

        let space = change.space.unwrap_or(0);
        let address = change.offset.as_deref().and_then(parse_offset).unwrap_or(0);

        let cdi = match resources {
            Some(resources) => &resources.cdi,
            None => {
                node_missing_rows.push(sync_row(
                    change,
                    node_name,
                    field_label,
                    None,
                    Some("CDI unavailable for sync row".to_string()),
                ));
                continue;
            }
        };

        // Find the field metadata (type, size) from CDI
        let meta = match find_field_meta_in_cdi(cdi, space, address) {
            Some(m) => m,
            None => {
                node_missing_rows.push(sync_row(
                    change,
                    node_name,
                    field_label,
                    None,
                    Some(format!(
                        "Field not found in CDI at space={} address={:#010x}",
                        space, address
                    )),
                ));
                continue;
            }
        };

        // Targeted read: fetch only this field from the live bus.
        let read_size = meta.size.min(MAX_ROW_READ_BYTES) as u8;
        let raw = match port
            .read_memory(&node_key, space, address, read_size, SYNC_MEMORY_TIMEOUT_MS)
            .await
        {
            Ok(data) => data,
            Err(e) => {
                node_missing_rows.push(sync_row(
                    change,
                    node_name,
                    field_label,
                    None,
                    Some(format!("Read failed: {}", e)),
                ));
                continue;
            }
        };

        let bus_value = match raw_bytes_to_value_string(&meta, &raw) {
            Some(v) => v,
            None => {
                node_missing_rows.push(sync_row(
                    change,
                    node_name,
                    field_label,
                    None,
                    Some("Cannot parse bus value".to_string()),
                ));
                continue;
            }
        };

        match classify_sync_row(&bus_value, &change.baseline_value, &change.planned_value) {
            SyncClassification::AlreadyApplied => {
                already_applied_count += 1;
                already_applied_ids.insert(change.change_id.clone());
                if let Some(value) = planned_config_value(&meta, space, address, &change.planned_value) {
                    leaf_updates.push(LeafUpdate::Commit {
                        node_key: node_key.clone(),
                        space,
                        address,
                        value,
                    });
                    baseline_updates.extend(change.offset.as_ref().map(|offset| BaselineUpdate {
                        node_key: node_key.clone(),
                        space,
                        offset: offset.clone(),
                        planned_value: change.planned_value.clone(),
                    }));
                }
            }
            SyncClassification::Clean => {
                clean_rows.push(sync_row(
                    change,
                    node_name,
                    Some(field_label.unwrap_or_else(|| meta.field_label.clone())),
                    Some(bus_value),
                    None,
                ));
            }
            SyncClassification::Conflict => {
                conflict_rows.push(sync_row(
                    change,
                    node_name,
                    Some(field_label.unwrap_or_else(|| meta.field_label.clone())),
                    Some(bus_value),
                    None,
                ));
            }
        }
    }

    SessionOutcome {
        session: SyncSession {
            conflict_rows,
            clean_rows,
            already_applied_count,
            node_missing_rows,
        },
        already_applied_ids,
        leaf_updates,
        baseline_updates,
    }
}

/// A change to the in-memory config tree that follows an apply.
#[derive(Debug, Clone, PartialEq)]
pub enum LeafUpdate {
    /// The board took the value: commit it to the tree leaf.
    Commit { node_key: String, space: u8, address: u32, value: ConfigValue },
    /// The board refused the write as read-only (0x1083): mark the leaf read-only.
    MarkReadOnly { node_key: String, space: u8, address: u32 },
}

/// A saved-snapshot baseline to move to the value just written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineUpdate {
    pub node_key: String,
    pub space: u8,
    pub offset: String,
    pub planned_value: String,
}

/// Result of applying chosen rows, with the follow-up updates as data. The
/// caller removes the cleared rows, applies the updates, persists, and sends
/// Update Complete to `nodes_written`.
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub result: ApplySyncResult,
    pub nodes_written: HashSet<String>,
    pub leaf_updates: Vec<LeafUpdate>,
    pub baseline_updates: Vec<BaselineUpdate>,
}

impl ApplyOutcome {
    /// Rows to remove from the pending rows: applied and read-only-cleared.
    pub fn cleared_ids(&self) -> HashSet<String> {
        self.result
            .applied
            .iter()
            .chain(self.result.read_only_cleared.iter())
            .cloned()
            .collect()
    }
}

/// Node keys whose config tree a set of leaf updates changes, each once, in
/// first-seen order. The caller tells the frontend to reload these trees.
pub fn changed_node_keys(updates: &[LeafUpdate]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for update in updates {
        let (LeafUpdate::Commit { node_key, .. } | LeafUpdate::MarkReadOnly { node_key, .. }) = update;
        if !keys.contains(node_key) {
            keys.push(node_key.clone());
        }
    }
    keys
}

/// A Config row resolved to the bytes to write.
struct PreparedWrite {
    node_key: String,
    space: u8,
    address: u32,
    config_val: ConfigValue,
    bytes: Vec<u8>,
}

/// Resolve a Config row to its node, field metadata and bytes: the node must be
/// on the bus with a peer session, and the planned value must convert.
async fn prepare_write<P: MemoryPort>(
    port: &P,
    change: &PendingChange,
    cdi_cache: &mut HashMap<String, Cdi>,
) -> Result<PreparedWrite, String> {
    let node_key = match &change.node_key {
        Some(key) => key.clone(),
        None => return Err("No node ID on change".to_string()),
    };

    // Synthesized (placeholder) nodes cannot be written to the bus.
    if let Err(e) = port.resolve_bus_node(&node_key).await {
        return Err(match e {
            BusNodeError::NotOnBus => format!("Node not found on bus: {}", node_key),
            BusNodeError::InvalidNodeKey => format!("Invalid node key: {}", node_key),
        });
    }
    port.peer_session_ready(&node_key).await?;

    let space = change.space.unwrap_or(0);
    let address = change.offset.as_deref().and_then(parse_offset).unwrap_or(0);

    // Resolve field metadata from layout CDI
    if !cdi_cache.contains_key(&node_key) {
        let res = port
            .load_layout_resources(&node_key)
            .map_err(|e| format!("CDI unavailable: {}", e))?;
        cdi_cache.insert(node_key.clone(), res.cdi);
    }
    let cdi = &cdi_cache[&node_key];

    let meta = find_field_meta_in_cdi(cdi, space, address).ok_or_else(|| {
        format!("Field not found in CDI at space={} address={:#010x}", space, address)
    })?;

    // Build a synthetic leaf for value conversion
    let config_val = planned_config_value(&meta, space, address, &change.planned_value).ok_or_else(|| {
        format!(
            "Cannot convert planned value '{}' to bytes for leaf type {:?}",
            change.planned_value, meta.leaf_type
        )
    })?;
    let bytes = serialize_config_value(&config_val, meta.leaf_type, meta.size);

    Ok(PreparedWrite { node_key, space, address, config_val, bytes })
}

/// The planned value as the typed config value its field holds, or `None` when
/// it does not convert.
fn planned_config_value(
    meta: &crate::sync::field_meta::FieldMeta,
    space: u8,
    address: u32,
    planned_value: &str,
) -> Option<ConfigValue> {
    let leaf = field_meta_to_leaf(meta, space, address);
    string_to_config_value(planned_value, &leaf)
}

async fn write_prepared<P: MemoryPort>(port: &P, prepared: &PreparedWrite) -> Result<(), String> {
    port.write_memory(
        &prepared.node_key,
        prepared.space,
        prepared.address,
        prepared.bytes.clone(),
        SYNC_MEMORY_TIMEOUT_MS,
    )
    .await
}

/// Error 0x1083 = read-only field (FR-017b).
fn is_read_only_error(err: &str) -> bool {
    err.contains("1083")
}

fn baseline_update_for(change: &PendingChange, prepared: &PreparedWrite) -> Option<BaselineUpdate> {
    change.offset.as_ref().map(|offset| BaselineUpdate {
        node_key: prepared.node_key.clone(),
        space: prepared.space,
        offset: offset.clone(),
        planned_value: change.planned_value.clone(),
    })
}

fn commit_update(prepared: PreparedWrite) -> LeafUpdate {
    LeafUpdate::Commit {
        node_key: prepared.node_key,
        space: prepared.space,
        address: prepared.address,
        value: prepared.config_val,
    }
}

fn mark_read_only_update(prepared: &PreparedWrite) -> LeafUpdate {
    LeafUpdate::MarkReadOnly {
        node_key: prepared.node_key.clone(),
        space: prepared.space,
        address: prepared.address,
    }
}

/// Write the chosen rows to the bus. `skip_ids` are only echoed; their rows stay pending.
pub async fn apply_changes<P: MemoryPort>(
    port: &P,
    changes: &[PendingChange],
    apply_ids: &[String],
    skip_ids: &[String],
) -> ApplyOutcome {
    let mut applied = Vec::new();
    let skipped: Vec<String> = skip_ids.to_vec();
    let mut failed = Vec::new();
    let mut read_only_cleared = Vec::new();
    let mut nodes_written = HashSet::new();
    let mut leaf_updates = Vec::new();
    let mut baseline_updates = Vec::new();

    // Cache parsed CDIs per node to avoid re-parsing for each change.
    let mut cdi_cache: HashMap<String, Cdi> = HashMap::new();

    macro_rules! fail {
        ($id:expr, $reason:expr) => {{
            failed.push(ApplySyncFailure { change_id: $id.clone(), reason: $reason });
            continue;
        }};
    }

    for change_id in apply_ids {
        let change = match changes.iter().find(|c| &c.change_id == change_id) {
            Some(c) => c,
            None => fail!(change_id, "Change not found in cache".to_string()),
        };

        // Non-config changes (bowtie metadata) don't require bus writes
        if change.kind != PendingChangeKind::Config {
            applied.push(change_id.clone());
            continue;
        }

        let prepared = match prepare_write(port, change, &mut cdi_cache).await {
            Ok(p) => p,
            Err(reason) => fail!(change_id, reason),
        };

        match write_prepared(port, &prepared).await {
            Ok(()) => {
                applied.push(change_id.clone());
                nodes_written.insert(prepared.node_key.clone());
                baseline_updates.extend(baseline_update_for(change, &prepared));
                leaf_updates.push(commit_update(prepared));
            }
            Err(err_str) => {
                // Error 0x1083 = read-only field (FR-017b)
                if is_read_only_error(&err_str) {
                    read_only_cleared.push(change_id.clone());
                    leaf_updates.push(mark_read_only_update(&prepared));
                } else {
                    failed.push(ApplySyncFailure { change_id: change_id.clone(), reason: err_str });
                }
            }
        }
    }

    ApplyOutcome {
        result: ApplySyncResult { applied, skipped, failed, read_only_cleared },
        nodes_written,
        leaf_updates,
        baseline_updates,
    }
}

/// Counts for the Save result.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWriteCounts {
    /// Rows the board accepted a write for (no readback; verification is slice E4).
    pub written: usize,
    /// Rows that could not be written.
    pub failed: usize,
    /// Rows the board refused as read-only (0x1083); cleared.
    pub read_only: usize,
    /// Rows still waiting after this Save, recorded by it or earlier.
    pub waiting: usize,
}

/// Result of [`write_recorded_changes`], as data for the caller to persist and apply.
#[derive(Debug, Clone, Default)]
pub struct SaveWriteOutcome {
    /// Rows whose status or reason changed and which stay pending; persist them.
    pub updated_rows: Vec<PendingChange>,
    /// Rows to remove: written and read-only cleared.
    pub cleared_ids: HashSet<String>,
    pub leaf_updates: Vec<LeafUpdate>,
    pub baseline_updates: Vec<BaselineUpdate>,
    /// Nodes that received a write (for Update Complete).
    pub nodes_written: HashSet<String>,
    pub counts: SaveWriteCounts,
}

/// Write the Config rows a Save recorded (`recorded_ids`) and report per-row
/// results. There is no pre-read and no readback: a write the board accepts is
/// treated as written (verification is slice E4). All other rows are untouched.
pub async fn write_recorded_changes<P: MemoryPort>(
    port: &P,
    rows: &[PendingChange],
    recorded_ids: &HashSet<String>,
) -> SaveWriteOutcome {
    let mut out = SaveWriteOutcome::default();
    let mut cdi_cache: HashMap<String, Cdi> = HashMap::new();
    // Rows whose status changed but stay pending, by change id.
    let mut updated: HashMap<String, PendingChange> = HashMap::new();

    let keep = |updated: &mut HashMap<String, PendingChange>, row: &PendingChange, reason: String| {
        let mut next = row.clone();
        next.status = PendingChangeStatus::Failed;
        next.error = Some(reason);
        updated.insert(row.change_id.clone(), next);
    };

    for row in rows
        .iter()
        .filter(|r| r.kind == PendingChangeKind::Config && recorded_ids.contains(&r.change_id))
    {
        let prepared = match prepare_write(port, row, &mut cdi_cache).await {
            Ok(prepared) => prepared,
            Err(reason) => {
                keep(&mut updated, row, reason);
                out.counts.failed += 1;
                continue;
            }
        };
        match write_prepared(port, &prepared).await {
            Ok(()) => {
                out.nodes_written.insert(prepared.node_key.clone());
                out.cleared_ids.insert(row.change_id.clone());
                out.baseline_updates.extend(baseline_update_for(row, &prepared));
                out.leaf_updates.push(commit_update(prepared));
                out.counts.written += 1;
            }
            Err(err) if is_read_only_error(&err) => {
                out.cleared_ids.insert(row.change_id.clone());
                out.leaf_updates.push(mark_read_only_update(&prepared));
                out.counts.read_only += 1;
            }
            Err(err) => {
                keep(&mut updated, row, err);
                out.counts.failed += 1;
            }
        }
    }

    out.counts.waiting = rows
        .iter()
        .filter(|r| !out.cleared_ids.contains(&r.change_id))
        .filter(|r| updated.get(&r.change_id).map_or(&r.status, |u| &u.status).is_waiting())
        .count();
    out.updated_rows = rows
        .iter()
        .filter_map(|r| updated.remove(&r.change_id))
        .collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::node_snapshot::{CaptureStatus, CdiReference, SnipSnapshot};
    use lcc_rs::cdi::{DataElement, IntElement, Segment, StringElement};
    use std::cell::RefCell;

    const NODE: &str = "050201020300";

    fn snapshot(node_key: &str) -> NodeSnapshot {
        NodeSnapshot {
            node_key: node_key.to_string(),
            node_id: None,
            profile_stem: None,
            lifecycle: Default::default(),
            captured_at: "2026-01-01T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot::default(),
            cdi_ref: CdiReference {
                cache_key: "k".to_string(),
                version: "1".to_string(),
                fingerprint: "f".to_string(),
            },
            config: Default::default(),
            producer_identified_events: Vec::new(),
        }
    }

    /// Segment "Config" in space 253: Int(2) "Value" at 0x10, String(128) "Name" at 0x20.
    fn cdi() -> Cdi {
        Cdi {
            identification: None,
            acdi: None,
            segments: vec![Segment {
                name: Some("Config".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: vec![
                    DataElement::Int(IntElement {
                        name: Some("Value".to_string()),
                        description: None,
                        size: 2,
                        offset: 16,
                        min: None,
                        max: None,
                        default: None,
                        map: None,
                        hints: None,
                    }),
                    DataElement::String(StringElement {
                        name: Some("Name".to_string()),
                        description: None,
                        size: 128,
                        offset: 14,
                    }),
                ],
            }],
        }
    }

    fn change(id: &str, offset: &str, baseline: &str, planned: &str) -> PendingChange {
        PendingChange {
            change_id: id.to_string(),
            kind: PendingChangeKind::Config,
            node_key: Some(NODE.to_string()),
            space: Some(253),
            offset: Some(offset.to_string()),
            baseline_value: baseline.to_string(),
            planned_value: planned.to_string(),
            status: PendingChangeStatus::Pending,
            error: None,
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            origin: None,
        }
    }

    #[derive(Default)]
    struct FakePort {
        on_bus: bool,
        reads: HashMap<(u8, u32), Result<Vec<u8>, String>>,
        write_results: HashMap<(u8, u32), Result<(), String>>,
        cdi: Option<Cdi>,
        read_calls: RefCell<Vec<(u8, u32, u8, u64)>>,
        write_calls: RefCell<Vec<(u8, u32, Vec<u8>, u64)>>,
    }

    impl FakePort {
        fn on_bus() -> Self {
            Self { on_bus: true, ..Default::default() }
        }
    }

    impl MemoryPort for FakePort {
        fn load_layout_resources(&self, node_key: &str) -> Result<LayoutNodeResources, String> {
            Ok(LayoutNodeResources {
                snapshot: snapshot(node_key),
                cdi: self.cdi.clone().unwrap_or_else(cdi),
            })
        }

        async fn resolve_bus_node(&self, _node_key: &str) -> Result<(), BusNodeError> {
            if self.on_bus { Ok(()) } else { Err(BusNodeError::NotOnBus) }
        }

        async fn peer_session_ready(&self, _node_key: &str) -> Result<(), String> {
            Ok(())
        }

        async fn read_memory(
            &self,
            _node_key: &str,
            space: u8,
            address: u32,
            size: u8,
            timeout_ms: u64,
        ) -> Result<Vec<u8>, String> {
            self.read_calls.borrow_mut().push((space, address, size, timeout_ms));
            self.reads.get(&(space, address)).cloned().unwrap_or_else(|| Err("no read scripted".to_string()))
        }

        async fn write_memory(
            &self,
            _node_key: &str,
            space: u8,
            address: u32,
            data: Vec<u8>,
            timeout_ms: u64,
        ) -> Result<(), String> {
            self.write_calls.borrow_mut().push((space, address, data.clone(), timeout_ms));
            let result = self.write_results.get(&(space, address)).cloned().unwrap_or(Ok(()));
            result
        }
    }

    fn port_with_value(bytes: Vec<u8>) -> FakePort {
        let mut p = FakePort::on_bus();
        p.reads.insert((253, 0x10), Ok(bytes));
        p
    }

    #[tokio::test]
    async fn bus_equal_to_planned_is_already_applied_and_row_is_not_listed() {
        let port = port_with_value(vec![0, 20]);
        let out = classify_session(&port, &[change("r1", "0x00000010", "10", "20")]).await;

        assert_eq!(out.session.already_applied_count, 1);
        assert!(out.already_applied_ids.contains("r1"));
        assert!(out.session.clean_rows.is_empty());
        assert!(out.session.conflict_rows.is_empty());
        assert!(out.session.node_missing_rows.is_empty());
    }

    #[tokio::test]
    async fn already_applied_row_commits_the_leaf_and_moves_the_baseline_like_a_write() {
        let port = port_with_value(vec![0, 20]);
        let out = classify_session(&port, &[change("r1", "0x00000010", "10", "20")]).await;

        assert_eq!(
            out.leaf_updates,
            vec![LeafUpdate::Commit {
                node_key: NODE.to_string(),
                space: 253,
                address: 0x10,
                value: ConfigValue::Int { value: 20 },
            }]
        );
        assert_eq!(
            out.baseline_updates,
            vec![BaselineUpdate {
                node_key: NODE.to_string(),
                space: 253,
                offset: "0x00000010".to_string(),
                planned_value: "20".to_string(),
            }]
        );
    }

    #[tokio::test]
    async fn bus_equal_to_baseline_is_clean_with_bus_value_and_label() {
        let port = port_with_value(vec![0, 10]);
        let out = classify_session(&port, &[change("r1", "0x00000010", "10", "20")]).await;

        assert_eq!(out.session.clean_rows.len(), 1);
        let row = &out.session.clean_rows[0];
        assert_eq!(row.change_id, "r1");
        assert_eq!(row.bus_value.as_deref(), Some("10"));
        assert_eq!(row.field_label.as_deref(), Some("Config.Value"));
        assert!(out.already_applied_ids.is_empty());
    }

    #[tokio::test]
    async fn bus_differing_from_baseline_and_planned_is_conflict() {
        let port = port_with_value(vec![0, 99]);
        let out = classify_session(&port, &[change("r1", "0x00000010", "10", "20")]).await;

        assert_eq!(out.session.conflict_rows.len(), 1);
        assert_eq!(out.session.conflict_rows[0].bus_value.as_deref(), Some("99"));
        assert!(out.session.clean_rows.is_empty());
        assert_eq!(out.session.already_applied_count, 0);
    }

    #[tokio::test]
    async fn read_failure_marks_node_missing_with_reason() {
        let mut port = FakePort::on_bus();
        port.reads.insert((253, 0x10), Err("timed out".to_string()));
        let out = classify_session(&port, &[change("r1", "0x00000010", "10", "20")]).await;

        assert_eq!(out.session.node_missing_rows.len(), 1);
        let row = &out.session.node_missing_rows[0];
        assert_eq!(row.error.as_deref(), Some("Read failed: timed out"));
        assert_eq!(row.bus_value, None);
    }

    #[tokio::test]
    async fn non_config_rows_go_to_clean_without_reading_the_bus() {
        let port = FakePort::on_bus();
        let mut c = change("r1", "0x00000010", "a", "b");
        c.kind = PendingChangeKind::BowtieMetadata;
        let out = classify_session(&port, &[c]).await;

        assert_eq!(out.session.clean_rows.len(), 1);
        assert_eq!(out.session.clean_rows[0].bus_value, None);
        assert!(port.read_calls.borrow().is_empty());
    }

    #[tokio::test]
    async fn rows_not_pending_are_left_out_of_the_session() {
        let port = port_with_value(vec![0, 10]);
        let mut c = change("r1", "0x00000010", "10", "20");
        c.status = PendingChangeStatus::Applied;
        let out = classify_session(&port, &[c]).await;

        assert!(out.session.clean_rows.is_empty());
        assert!(port.read_calls.borrow().is_empty());
    }

    #[tokio::test]
    async fn failed_and_conflict_rows_are_classified_again_in_the_session() {
        let port = port_with_value(vec![0, 99]);
        let mut failed = change("r1", "0x00000010", "10", "20");
        failed.status = PendingChangeStatus::Failed;
        let mut conflict = change("r2", "0x00000010", "10", "20");
        conflict.status = PendingChangeStatus::Conflict;
        let out = classify_session(&port, &[failed, conflict]).await;

        assert_eq!(out.session.conflict_rows.len(), 2);
    }

    #[tokio::test]
    async fn reads_are_capped_at_64_bytes_with_a_3_second_timeout() {
        let mut port = FakePort::on_bus();
        port.reads.insert((253, 0x20), Ok(vec![0; 64]));
        let _ = classify_session(&port, &[change("r1", "0x00000020", "", "x")]).await;

        assert_eq!(port.read_calls.borrow().as_slice(), &[(253, 0x20, 64, 3000)]);
    }

    #[tokio::test]
    async fn node_not_on_bus_is_missing_without_reason_and_unreadable() {
        let port = FakePort::default();
        let out = classify_session(&port, &[change("r1", "0x00000010", "10", "20")]).await;

        assert_eq!(out.session.node_missing_rows.len(), 1);
        assert_eq!(out.session.node_missing_rows[0].error, None);
        assert!(port.read_calls.borrow().is_empty());
    }

    fn apply_ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn skipped_ids_are_only_echoed_and_nothing_is_written() {
        let port = FakePort::on_bus();
        let rows = [change("r1", "0x00000010", "10", "20")];
        let out = apply_changes(&port, &rows, &[], &apply_ids(&["r1"])).await;

        assert_eq!(out.result.skipped, vec!["r1".to_string()]);
        assert!(out.result.applied.is_empty());
        assert!(out.result.failed.is_empty());
        assert!(out.cleared_ids().is_empty());
        assert!(out.leaf_updates.is_empty());
        assert!(port.write_calls.borrow().is_empty());
    }

    #[tokio::test]
    async fn successful_write_clears_row_and_yields_leaf_and_baseline_updates() {
        let port = FakePort::on_bus();
        let rows = [change("r1", "0x00000010", "10", "20")];
        let out = apply_changes(&port, &rows, &apply_ids(&["r1"]), &[]).await;

        assert_eq!(out.result.applied, vec!["r1".to_string()]);
        assert!(out.result.failed.is_empty());
        assert_eq!(out.cleared_ids(), HashSet::from(["r1".to_string()]));
        assert_eq!(port.write_calls.borrow().as_slice(), &[(253, 0x10, vec![0u8, 20], 3000)]);
        assert_eq!(
            out.leaf_updates,
            vec![LeafUpdate::Commit {
                node_key: NODE.to_string(),
                space: 253,
                address: 0x10,
                value: ConfigValue::Int { value: 20 },
            }]
        );
        assert_eq!(
            out.baseline_updates,
            vec![BaselineUpdate {
                node_key: NODE.to_string(),
                space: 253,
                offset: "0x00000010".to_string(),
                planned_value: "20".to_string(),
            }]
        );
        assert_eq!(out.nodes_written, HashSet::from([NODE.to_string()]));
    }

    #[tokio::test]
    async fn error_1083_clears_row_and_marks_field_read_only() {
        let mut port = FakePort::on_bus();
        port.write_results.insert((253, 0x10), Err("Write rejected: error 0x1083".to_string()));
        let rows = [change("r1", "0x00000010", "10", "20")];
        let out = apply_changes(&port, &rows, &apply_ids(&["r1"]), &[]).await;

        assert_eq!(out.result.read_only_cleared, vec!["r1".to_string()]);
        assert!(out.result.applied.is_empty());
        assert!(out.result.failed.is_empty());
        assert_eq!(out.cleared_ids(), HashSet::from(["r1".to_string()]));
        assert_eq!(
            out.leaf_updates,
            vec![LeafUpdate::MarkReadOnly { node_key: NODE.to_string(), space: 253, address: 0x10 }]
        );
        assert!(out.baseline_updates.is_empty());
        assert!(out.nodes_written.is_empty());
    }

    #[tokio::test]
    async fn other_write_error_leaves_row_pending_and_counts_as_failed_with_reason() {
        let mut port = FakePort::on_bus();
        port.write_results.insert((253, 0x10), Err("timeout".to_string()));
        let rows = [change("r1", "0x00000010", "10", "20")];
        let out = apply_changes(&port, &rows, &apply_ids(&["r1"]), &[]).await;

        assert_eq!(out.result.failed.len(), 1);
        assert_eq!(out.result.failed[0].change_id, "r1");
        assert_eq!(out.result.failed[0].reason, "timeout");
        assert!(out.cleared_ids().is_empty());
        assert!(out.leaf_updates.is_empty());
        assert!(out.baseline_updates.is_empty());
    }

    #[tokio::test]
    async fn unknown_change_id_is_a_failure() {
        let port = FakePort::on_bus();
        let out = apply_changes(&port, &[], &apply_ids(&["nope"]), &[]).await;

        assert_eq!(out.result.failed.len(), 1);
        assert_eq!(out.result.failed[0].change_id, "nope");
        assert_eq!(out.result.failed[0].reason, "Change not found in cache");
    }

    #[tokio::test]
    async fn non_config_rows_apply_without_a_bus_write() {
        let port = FakePort::on_bus();
        let mut c = change("r1", "0x00000010", "a", "b");
        c.kind = PendingChangeKind::BowtieMetadata;
        let out = apply_changes(&port, &[c], &apply_ids(&["r1"]), &[]).await;

        assert_eq!(out.result.applied, vec!["r1".to_string()]);
        assert!(port.write_calls.borrow().is_empty());
        assert!(out.baseline_updates.is_empty());
    }

    // ---- write_recorded_changes (Save path) ----

    fn ids(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn one_value_save_does_one_write_and_zero_reads_and_commits() {
        let port = FakePort::on_bus();
        let rows = [change("r1", "0x00000010", "10", "20")];
        let out = write_recorded_changes(&port, &rows, &ids(&["r1"])).await;

        assert_eq!(port.write_calls.borrow().len(), 1);
        assert!(port.read_calls.borrow().is_empty(), "no pre-read and no readback");
        assert_eq!(out.cleared_ids, ids(&["r1"]));
        assert!(out.updated_rows.is_empty());
        assert_eq!(
            out.leaf_updates,
            vec![LeafUpdate::Commit {
                node_key: NODE.to_string(),
                space: 253,
                address: 0x10,
                value: ConfigValue::Int { value: 20 },
            }]
        );
        assert_eq!(out.baseline_updates.len(), 1);
        assert_eq!(out.baseline_updates[0].planned_value, "20");
        assert_eq!(out.nodes_written, ids(&[NODE]));
        assert_eq!(out.counts, SaveWriteCounts { written: 1, ..Default::default() });
    }

    #[tokio::test]
    async fn rows_not_recorded_by_this_save_are_untouched_and_counted_waiting() {
        let port = FakePort::on_bus();
        let rows = [
            change("r1", "0x00000010", "10", "20"),
            change("old", "0x00000020", "a", "b"),
        ];
        let out = write_recorded_changes(&port, &rows, &ids(&["r1"])).await;

        assert_eq!(port.write_calls.borrow().len(), 1);
        assert_eq!(out.cleared_ids, ids(&["r1"]));
        assert!(out.updated_rows.iter().all(|r| r.change_id != "old"));
        assert_eq!(out.counts.waiting, 1);
    }

    #[tokio::test]
    async fn error_1083_clears_the_row_and_marks_the_field_read_only() {
        let mut port = FakePort::on_bus();
        port.write_results.insert((253, 0x10), Err("Write rejected: error 0x1083".to_string()));
        let rows = [change("r1", "0x00000010", "10", "20")];
        let out = write_recorded_changes(&port, &rows, &ids(&["r1"])).await;

        assert_eq!(out.cleared_ids, ids(&["r1"]));
        assert_eq!(
            out.leaf_updates,
            vec![LeafUpdate::MarkReadOnly { node_key: NODE.to_string(), space: 253, address: 0x10 }]
        );
        assert!(port.read_calls.borrow().is_empty());
        assert_eq!(out.counts.read_only, 1);
    }

    #[tokio::test]
    async fn other_write_error_keeps_the_row_failed_with_the_reason() {
        let mut port = FakePort::on_bus();
        port.write_results.insert((253, 0x10), Err("timeout".to_string()));
        let rows = [change("r1", "0x00000010", "10", "20")];
        let out = write_recorded_changes(&port, &rows, &ids(&["r1"])).await;

        assert!(out.cleared_ids.is_empty());
        assert_eq!(out.updated_rows[0].status, PendingChangeStatus::Failed);
        assert_eq!(out.updated_rows[0].error.as_deref(), Some("timeout"));
        assert!(out.leaf_updates.is_empty());
        assert!(out.baseline_updates.is_empty());
        assert_eq!(out.counts.failed, 1);
        assert_eq!(out.counts.waiting, 1);
    }

    fn many_int_cdi(n: usize) -> Cdi {
        let element = |i: usize| {
            DataElement::Int(IntElement {
                name: Some(format!("F{i}")),
                description: None,
                size: 2,
                offset: 0,
                min: None,
                max: None,
                default: None,
                map: None,
                hints: None,
            })
        };
        Cdi {
            identification: None,
            acdi: None,
            segments: vec![Segment {
                name: Some("Config".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: (0..n).map(element).collect(),
            }],
        }
    }

    #[tokio::test]
    async fn fifty_fields_are_fifty_writes_and_zero_reads() {
        let mut port = FakePort::on_bus();
        port.cdi = Some(many_int_cdi(50));
        let rows: Vec<PendingChange> = (0..50)
            .map(|i| change(&format!("r{i}"), &format!("0x{:08x}", i * 2), "0", "7"))
            .collect();
        let recorded: HashSet<String> = rows.iter().map(|r| r.change_id.clone()).collect();
        let out = write_recorded_changes(&port, &rows, &recorded).await;

        assert_eq!(port.write_calls.borrow().len(), 50);
        assert!(port.read_calls.borrow().is_empty());
        assert_eq!(out.cleared_ids.len(), 50);
        assert_eq!(out.counts.written, 50);
        assert_eq!(out.counts.waiting, 0);
    }
}

#[cfg(test)]
mod changed_nodes_tests {
    use super::*;

    #[test]
    fn nodes_whose_trees_changed_are_listed_once_each() {
        let commit = |node: &str, address: u32| LeafUpdate::Commit {
            node_key: node.to_string(),
            space: 253,
            address,
            value: ConfigValue::Int { value: 1 },
        };
        let updates = vec![
            commit("050201020300", 0x10),
            LeafUpdate::MarkReadOnly { node_key: "050201020301".to_string(), space: 253, address: 0x20 },
            commit("050201020300", 0x12),
        ];

        assert_eq!(
            changed_node_keys(&updates),
            vec!["050201020300".to_string(), "050201020301".to_string()]
        );
    }
}
