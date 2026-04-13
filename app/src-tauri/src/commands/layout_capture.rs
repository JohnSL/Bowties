//! Offline layout capture/open commands.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::{Emitter, Manager};

use crate::layout::manifest::LayoutManifest;
use crate::layout::node_snapshot::{
    capture_status_from_missing, missing_detail, CaptureStatus, CdiReference, NodeSnapshot,
    SnapshotLeafValue, SnipSnapshot,
};
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
    pub cdi_files_copied: usize,
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

fn canonical_to_dotted_node_id(node_id: &str) -> String {
    node_id
        .as_bytes()
        .chunks(2)
        .map(|c| std::str::from_utf8(c).unwrap_or("00"))
        .collect::<Vec<_>>()
        .join(".")
}

fn config_value_to_string(value: &crate::node_tree::ConfigValue) -> String {
    match value {
        crate::node_tree::ConfigValue::Int { value } => value.to_string(),
        crate::node_tree::ConfigValue::String { value } => value.clone(),
        crate::node_tree::ConfigValue::EventId { hex, .. } => hex.clone(),
        crate::node_tree::ConfigValue::Float { value } => value.to_string(),
    }
}

fn sanitize_cache_fragment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn cdi_cache_path_for_snapshot(snapshot: &NodeSnapshot, app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data directory: {}", e))?;

    let cdi_filename = format!(
        "{}_{}_{}.cdi.xml",
        sanitize_cache_fragment(&snapshot.snip.manufacturer_name),
        sanitize_cache_fragment(&snapshot.snip.model_name),
        sanitize_cache_fragment(&snapshot.cdi_ref.version),
    );
    Ok(app_data_dir.join("cdi_cache").join(cdi_filename))
}

fn collect_leaf_values(
    nodes: &[crate::node_tree::ConfigNode],
    hierarchy: &mut Vec<String>,
    snapshot: &mut NodeSnapshot,
    missing: &mut Vec<String>,
) {
    for node in nodes {
        match node {
            crate::node_tree::ConfigNode::Leaf(leaf) => {
                let offset_key = format!("0x{:08X}", leaf.address);
                if let Some(v) = &leaf.value {
                    let value = config_value_to_string(v);
                        let mut named_path = hierarchy.clone();
                    named_path.push(leaf.name.clone());
                    snapshot.add_config_leaf(
                        &named_path,
                        SnapshotLeafValue {
                            value,
                            space: Some(leaf.space),
                            offset: Some(offset_key),
                        },
                    );
                } else {
                    missing.push(missing_detail(leaf.space, &offset_key, &leaf.path));
                }
            }
            crate::node_tree::ConfigNode::Group(group) => {
                let mut pushed = false;
                if let Some(group_key) = group_key(group) {
                    hierarchy.push(group_key);
                    pushed = true;
                }
                collect_leaf_values(&group.children, hierarchy, snapshot, missing);
                if pushed {
                    let _ = hierarchy.pop();
                }
            }
        }
    }
}

fn group_key(group: &crate::node_tree::GroupNode) -> Option<String> {
    // Replication wrapper groups (instance=0) are structural only.
    if group.instance == 0 && group.replication_count > 1 {
        return None;
    }

    if group.replication_count > 1 && group.instance > 0 {
        return Some(format!("{}({})", group.name, group.instance - 1));
    }

    Some(group.name.clone())
}

async fn build_node_snapshot(
    handle: &crate::node_proxy::NodeProxyHandle,
    captured_at: &str,
    producer_events: Vec<String>,
) -> Result<NodeSnapshot, String> {
    let snapshot = handle.get_snapshot().await?;
    let tree = handle.get_config_tree().await?;

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

    let mut snapshot = NodeSnapshot {
        node_id: canonical_node_id(&snapshot.node_id.to_hex_string()),
        captured_at: captured_at.to_string(),
        capture_status: CaptureStatus::Complete,
        missing: Vec::new(),
        snip,
        cdi_ref: CdiReference {
            cache_key,
            version,
            fingerprint: cdi_fingerprint,
        },
        config: BTreeMap::new(),
        producer_identified_events: producer_events,
    };

    let mut missing = Vec::new();
    if let Some(tree) = tree {
        for segment in &tree.segments {
            let mut hierarchy = vec![segment.name.clone()];
            collect_leaf_values(&segment.children, &mut hierarchy, &mut snapshot, &mut missing);
        }
    } else {
        missing.push("configuration tree not available".to_string());
    }

    snapshot.missing = missing;
    snapshot.capture_status = capture_status_from_missing(&snapshot.missing);
    Ok(snapshot)
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
                producer_events_by_node
                    .entry(key)
                    .or_default()
                    .push(bowtie.event_id_hex.clone());
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
    node_snapshots: Option<Vec<NodeSnapshot>>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SaveLayoutResult, String> {
    let target = std::path::Path::new(&path);
    if target.is_dir() {
        return Err(format!(
            "Save target must be a layout file path, not a directory: {}",
            target.display()
        ));
    }
    if target.exists() && !overwrite {
        return Err(format!("Target layout file already exists: {}", target.display()));
    }

    let captured_at = chrono::Utc::now().to_rfc3339();

    let snapshots = if let Some(snaps) = node_snapshots {
        snaps
    } else {
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

        let mut out = Vec::new();
        for handle in &handles {
            let node_key = canonical_node_id(&handle.node_id.to_hex_string());
            let producer_events = producer_events_by_node
                .get(&node_key)
                .cloned()
                .unwrap_or_default();
            out.push(build_node_snapshot(handle, &captured_at, producer_events).await?);
        }
        out
    };

    let partial_nodes = snapshots
        .iter()
        .filter(|s| s.capture_status == CaptureStatus::Partial)
        .map(|s| s.node_id.clone())
        .collect::<Vec<_>>();

    let mut manifest_captured_at = snapshots
        .first()
        .map(|s| s.captured_at.clone())
        .unwrap_or_else(|| captured_at.clone());
    let mut layout_id = target
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("layout")
        .to_string();
    let companion_dir = crate::layout::io::derive_companion_dir_name(target)?;

    let mut bowties = crate::layout::types::LayoutFile::default();
    let mut offline_changes = Vec::<OfflineChange>::new();

    if target.exists() {
        if let Ok(previous) = crate::layout::io::read_layout_capture(target) {
            layout_id = previous.manifest.layout_id;
            manifest_captured_at = previous.manifest.captured_at;
            bowties = previous.bowties;
            offline_changes = previous.offline_changes;
        }
    }

    // Single pre-save edit path: if saving the currently active offline layout,
    // persist the in-memory offline delta cache as the authoritative set.
    let active_context = state.active_layout.read().await.clone();
    if let Some(ctx) = active_context {
        if ctx.mode == ActiveLayoutMode::OfflineFile
            && std::path::Path::new(&ctx.root_path) == target
        {
            offline_changes = state.offline_changes_cache.read().await.clone();
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

    let manifest = LayoutManifest::new(
        layout_id.clone(),
        manifest_captured_at.clone(),
        chrono::Utc::now().to_rfc3339(),
        companion_dir,
    );

    // Collect CDI files from cache - error if any snapshot's CDI is missing
    let mut cdi_files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for snapshot in &snapshots {
        let cdi_path = cdi_cache_path_for_snapshot(&snapshot, &app)?;
        if !cdi_path.exists() {
            return Err(format!(
                "CDI file not found in cache for node {}: expected at {} (cache key: {})",
                snapshot.node_id,
                cdi_path.display(),
                snapshot.cdi_ref.cache_key
            ));
        }
        cdi_files.push((snapshot.cdi_ref.cache_key.clone(), cdi_path));
    }

    let cdi_files_count = cdi_files.len();
    let write_data = crate::layout::io::LayoutDirectoryWriteData {
        manifest,
        node_snapshots: snapshots,
        bowties,
        offline_changes,
        cdi_files,
    };

    crate::layout::io::write_layout_capture(target, &write_data)?;

    let _ = crate::commands::bowties::set_recent_layout(path.clone(), app.clone()).await;

    let context = ActiveLayoutContext {
        layout_id,
        root_path: path.clone(),
        mode: ActiveLayoutMode::OfflineFile,
        captured_at: Some(manifest_captured_at),
        pending_offline_change_count: write_data.offline_changes.len(),
    };
    *state.active_layout.write().await = Some(context);
    *state.offline_changes_cache.write().await = write_data.offline_changes.clone();

    Ok(SaveLayoutResult {
        manifest_path: target.to_string_lossy().to_string(),
        node_files_written: write_data.node_snapshots.len(),
        cdi_files_copied: cdi_files_count,
        warnings: partial_nodes,
    })
}

#[tauri::command]
pub async fn open_layout_directory(
    path: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<OpenLayoutResult, String> {
    let input_path = std::path::Path::new(&path);
    let loaded = crate::layout::io::read_layout_capture(input_path)?;
    let recent_path = input_path.to_string_lossy().to_string();

    let partial_nodes = loaded
        .node_snapshots
        .iter()
        .filter(|n| n.capture_status == CaptureStatus::Partial)
        .map(|n| n.node_id.clone())
        .collect::<Vec<_>>();

    let _ = crate::commands::bowties::set_recent_layout(recent_path.clone(), app.clone()).await;

    // Load offline changes into cache
    *state.offline_changes_cache.write().await = loaded.offline_changes.clone();

    let context = ActiveLayoutContext {
        layout_id: loaded.manifest.layout_id.clone(),
        root_path: recent_path.clone(),
        mode: ActiveLayoutMode::OfflineFile,
        captured_at: Some(loaded.manifest.captured_at.clone()),
        pending_offline_change_count: loaded.offline_changes.len(),
    };
    *state.active_layout.write().await = Some(context);

    let _ = app.emit(
        "layout-opened",
        serde_json::json!({
            "layoutId": loaded.manifest.layout_id,
            "path": recent_path,
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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<CloseLayoutResult, String> {
    if matches!(decision, CloseLayoutDecision::Cancel) {
        return Ok(CloseLayoutResult {
            closed: false,
            reason: Some("cancelled".to_string()),
        });
    }

    *state.active_layout.write().await = None;
    *state.offline_changes_cache.write().await = Vec::new();
    crate::commands::bowties::clear_recent_layout(app).await?;

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
        mode: ActiveLayoutMode::OfflineFile,
        captured_at: Some(created_at.clone()),
        pending_offline_change_count: 0,
    };
    *state.active_layout.write().await = Some(context);
    *state.offline_changes_cache.write().await = Vec::new();

    Ok(NewLayoutResult {
        layout_id,
        created_at,
    })
}

/// Build a fully CDI-structured `NodeConfigTree` for a node loaded from an
/// active offline layout context.
#[tauri::command]
pub async fn build_offline_node_tree(
    node_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<crate::node_tree::NodeConfigTree, String> {
    let context = {
        let guard = state.active_layout.read().await;
        guard
            .as_ref()
            .cloned()
            .filter(|c| c.mode == crate::state::ActiveLayoutMode::OfflineFile)
            .ok_or_else(|| "No offline layout is active".to_string())?
    };

    let base_file = std::path::Path::new(&context.root_path);
    let companion_dir = crate::layout::io::derive_companion_dir_path(base_file)?;
    let snapshot_path = companion_dir
        .join("nodes")
        .join(format!("{}.yaml", node_id.to_uppercase()));

    let snapshot: crate::layout::node_snapshot::NodeSnapshot = crate::layout::io::read_yaml_file(&snapshot_path)
        .map_err(|e| format!("Cannot load snapshot {}: {}", snapshot_path.display(), e))?;

    let cdi_path = cdi_cache_path_for_snapshot(&snapshot, &app)?;
    if !cdi_path.exists() {
        return Err(format!(
            "CDI not in cache: {} (key: {})",
            cdi_path.display(),
            snapshot.cdi_ref.cache_key
        ));
    }
    let xml = std::fs::read_to_string(&cdi_path)
        .map_err(|e| format!("Cannot read CDI cache file {}: {}", cdi_path.display(), e))?;

    let cdi = lcc_rs::cdi::parser::parse_cdi(&xml)
        .map_err(|e| format!("Cannot parse CDI for {}: {}", node_id, e))?;

    let dotted_id = canonical_to_dotted_node_id(&node_id);
    let mut tree = crate::node_tree::build_node_config_tree(&dotted_id, &cdi);
    crate::node_tree::merge_snapshot_path_values(&mut tree, &snapshot.config);

    if let Some(identity) = &cdi.identification {
        let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
        let model = identity.model.as_deref().unwrap_or("");
        if !manufacturer.is_empty() || !model.is_empty() {
            if let Some(profile) = crate::profile::load_profile(manufacturer, model, &cdi, &app, &state.profiles).await {
                let report = crate::profile::annotate_tree(&mut tree, &profile, &cdi);
                eprintln!(
                    "[offline profile] {} - {} event roles, {} rules applied, {} warnings",
                    node_id,
                    report.event_roles_applied,
                    report.rules_applied,
                    report.warnings.len()
                );
            }
        }
    }

    Ok(tree)
}
