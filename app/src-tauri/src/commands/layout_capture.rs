//! Offline layout capture/open commands.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::Emitter;

use crate::layout::manifest::LayoutManifest;
use crate::layout::node_snapshot::{capture_status_from_missing, missing_detail, CaptureStatus, CdiReference, NodeSnapshot, SnipSnapshot};
use crate::layout::offline_changes::OfflineChange;
use crate::state::{ActiveLayoutContext, ActiveLayoutMode, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSummary {
    pub captured_at: String,
    pub node_count: usize,
    pub complete_count: usize,
    pub partial_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLayoutResult {
    pub manifest_path: String,
    pub node_files_written: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenLayoutResult {
    pub layout_id: String,
    pub captured_at: String,
    pub offline_mode: bool,
    pub node_count: usize,
    pub partial_nodes: Vec<String>,
    pub pending_offline_change_count: usize,
    pub node_snapshots: Vec<NodeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseLayoutResult {
    pub closed: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseLayoutDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewLayoutResult {
    pub layout_id: String,
    pub created_at: String,
}

fn canonical_node_id(node_id_dotted_hex: &str) -> String {
    node_id_dotted_hex.replace('.', "").to_uppercase()
}

fn config_value_to_string(value: &crate::node_tree::ConfigValue) -> String {
    match value {
        crate::node_tree::ConfigValue::Int { value } => value.to_string(),
        crate::node_tree::ConfigValue::String { value } => value.clone(),
        crate::node_tree::ConfigValue::EventId { hex, .. } => hex.clone(),
        crate::node_tree::ConfigValue::Float { value } => value.to_string(),
    }
}

fn collect_leaf_values(
    nodes: &[crate::node_tree::ConfigNode],
    values: &mut BTreeMap<String, BTreeMap<String, String>>,
    missing: &mut Vec<String>,
) {
    for node in nodes {
        match node {
            crate::node_tree::ConfigNode::Leaf(leaf) => {
                let space_key = leaf.space.to_string();
                let offset_key = format!("0x{:08X}", leaf.address);
                if let Some(v) = &leaf.value {
                    values
                        .entry(space_key)
                        .or_default()
                        .insert(offset_key, config_value_to_string(v));
                } else {
                    missing.push(missing_detail(leaf.space, &offset_key, &leaf.path));
                }
            }
            crate::node_tree::ConfigNode::Group(group) => {
                collect_leaf_values(&group.children, values, missing);
            }
        }
    }
}

async fn build_node_snapshot(
    handle: &crate::node_proxy::NodeProxyHandle,
    captured_at: &str,
    producer_events: Vec<String>,
) -> Result<NodeSnapshot, String> {
    let snapshot = handle.get_snapshot().await?;
    let tree = handle.get_config_tree().await?;

    let mut values: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut missing: Vec<String> = Vec::new();

    if let Some(tree) = tree {
        for segment in &tree.segments {
            collect_leaf_values(&segment.children, &mut values, &mut missing);
        }
    } else {
        missing.push("configuration tree not available".to_string());
    }

    let capture_status = capture_status_from_missing(&missing);

    let cdi_fingerprint = snapshot
        .cdi
        .as_ref()
        .map(|c| format!("len:{}", c.xml_content.len()))
        .unwrap_or_else(|| "missing".to_string());

    let (cache_key, version) = if let Some(snip) = &snapshot.snip_data {
        (
            format!(
                "{}_{}_{}",
                snip.manufacturer.replace(' ', "_"),
                snip.model.replace(' ', "_"),
                snip.software_version.replace(' ', "_")
            ),
            snip.software_version.clone(),
        )
    } else {
        ("unknown_node_type".to_string(), "unknown".to_string())
    };

    let snip = if let Some(snip) = snapshot.snip_data {
        SnipSnapshot {
            user_name: snip.user_name,
            user_description: snip.user_description,
            manufacturer_name: snip.manufacturer,
            model_name: snip.model,
        }
    } else {
        SnipSnapshot::default()
    };

    Ok(NodeSnapshot {
        node_id: canonical_node_id(&snapshot.node_id.to_hex_string()),
        captured_at: captured_at.to_string(),
        capture_status,
        missing,
        snip,
        cdi_ref: CdiReference {
            cache_key,
            version,
            fingerprint: cdi_fingerprint,
        },
        values,
        producer_identified_events: producer_events,
    })
}

#[tauri::command]
pub async fn capture_layout_snapshot(
    include_producer_events: bool,
    state: tauri::State<'_, AppState>,
) -> Result<CaptureSummary, String> {
    let _ = include_producer_events;

    let captured_at = chrono::Utc::now().to_rfc3339();
    let handles = state.node_registry.get_all_handles().await;

    let mut producer_events_by_node: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(catalog) = state.bowties_catalog.read().await.clone() {
        for bowtie in catalog.bowties {
            for producer in bowtie.producers {
                let key = canonical_node_id(&producer.node_id);
                producer_events_by_node.entry(key).or_default().push(bowtie.event_id_hex.clone());
            }
        }
    }

    let mut node_count = 0usize;
    let mut complete_count = 0usize;
    let mut partial_count = 0usize;

    for handle in handles {
        let node_key = canonical_node_id(&handle.node_id.to_hex_string());
        let producer_events = producer_events_by_node
            .get(&node_key)
            .cloned()
            .unwrap_or_default();
        let snap = build_node_snapshot(&handle, &captured_at, producer_events).await?;
        node_count += 1;
        if snap.capture_status == CaptureStatus::Complete {
            complete_count += 1;
        } else {
            partial_count += 1;
        }
    }

    Ok(CaptureSummary {
        captured_at,
        node_count,
        complete_count,
        partial_count,
    })
}

#[tauri::command]
pub async fn save_layout_directory(
    path: String,
    overwrite: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SaveLayoutResult, String> {
    let target = std::path::Path::new(&path);
    if target.exists() && !overwrite {
        return Err(format!("Target directory already exists: {}", target.display()));
    }

    let captured_at = chrono::Utc::now().to_rfc3339();
    let handles = state.node_registry.get_all_handles().await;

    let mut producer_events_by_node: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(catalog) = state.bowties_catalog.read().await.clone() {
        for bowtie in &catalog.bowties {
            for producer in &bowtie.producers {
                let key = canonical_node_id(&producer.node_id);
                producer_events_by_node
                    .entry(key)
                    .or_default()
                    .push(bowtie.event_id_hex.clone());
            }
        }
    }

    let mut node_snapshots = Vec::new();
    let mut partial_nodes = Vec::new();
    for handle in &handles {
        let node_key = canonical_node_id(&handle.node_id.to_hex_string());
        let producer_events = producer_events_by_node
            .get(&node_key)
            .cloned()
            .unwrap_or_default();
        let snap = build_node_snapshot(handle, &captured_at, producer_events).await?;
        if snap.capture_status == CaptureStatus::Partial {
            partial_nodes.push(snap.node_id.clone());
        }
        node_snapshots.push(snap);
    }

    let manifest = LayoutManifest::new(
        target
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("layout")
            .to_string(),
        captured_at.clone(),
        chrono::Utc::now().to_rfc3339(),
    );

    let mut bowties = crate::layout::types::LayoutFile::default();

    if target.exists() {
        if let Ok(previous) = crate::layout::io::read_layout_directory(target) {
            bowties.role_classifications = previous.bowties.role_classifications;
            bowties.bowties = previous.bowties.bowties;
        }
    }

    if let Some(catalog) = state.bowties_catalog.read().await.clone() {
        for b in catalog.bowties {
            if !b.tags.is_empty() || b.name.is_some() {
                bowties.bowties.insert(
                    b.event_id_hex,
                    crate::layout::types::BowtieMetadata {
                        name: b.name,
                        tags: b.tags,
                    },
                );
            }
        }
    }

    let write_data = crate::layout::io::LayoutDirectoryWriteData {
        manifest,
        node_snapshots,
        bowties,
        offline_changes: Vec::<OfflineChange>::new(),
    };

    crate::layout::io::write_layout_directory(target, &write_data)?;

    let _ = crate::commands::bowties::set_recent_layout(path.clone(), app.clone()).await;

    let context = ActiveLayoutContext {
        layout_id: target
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("layout")
            .to_string(),
        root_path: path.clone(),
        mode: ActiveLayoutMode::OfflineDirectory,
        captured_at: Some(captured_at),
        pending_offline_change_count: 0,
    };
    *state.active_layout.write().await = Some(context);

    Ok(SaveLayoutResult {
        manifest_path: target.join("manifest.yaml").to_string_lossy().to_string(),
        node_files_written: handles.len(),
        warnings: partial_nodes,
    })
}

#[tauri::command]
pub async fn open_layout_directory(
    path: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<OpenLayoutResult, String> {
    let root = std::path::Path::new(&path);
    let loaded = crate::layout::io::read_layout_directory(root)?;

    let partial_nodes = loaded
        .node_snapshots
        .iter()
        .filter(|n| n.capture_status == CaptureStatus::Partial)
        .map(|n| n.node_id.clone())
        .collect::<Vec<_>>();

    let _ = crate::commands::bowties::set_recent_layout(path.clone(), app.clone()).await;

    let context = ActiveLayoutContext {
        layout_id: loaded.manifest.layout_id.clone(),
        root_path: path.clone(),
        mode: ActiveLayoutMode::OfflineDirectory,
        captured_at: Some(loaded.manifest.captured_at.clone()),
        pending_offline_change_count: loaded.offline_changes.len(),
    };
    *state.active_layout.write().await = Some(context);

    let _ = app.emit(
        "layout-opened",
        serde_json::json!({
            "layoutId": loaded.manifest.layout_id,
            "path": path,
            "capturedAt": loaded.manifest.captured_at,
            "offlineMode": true,
            "nodeCount": loaded.node_snapshots.len(),
        }),
    );

    Ok(OpenLayoutResult {
        layout_id: loaded.manifest.layout_id,
        captured_at: loaded.manifest.captured_at,
        offline_mode: true,
        node_count: loaded.node_snapshots.len(),
        partial_nodes,
        pending_offline_change_count: loaded.offline_changes.len(),
        node_snapshots: loaded.node_snapshots,
    })
}

#[tauri::command]
pub async fn close_layout(
    decision: CloseLayoutDecision,
    state: tauri::State<'_, AppState>,
) -> Result<CloseLayoutResult, String> {
    if matches!(decision, CloseLayoutDecision::Cancel) {
        return Ok(CloseLayoutResult {
            closed: false,
            reason: Some("cancelled".to_string()),
        });
    }

    *state.active_layout.write().await = None;

    Ok(CloseLayoutResult {
        closed: true,
        reason: None,
    })
}

#[tauri::command]
pub async fn create_new_layout_capture(
    state: tauri::State<'_, AppState>,
) -> Result<NewLayoutResult, String> {
    let created_at = chrono::Utc::now().to_rfc3339();
    let layout_id = format!("layout-{}", chrono::Utc::now().timestamp());

    let context = ActiveLayoutContext {
        layout_id: layout_id.clone(),
        root_path: String::new(),
        mode: ActiveLayoutMode::OfflineDirectory,
        captured_at: Some(created_at.clone()),
        pending_offline_change_count: 0,
    };
    *state.active_layout.write().await = Some(context);

    Ok(NewLayoutResult {
        layout_id,
        created_at,
    })
}
