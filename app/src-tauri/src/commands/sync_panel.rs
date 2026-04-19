//! Offline sync panel commands.
//!
//! Implements layout-match scoring, sync session building, and selective apply.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tauri::Manager;
use crate::layout::offline_changes::{OfflineChange, OfflineChangeKind, OfflineChangeStatus};
use crate::node_tree::{ConfigValue, LeafNode, LeafType};
use crate::state::{AppState, ActiveLayoutMode, SyncMode};

fn same_change_target(a: &OfflineChange, b: &OfflineChange) -> bool {
    a.kind == b.kind
        && a.node_id == b.node_id
        && a.space == b.space
        && a.offset == b.offset
        && a.baseline_value == b.baseline_value
}

/// Parse a hex offset string like "0x00000120" into a u32 address.
fn parse_offset(offset: &str) -> Option<u32> {
    let trimmed = offset.strip_prefix("0x").or_else(|| offset.strip_prefix("0X")).unwrap_or(offset);
    u32::from_str_radix(trimmed, 16).ok()
}

/// Parse a string value back into a `ConfigValue` using the leaf's type/size metadata.
fn string_to_config_value(s: &str, leaf: &LeafNode) -> Option<ConfigValue> {
    use crate::node_tree::LeafType;
    match leaf.element_type {
        LeafType::Int => {
            let v: i64 = s.parse().ok()?;
            Some(ConfigValue::Int { value: v })
        }
        LeafType::String => Some(ConfigValue::String { value: s.to_string() }),
        LeafType::Float => {
            let v: f64 = s.parse().ok()?;
            Some(ConfigValue::Float { value: v })
        }
        LeafType::EventId => {
            // Parse dotted-hex format: "01.02.03.04.05.06.07.08"
            let parts: Vec<&str> = s.split('.').collect();
            if parts.len() != 8 {
                return None;
            }
            let mut bytes = [0u8; 8];
            for (i, part) in parts.iter().enumerate() {
                bytes[i] = u8::from_str_radix(part, 16).ok()?;
            }
            let hex = s.to_string();
            Some(ConfigValue::EventId { bytes, hex })
        }
        _ => None,
    }
}

// ── CDI-based field metadata for targeted reads ─────────────────────────────

/// Metadata for a single CDI leaf element, resolved by walking the CDI tree.
#[derive(Debug, Clone)]
struct FieldMeta {
    leaf_type: LeafType,
    size: u32,
}

/// Resolve CDI XML for a node from the layout companion directory.
///
/// Loads the node snapshot YAML to get the CDI reference, then reads the
/// cached CDI file.  Returns the parsed CDI or an error.
fn resolve_layout_cdi(
    canonical_node_id: &str,
    root_path: &str,
    app: &tauri::AppHandle,
) -> Result<lcc_rs::cdi::Cdi, String> {
    let base_file = std::path::Path::new(root_path);
    let companion_dir = crate::layout::io::derive_companion_dir_path(base_file)?;
    let snapshot_path = companion_dir
        .join("nodes")
        .join(format!("{}.yaml", canonical_node_id.to_uppercase()));

    let snapshot: crate::layout::node_snapshot::NodeSnapshot =
        crate::layout::io::read_yaml_file(&snapshot_path)
            .map_err(|e| format!("Cannot load snapshot {}: {}", snapshot_path.display(), e))?;

    // Build the CDI cache path from snapshot SNIP metadata
    let sanitize = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect()
    };
    let cdi_filename = format!(
        "{}_{}_{}.cdi.xml",
        sanitize(&snapshot.snip.manufacturer_name),
        sanitize(&snapshot.snip.model_name),
        sanitize(&snapshot.cdi_ref.version),
    );

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {}", e))?;
    let cdi_path = app_data_dir.join("cdi_cache").join(&cdi_filename);

    // Try app cache first, then layout companion cdi/ dir
    let xml = if cdi_path.exists() {
        std::fs::read_to_string(&cdi_path)
            .map_err(|e| format!("Cannot read CDI {}: {}", cdi_path.display(), e))?
    } else {
        let layout_cdi = companion_dir.join("cdi").join(&cdi_filename);
        if layout_cdi.exists() {
            std::fs::read_to_string(&layout_cdi)
                .map_err(|e| format!("Cannot read layout CDI {}: {}", layout_cdi.display(), e))?
        } else {
            return Err(format!(
                "CDI not found for node {} (tried {} and {})",
                canonical_node_id,
                cdi_path.display(),
                companion_dir.join("cdi").join(&cdi_filename).display()
            ));
        }
    };

    lcc_rs::cdi::parser::parse_cdi(&xml)
        .map_err(|e| format!("Cannot parse CDI for {}: {}", canonical_node_id, e))
}

/// Walk parsed CDI elements recursively to find a leaf at the given absolute
/// address within the given space.  Returns the leaf's type and size.
fn find_field_meta_in_cdi(cdi: &lcc_rs::cdi::Cdi, space: u8, address: u32) -> Option<FieldMeta> {
    for segment in &cdi.segments {
        if segment.space != space {
            continue;
        }
        if let Some(meta) = walk_elements_for_meta(&segment.elements, segment.origin as i32, 0, space, address) {
            return Some(meta);
        }
    }
    None
}

/// Recursively walk CDI elements using cursor-based addressing to locate a
/// leaf at `target_address` in `target_space`.
fn walk_elements_for_meta(
    elements: &[lcc_rs::cdi::DataElement],
    segment_origin: i32,
    base_offset: i32,
    target_space: u8,
    target_address: u32,
) -> Option<FieldMeta> {
    use lcc_rs::cdi::DataElement;

    let mut cursor: i32 = 0;

    for element in elements {
        match element {
            DataElement::Group(g) => {
                cursor += g.offset;
                let group_start = base_offset + cursor;
                let stride = g.calculate_size();
                let effective_replication = if stride == 0 && g.replication > 1 { 1u32 } else { g.replication };

                for instance in 0..effective_replication {
                    let instance_base = group_start + instance as i32 * stride;
                    if let Some(meta) = walk_elements_for_meta(
                        &g.elements, segment_origin, instance_base, target_space, target_address
                    ) {
                        return Some(meta);
                    }
                }
                cursor += effective_replication as i32 * stride;
            }
            DataElement::Int(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta { leaf_type: LeafType::Int, size: e.size as u32 });
                }
                cursor += e.size as i32;
            }
            DataElement::String(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta { leaf_type: LeafType::String, size: e.size as u32 });
                }
                cursor += e.size as i32;
            }
            DataElement::EventId(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta { leaf_type: LeafType::EventId, size: 8 });
                }
                cursor += 8;
            }
            DataElement::Float(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta { leaf_type: LeafType::Float, size: e.size as u32 });
                }
                cursor += e.size as i32;
            }
            DataElement::Action(e) => { cursor += e.offset + 1; }
            DataElement::Blob(e) => { cursor += e.offset + e.size as i32; }
        }
    }
    None
}

/// Parse raw bytes from a memory read into a string value matching the format
/// used by `config_value_to_string`.
fn raw_bytes_to_value_string(meta: &FieldMeta, raw: &[u8]) -> Option<String> {
    use crate::node_tree::LeafType as LT;
    match meta.leaf_type {
        LT::Int => {
            let val = match meta.size {
                1 => raw.first().map(|&b| b as i64),
                2 if raw.len() >= 2 => Some(i16::from_be_bytes([raw[0], raw[1]]) as i64),
                4 if raw.len() >= 4 => Some(i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as i64),
                8 if raw.len() >= 8 => Some(i64::from_be_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ])),
                _ => None,
            };
            val.map(|v| v.to_string())
        }
        LT::String => {
            let s: String = raw
                .iter()
                .take(meta.size as usize)
                .take_while(|&&b| b != 0)
                .filter(|&&b| b != 0xFF)
                .map(|&b| b as char)
                .collect();
            Some(s)
        }
        LT::EventId => {
            if raw.len() >= 8 {
                let hex = raw[..8]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(".");
                Some(hex)
            } else {
                None
            }
        }
        LT::Float => {
            if meta.size == 4 && raw.len() >= 4 {
                let val = f32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
                Some((val as f64).to_string())
            } else if meta.size == 8 && raw.len() >= 8 {
                let val = f64::from_be_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ]);
                Some(val.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Build a synthetic `LeafNode` from CDI field metadata, used by
/// `string_to_config_value` and `serialize_config_value` in `apply_sync_changes`.
fn field_meta_to_leaf(meta: &FieldMeta, space: u8, address: u32) -> LeafNode {
    LeafNode {
        name: String::new(),
        description: None,
        element_type: meta.leaf_type,
        address,
        size: meta.size,
        space,
        path: Vec::new(),
        value: None,
        event_role: None,
        constraints: None,
        button_text: None,
        dialog_text: None,
        action_value: 0,
        hint_slider: None,
        hint_radio: false,
        modified_value: None,
        write_state: None,
        write_error: None,
        read_only: false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineChangeInput {
    pub kind: String,
    pub node_id: Option<String>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineChangeRow {
    pub change_id: String,
    pub kind: String,
    pub node_id: Option<String>,
    pub space: Option<u8>,
    pub offset: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub status: String,
    pub error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutMatchThresholds {
    pub likely_same_min: u8,
    pub uncertain_min: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutMatchStatus {
    pub overlap_percent: f64,
    pub classification: String,
    pub expected_thresholds: LayoutMatchThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRow {
    pub change_id: String,
    pub node_id: Option<String>,
    pub baseline_value: String,
    pub planned_value: String,
    pub bus_value: Option<String>,
    pub resolution: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSession {
    pub conflict_rows: Vec<SyncRow>,
    pub clean_rows: Vec<SyncRow>,
    pub already_applied_count: usize,
    pub node_missing_rows: Vec<SyncRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySyncFailure {
    pub change_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySyncResult {
    pub applied: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<ApplySyncFailure>,
    pub read_only_cleared: Vec<String>,
}

#[tauri::command]
pub async fn set_offline_change(
    change: OfflineChangeInput,
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

    // Create the offline change record
    let offline_change = OfflineChange {
        change_id: change_id.clone(),
        kind: match change.kind.as_str() {
            "config" => OfflineChangeKind::Config,
            "bowtieMetadata" => OfflineChangeKind::BowtieMetadata,
            "bowtieEvent" => OfflineChangeKind::BowtieEvent,
            _ => return Err(format!("Invalid change kind: {}", change.kind)),
        },
        node_id: change.node_id,
        space: change.space,
        offset: change.offset,
        baseline_value: change.baseline_value,
        planned_value: change.planned_value,
        status: OfflineChangeStatus::Pending,
        error: None,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    // Validate the change
    offline_change.validate()?;

    // In-memory only pre-save path: upsert on same target.
    let mut change_id_out = change_id;
    {
        let mut cache = state.offline_changes_cache.write().await;
        if let Some(existing) = cache.iter_mut().find(|c| same_change_target(c, &offline_change)) {
            existing.planned_value = offline_change.planned_value;
            existing.updated_at = offline_change.updated_at;
            existing.status = OfflineChangeStatus::Pending;
            existing.error = None;
            change_id_out = existing.change_id.clone();
        } else {
            cache.push(offline_change);
        }
    }

    // Update pending count in active layout context
    {
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_offline_change_count = state.offline_changes_cache.read().await.len();
        }
    }

    Ok(change_id_out)
}

#[tauri::command]
pub async fn revert_offline_change(
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
    let removed = {
        let mut cache = state.offline_changes_cache.write().await;
        let initial_len = cache.len();
        cache.retain(|c| c.change_id != change_id);
        cache.len() < initial_len
    };

    if !removed {
        return Err(format!("Change not found: {}", change_id));
    }

    // Update pending count in active layout context
    {
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_offline_change_count = state.offline_changes_cache.read().await.len();
        }
    }

    Ok(true)
}

#[tauri::command]
pub async fn list_offline_changes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<OfflineChangeRow>, String> {
    // Verify active offline layout
    let _context = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?
    };

    let changes = state.offline_changes_cache.read().await;
    let rows = changes
        .iter()
        .map(|c| OfflineChangeRow {
            change_id: c.change_id.clone(),
            kind: format!("{:?}", c.kind).to_lowercase(),
            node_id: c.node_id.clone(),
            space: c.space,
            offset: c.offset.clone(),
            baseline_value: c.baseline_value.clone(),
            planned_value: c.planned_value.clone(),
            status: format!("{:?}", c.status).to_lowercase(),
            error: c.error.clone(),
            updated_at: c.updated_at.clone(),
        })
        .collect();

    Ok(rows)
}

#[tauri::command]
pub async fn replace_offline_changes(
    changes: Vec<OfflineChangeInput>,
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
        let kind = match change.kind.as_str() {
            "config" => OfflineChangeKind::Config,
            "bowtieMetadata" => OfflineChangeKind::BowtieMetadata,
            "bowtieEvent" => OfflineChangeKind::BowtieEvent,
            _ => return Err(format!("Invalid change kind: {}", change.kind)),
        };

        let row = OfflineChange {
            change_id: format!("{}-{}", Uuid::new_v4(), chrono::Utc::now().timestamp_millis()),
            kind,
            node_id: change.node_id,
            space: change.space,
            offset: change.offset,
            baseline_value: change.baseline_value,
            planned_value: change.planned_value,
            status: OfflineChangeStatus::Pending,
            error: None,
            updated_at: now.clone(),
        };
        row.validate()?;
        rows.push(row);
    }

    let count = rows.len();
    *state.offline_changes_cache.write().await = rows;

    let mut guard = state.active_layout.write().await;
    if let Some(ctx) = &mut *guard {
        ctx.pending_offline_change_count = count;
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
        .layout_node_ids
        .iter()
        .map(|id| id.replace('.', "").to_uppercase())
        .collect();

    if layout_ids.is_empty() {
        return Ok(LayoutMatchStatus {
            overlap_percent: 0.0,
            classification: "likely_different".to_string(),
            expected_thresholds: LayoutMatchThresholds {
                likely_same_min: 80,
                uncertain_min: 40,
            },
        });
    }

    let discovered_ids: std::collections::HashSet<String> = discovered_node_ids
        .iter()
        .map(|id| id.replace('.', "").to_uppercase())
        .collect();

    let matched = layout_ids.intersection(&discovered_ids).count();
    let overlap_percent = (matched as f64 / layout_ids.len() as f64) * 100.0;

    let classification = if overlap_percent >= 80.0 {
        "likely_same"
    } else if overlap_percent >= 40.0 {
        "uncertain"
    } else {
        "likely_different"
    };

    Ok(LayoutMatchStatus {
        overlap_percent,
        classification: classification.to_string(),
        expected_thresholds: LayoutMatchThresholds {
            likely_same_min: 80,
            uncertain_min: 40,
        },
    })
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

    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);

    let changes = state.offline_changes_cache.read().await.clone();
    let pending: Vec<&OfflineChange> = changes
        .iter()
        .filter(|c| c.status == OfflineChangeStatus::Pending)
        .collect();

    let mut conflict_rows = Vec::new();
    let mut clean_rows = Vec::new();
    let mut already_applied_count: usize = 0;
    let mut node_missing_rows = Vec::new();

    // Cache parsed CDIs per node to avoid re-parsing for each change
    let mut cdi_cache: std::collections::HashMap<String, lcc_rs::cdi::Cdi> = std::collections::HashMap::new();

    for change in pending {
        // Only config changes go through bus comparison; bowtie metadata
        // changes are always clean (they don't live on the bus).
        if change.kind != OfflineChangeKind::Config {
            clean_rows.push(SyncRow {
                change_id: change.change_id.clone(),
                node_id: change.node_id.clone(),
                baseline_value: change.baseline_value.clone(),
                planned_value: change.planned_value.clone(),
                bus_value: None,
                resolution: "unresolved".to_string(),
                error: None,
            });
            continue;
        }

        let node_id_str = match &change.node_id {
            Some(id) => id.clone(),
            None => {
                node_missing_rows.push(SyncRow {
                    change_id: change.change_id.clone(),
                    node_id: None,
                    baseline_value: change.baseline_value.clone(),
                    planned_value: change.planned_value.clone(),
                    bus_value: None,
                    resolution: "unresolved".to_string(),
                    error: Some("No node ID".to_string()),
                });
                continue;
            }
        };

        // Try to find this node on the bus via the node registry
        let parsed_node_id = match lcc_rs::NodeID::from_hex_string(&node_id_str) {
            Ok(id) => id,
            Err(_) => {
                node_missing_rows.push(SyncRow {
                    change_id: change.change_id.clone(),
                    node_id: change.node_id.clone(),
                    baseline_value: change.baseline_value.clone(),
                    planned_value: change.planned_value.clone(),
                    bus_value: None,
                    resolution: "unresolved".to_string(),
                    error: Some(format!("Invalid node ID: {}", node_id_str)),
                });
                continue;
            }
        };

        let proxy = match state.node_registry.get(&parsed_node_id).await {
            Some(p) => p,
            None => {
                node_missing_rows.push(SyncRow {
                    change_id: change.change_id.clone(),
                    node_id: change.node_id.clone(),
                    baseline_value: change.baseline_value.clone(),
                    planned_value: change.planned_value.clone(),
                    bus_value: None,
                    resolution: "unresolved".to_string(),
                    error: None,
                });
                continue;
            }
        };
        let alias = proxy.alias;

        let space = change.space.unwrap_or(0);
        let address = change
            .offset
            .as_deref()
            .and_then(parse_offset)
            .unwrap_or(0);

        // Resolve CDI from layout for this node (cached per node)
        let canonical = node_id_str.replace('.', "").to_uppercase();
        let cdi = if let Some(c) = cdi_cache.get(&canonical) {
            c
        } else {
            match resolve_layout_cdi(&canonical, &context.root_path, &app) {
                Ok(c) => {
                    cdi_cache.insert(canonical.clone(), c);
                    cdi_cache.get(&canonical).unwrap()
                }
                Err(e) => {
                    node_missing_rows.push(SyncRow {
                        change_id: change.change_id.clone(),
                        node_id: change.node_id.clone(),
                        baseline_value: change.baseline_value.clone(),
                        planned_value: change.planned_value.clone(),
                        bus_value: None,
                        resolution: "unresolved".to_string(),
                        error: Some(format!("CDI unavailable: {}", e)),
                    });
                    continue;
                }
            }
        };

        // Find the field metadata (type, size) from CDI
        let meta = match find_field_meta_in_cdi(cdi, space, address) {
            Some(m) => m,
            None => {
                node_missing_rows.push(SyncRow {
                    change_id: change.change_id.clone(),
                    node_id: change.node_id.clone(),
                    baseline_value: change.baseline_value.clone(),
                    planned_value: change.planned_value.clone(),
                    bus_value: None,
                    resolution: "unresolved".to_string(),
                    error: Some(format!(
                        "Field not found in CDI at space={} address={:#010x}",
                        space, address
                    )),
                });
                continue;
            }
        };

        // Targeted read: fetch only this field from the live bus
        let read_size = meta.size.min(64) as u8;
        let raw = {
            let mut conn = connection.lock().await;
            conn.read_memory(alias, space, address, read_size, 3000).await
        };

        let raw = match raw {
            Ok(data) => data,
            Err(e) => {
                node_missing_rows.push(SyncRow {
                    change_id: change.change_id.clone(),
                    node_id: change.node_id.clone(),
                    baseline_value: change.baseline_value.clone(),
                    planned_value: change.planned_value.clone(),
                    bus_value: None,
                    resolution: "unresolved".to_string(),
                    error: Some(format!("Read failed: {}", e)),
                });
                continue;
            }
        };

        let bus_value = match raw_bytes_to_value_string(&meta, &raw) {
            Some(v) => v,
            None => {
                node_missing_rows.push(SyncRow {
                    change_id: change.change_id.clone(),
                    node_id: change.node_id.clone(),
                    baseline_value: change.baseline_value.clone(),
                    planned_value: change.planned_value.clone(),
                    bus_value: None,
                    resolution: "unresolved".to_string(),
                    error: Some("Cannot parse bus value".to_string()),
                });
                continue;
            }
        };

        // Classify the row
        if bus_value == change.planned_value {
            already_applied_count += 1;
        } else if bus_value == change.baseline_value {
            clean_rows.push(SyncRow {
                change_id: change.change_id.clone(),
                node_id: change.node_id.clone(),
                baseline_value: change.baseline_value.clone(),
                planned_value: change.planned_value.clone(),
                bus_value: Some(bus_value),
                resolution: "unresolved".to_string(),
                error: None,
            });
        } else {
            conflict_rows.push(SyncRow {
                change_id: change.change_id.clone(),
                node_id: change.node_id.clone(),
                baseline_value: change.baseline_value.clone(),
                planned_value: change.planned_value.clone(),
                bus_value: Some(bus_value),
                resolution: "unresolved".to_string(),
                error: None,
            });
        }
    }

    Ok(SyncSession {
        conflict_rows,
        clean_rows,
        already_applied_count,
        node_missing_rows,
    })
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
    let conn_lock = state.connection.read().await;
    let connection = conn_lock
        .as_ref()
        .ok_or("Not connected to network")?
        .clone();
    drop(conn_lock);

    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    let mut read_only_cleared = Vec::new();

    // Mark explicitly skipped rows
    for id in &skip_change_ids {
        skipped.push(id.clone());
    }

    // Process apply rows
    let changes = state.offline_changes_cache.read().await.clone();

    // Cache parsed CDIs per node to avoid re-parsing for each change
    let mut cdi_cache: std::collections::HashMap<String, lcc_rs::cdi::Cdi> = std::collections::HashMap::new();

    // Track which nodes had successful writes for Update Complete
    let mut write_success_nodes = std::collections::HashSet::new();

    // Track applied config changes for snapshot baseline updates: (canonical_node_id, space, offset_hex, planned_value)
    let mut applied_config_details: Vec<(String, u8, String, String)> = Vec::new();

    for change_id in &apply_change_ids {
        let change = match changes.iter().find(|c| &c.change_id == change_id) {
            Some(c) => c,
            None => {
                failed.push(ApplySyncFailure {
                    change_id: change_id.clone(),
                    reason: "Change not found in cache".to_string(),
                });
                continue;
            }
        };

        // Non-config changes (bowtie metadata) don't require bus writes
        if change.kind != OfflineChangeKind::Config {
            applied.push(change_id.clone());
            continue;
        }

        let node_id_str = match &change.node_id {
            Some(id) => id.clone(),
            None => {
                failed.push(ApplySyncFailure {
                    change_id: change_id.clone(),
                    reason: "No node ID on change".to_string(),
                });
                continue;
            }
        };

        let parsed_node_id = match lcc_rs::NodeID::from_hex_string(&node_id_str) {
            Ok(id) => id,
            Err(e) => {
                failed.push(ApplySyncFailure {
                    change_id: change_id.clone(),
                    reason: format!("Invalid node ID: {}", e),
                });
                continue;
            }
        };

        let proxy = match state.node_registry.get(&parsed_node_id).await {
            Some(p) => p,
            None => {
                failed.push(ApplySyncFailure {
                    change_id: change_id.clone(),
                    reason: format!("Node not found on bus: {}", node_id_str),
                });
                continue;
            }
        };
        let alias = proxy.alias;

        let space = change.space.unwrap_or(0);
        let address = change
            .offset
            .as_deref()
            .and_then(parse_offset)
            .unwrap_or(0);

        // Resolve field metadata from layout CDI
        let canonical = node_id_str.replace('.', "").to_uppercase();
        let cdi = if let Some(c) = cdi_cache.get(&canonical) {
            c
        } else {
            match resolve_layout_cdi(&canonical, &context.root_path, &app) {
                Ok(c) => {
                    cdi_cache.insert(canonical.clone(), c);
                    cdi_cache.get(&canonical).unwrap()
                }
                Err(e) => {
                    failed.push(ApplySyncFailure {
                        change_id: change_id.clone(),
                        reason: format!("CDI unavailable: {}", e),
                    });
                    continue;
                }
            }
        };

        let meta = match find_field_meta_in_cdi(cdi, space, address) {
            Some(m) => m,
            None => {
                failed.push(ApplySyncFailure {
                    change_id: change_id.clone(),
                    reason: format!("Field not found in CDI at space={} address={:#010x}", space, address),
                });
                continue;
            }
        };

        // Build a synthetic leaf for value conversion
        let leaf = field_meta_to_leaf(&meta, space, address);

        // Convert the planned value string to a ConfigValue, then serialize to bytes
        let config_val = match string_to_config_value(&change.planned_value, &leaf) {
            Some(v) => v,
            None => {
                failed.push(ApplySyncFailure {
                    change_id: change_id.clone(),
                    reason: format!("Cannot convert planned value '{}' to bytes for leaf type {:?}",
                        change.planned_value, meta.leaf_type),
                });
                continue;
            }
        };

        let bytes = crate::commands::cdi::serialize_config_value(&config_val, meta.leaf_type, meta.size);

        // Write to the bus node
        let mut conn = connection.lock().await;
        let result = conn.write_memory(alias, space, address, &bytes).await;
        drop(conn);

        match result {
            Ok(()) => {
                applied.push(change_id.clone());
                write_success_nodes.insert((node_id_str.clone(), parsed_node_id));

                // Record details for snapshot baseline update
                if let Some(offset_hex) = &change.offset {
                    applied_config_details.push((
                        canonical.clone(),
                        space,
                        offset_hex.clone(),
                        change.planned_value.clone(),
                    ));
                }

                // Commit the value in the tree if it's been populated
                if let Ok(Some(mut tree)) = proxy.get_config_tree().await {
                    crate::node_tree::update_leaf_value(&mut tree, space, address, config_val);
                    let _ = proxy.set_config_tree(tree).await;
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                // Error 0x1083 = read-only field (FR-017b)
                if err_str.contains("1083") {
                    read_only_cleared.push(change_id.clone());
                    // Mark leaf as read-only in tree if available
                    if let Ok(Some(mut tree)) = proxy.get_config_tree().await {
                        crate::node_tree::revert_and_mark_leaf_read_only(&mut tree, space, address);
                        let _ = proxy.set_config_tree(tree).await;
                    }
                } else {
                    failed.push(ApplySyncFailure {
                        change_id: change_id.clone(),
                        reason: err_str,
                    });
                }
            }
        }
    }

    // Send Update Complete to each node that had successful writes
    for (node_id_str, parsed_nid) in &write_success_nodes {
        if let Some(proxy) = state.node_registry.get(parsed_nid).await {
            let mut conn = connection.lock().await;
            if let Err(e) = conn.send_update_complete(proxy.alias).await {
                eprintln!("[sync] Update Complete failed for {}: {}", node_id_str, e);
            }
        }
    }

    // Remove applied and read-only-cleared rows from offline changes cache
    {
        let all_cleared: std::collections::HashSet<&String> = applied.iter()
            .chain(read_only_cleared.iter())
            .collect();

        let mut cache = state.offline_changes_cache.write().await;
        cache.retain(|c| !all_cleared.contains(&c.change_id));

        // Update pending count
        let mut guard = state.active_layout.write().await;
        if let Some(ctx) = &mut *guard {
            ctx.pending_offline_change_count = cache.len();
        }
    }

    // Persist offline-changes.yaml and update node snapshot baselines on disk
    if !applied.is_empty() || !read_only_cleared.is_empty() {
        if let Ok(companion_dir) = crate::layout::io::derive_companion_dir_path(
            std::path::Path::new(&context.root_path),
        ) {
            // Write updated offline-changes.yaml
            let cache_snapshot = state.offline_changes_cache.read().await.clone();
            let changes_path = companion_dir.join("offline-changes.yaml");
            if let Err(e) = crate::layout::io::write_yaml_file(&changes_path, &cache_snapshot) {
                eprintln!("[sync] Failed to persist offline-changes.yaml: {}", e);
            }

            // Update node snapshot baselines for successfully applied config changes
            let nodes_dir = companion_dir.join("nodes");
            let mut updated_snapshots: std::collections::HashMap<String, crate::layout::node_snapshot::NodeSnapshot> =
                std::collections::HashMap::new();

            for (canonical_nid, space, offset_hex, planned_value) in &applied_config_details {
                let snapshot = updated_snapshots.entry(canonical_nid.clone()).or_insert_with(|| {
                    let node_path = crate::layout::io::derive_node_file_path(&nodes_dir, canonical_nid);
                    match crate::layout::io::read_yaml_file::<crate::layout::node_snapshot::NodeSnapshot>(&node_path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[sync] Failed to read snapshot for {}: {}", canonical_nid, e);
                            // Return a dummy that won't be saved (we check below)
                            crate::layout::node_snapshot::NodeSnapshot {
                                node_id: String::new(),
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

                crate::layout::node_snapshot::update_snapshot_baseline(
                    &mut snapshot.config,
                    *space,
                    offset_hex,
                    planned_value,
                );
            }

            // Write back modified snapshots
            for (canonical_nid, snapshot) in &updated_snapshots {
                if snapshot.node_id.is_empty() {
                    continue; // Skip dummies from failed reads
                }
                let node_path = crate::layout::io::derive_node_file_path(&nodes_dir, canonical_nid);
                if let Err(e) = crate::layout::io::write_yaml_file(&node_path, snapshot) {
                    eprintln!("[sync] Failed to persist snapshot for {}: {}", canonical_nid, e);
                }
            }
        }
    }

    Ok(ApplySyncResult {
        applied,
        skipped,
        failed,
        read_only_cleared,
    })
}
