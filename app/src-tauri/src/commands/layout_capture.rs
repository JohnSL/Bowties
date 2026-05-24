//! Offline layout capture/open commands.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use tauri::{Emitter, Manager};

use crate::layout::manifest::LayoutManifest;
use crate::layout::node_snapshot::{
    capture_status_from_missing, missing_detail, CaptureStatus, CdiReference, NodeSnapshot,
    SnapshotLeafValue, SnipSnapshot,
};
use crate::layout::offline_changes::OfflineChange;
use crate::layout::types::LayoutFile;
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
    /// The persisted layout file data (ADR-0002: backend returns authoritative copy).
    pub layout: LayoutFile,
    /// Canonical (uppercase, no-dots) node IDs of every snapshot written to the
    /// companion `nodes/` directory after this save. Frontend uses this to
    /// distinguish saved nodes from unsaved discovered nodes (S8).
    pub persisted_node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenLayoutResult {
    pub layout_id: String,
    pub captured_at: String,
    pub layout: LayoutFile,
    pub offline_mode: bool,
    pub node_count: usize,
    pub partial_nodes: Vec<String>,
    pub pending_offline_change_count: usize,
    pub node_snapshots: Vec<NodeSnapshot>,
    /// True when the layout journal (ADR-0006) rolled back an
    /// interrupted prior save before this open. The frontend surfaces
    /// a one-line notice when set.
    pub recovery_occurred: bool,
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

/// Result of the three-phase `save_layout_with_bus_writes` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWithBusWriteResult {
    /// Layout was successfully saved to disk in phase 1.
    pub layout_saved: bool,
    /// Bus write result from phase 2 (None if not connected or no pending writes).
    pub bus_writes: Option<super::cdi::WriteModifiedResult>,
    /// Phase 3 reconcile save completed (only true when ≥1 bus write succeeded).
    pub reconciled: bool,
    /// Catalog was rebuilt by the backend after save.
    pub catalog_rebuilt: bool,
    /// Partial-capture node IDs from the initial layout save (same semantics as SaveLayoutResult.warnings).
    pub warnings: Vec<String>,
    /// The persisted layout file data (ADR-0002: backend returns authoritative copy).
    pub layout: LayoutFile,
    /// Canonical node IDs persisted on disk after this save (S8).
    pub persisted_node_ids: Vec<String>,
}



fn canonical_node_id(node_id_dotted_hex: &str) -> String {
    node_id_dotted_hex.replace('.', "").to_uppercase()
}

fn config_value_to_string(value: &crate::node_tree::ConfigValue) -> String {
    value.to_snapshot_string()
}

fn collect_leaf_values(
    nodes: &[crate::node_tree::ConfigNode],
    hierarchy: &mut Vec<String>,
    snapshot: &mut NodeSnapshot,
    missing: &mut Vec<String>,
    state: &AppState,
    node_id: &str,
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
                    crate::bwlog!(
                        state,
                        "[layout capture] missing value: node={} leaf={} hierarchy={} cdi_path={} space={} offset={} type={:?}",
                        node_id,
                        leaf.name,
                        hierarchy.join(" / "),
                        leaf.path.join("/"),
                        leaf.space,
                        offset_key,
                        leaf.element_type,
                    );
                    missing.push(missing_detail(leaf.space, &offset_key, &leaf.path));
                }
            }
            crate::node_tree::ConfigNode::Group(group) => {
                let mut pushed = false;
                if let Some(group_key) = group_key(group) {
                    hierarchy.push(group_key);
                    pushed = true;
                }
                collect_leaf_values(&group.children, hierarchy, snapshot, missing, state, node_id);
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
    state: &AppState,
) -> Result<NodeSnapshot, String> {
    let snapshot = handle.get_snapshot().await?;
    let tree = handle.get_config_tree().await?;

    let cdi_fingerprint = if let Some(cdi) = &snapshot.cdi {
        format!("len:{}", cdi.xml_content.len())
    } else if snapshot.pip_status == lcc_rs::PIPStatus::Complete
        && snapshot.pip_flags.as_ref().is_some_and(|f| !f.cdi)
    {
        "not_supported".to_string()
    } else {
        "missing".to_string()
    };

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
        node_id: snapshot.node_id,
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

    let tree_segment_count = tree.as_ref().map(|t| t.segments.len()).unwrap_or(0);
    crate::bwlog!(
        state,
        "[layout capture] snapshot start: node={} manufacturer={} model={} tree_available={} segments={}",
        snapshot.node_id,
        snapshot.snip.manufacturer_name,
        snapshot.snip.model_name,
        tree.is_some(),
        tree_segment_count,
    );
    let node_id_for_logs = snapshot.node_id.to_string();

    let mut missing = Vec::new();
    if let Some(tree) = tree {
        for segment in &tree.segments {
            let mut hierarchy = vec![segment.name.clone()];
            collect_leaf_values(
                &segment.children,
                &mut hierarchy,
                &mut snapshot,
                &mut missing,
                state,
                &node_id_for_logs,
            );
        }
    } else {
        crate::bwlog!(
            state,
            "[layout capture] missing configuration tree: node={} manufacturer={} model={}",
            snapshot.node_id,
            snapshot.snip.manufacturer_name,
            snapshot.snip.model_name,
        );
        missing.push("configuration tree not available".to_string());
    }

    snapshot.missing = missing;
    snapshot.capture_status = capture_status_from_missing(&snapshot.missing);
    crate::bwlog!(
        state,
        "[layout capture] snapshot complete: node={} status={:?} missing_count={}",
        snapshot.node_id,
        snapshot.capture_status,
        snapshot.missing.len(),
    );
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
        let node_key = handle.node_id.to_canonical();
        let producer_events = producer_events_by_node
            .get(&node_key)
            .cloned()
            .unwrap_or_default();
        let snap = build_node_snapshot(&handle, &captured_at, producer_events, state.inner()).await?;
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
    deltas: Vec<crate::layout::types::LayoutEditDelta>,
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

    // Read existing layout data once (needed for layout_id, bowties, offline_changes,
    // and as fallback snapshot source when re-saving offline).
    let previous = if target.exists() {
        crate::layout::read_capture(target).ok()
    } else {
        None
    };

    // Resolve snapshots: live node registry takes priority (fresh data from bus),
    // otherwise fall back to existing companion dir snapshots (re-save while offline).
    //
    // S8: the layout is the durable source of truth for which nodes belong to it.
    // The permitted set is the union of (a) node IDs already saved in this layout
    // and (b) node IDs explicitly promoted via an `AddNode` delta in this save.
    // Discovered nodes that are not in either set are excluded — they remain
    // in-memory drafts on the frontend and do not pollute layout A when the user
    // accidentally connects to bus B.
    let previous_node_ids: std::collections::BTreeSet<String> = previous
        .as_ref()
        .map(|p| {
            p.node_snapshots
                .iter()
                .map(|s| s.node_id.to_canonical())
                .collect()
        })
        .unwrap_or_default();
    let added_node_ids: std::collections::BTreeSet<String> = deltas
        .iter()
        .filter_map(|d| d.as_add_node().map(|s| s.replace('.', "").to_uppercase()))
        .collect();
    let permitted_node_ids: std::collections::BTreeSet<String> = previous_node_ids
        .union(&added_node_ids)
        .cloned()
        .collect();
    let has_persisted_layout = previous.is_some();

    let handles = state.node_registry.get_all_handles().await;
    let mut snapshots = if !handles.is_empty() {
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
        let mut covered: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for handle in &handles {
            let node_key = handle.node_id.to_canonical();
            // S8: skip handles for nodes not in the permitted set. Exception:
            // before the layout has ever been persisted (first save of a brand
            // new layout that has no `AddNode` deltas yet), we keep every
            // discovered handle so existing legacy callers and tests continue
            // to work. Once the layout is on disk, every node must be either
            // already saved or explicitly added via a delta.
            if has_persisted_layout && !permitted_node_ids.contains(&node_key) {
                continue;
            }
            let producer_events = producer_events_by_node
                .get(&node_key)
                .cloned()
                .unwrap_or_default();
            out.push(build_node_snapshot(handle, &captured_at, producer_events, state.inner()).await?);
            covered.insert(node_key);
        }
        // S8: preserve previously-saved snapshots for permitted nodes that are
        // not currently on the bus (saved + off-bus case). Without this, a
        // re-save while one of the layout's nodes is offline would silently
        // drop that node from the layout.
        if let Some(ref prev) = previous {
            for snap in &prev.node_snapshots {
                let key = snap.node_id.to_canonical();
                if permitted_node_ids.contains(&key) && !covered.contains(&key) {
                    out.push(snap.clone());
                }
            }
        }
        out
    } else if let Some(ref prev) = previous {
        prev.node_snapshots.clone()
    } else {
        Vec::new()
    };

    // Filter out nodes that do not support CDI — they cannot be usefully saved
    // and cause "(Not captured)" banners on re-open.
    snapshots.retain(|s| {
        s.cdi_ref.fingerprint != "not_supported" && s.cdi_ref.fingerprint != "missing"
    });

    let partial_nodes: Vec<String> = snapshots
        .iter()
        .filter(|s| s.capture_status == CaptureStatus::Partial)
        .map(|s| s.node_id.to_canonical())
        .collect();

    let mut manifest_captured_at = snapshots
        .first()
        .map(|s| s.captured_at.clone())
        .unwrap_or_else(|| captured_at.clone());
    let mut layout_id = target
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("layout")
        .to_string();
    // `save_capture` (below) sets `manifest.companion_dir` itself from the
    // base-file name, so we pass an empty string here.
    let companion_dir = String::new();

    // ADR-0002: read disk-authoritative layout, apply frontend deltas.
    let mut bowties = previous.as_ref().map(|p| p.bowties.clone()).unwrap_or_default();
    crate::layout::types::apply_layout_deltas(&mut bowties, deltas);
    let mut offline_changes = Vec::<OfflineChange>::new();

    if let Some(prev) = previous {
        layout_id = prev.manifest.layout_id;
        manifest_captured_at = prev.manifest.captured_at;
        offline_changes = prev.offline_changes;
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
        // Persist all resolved (non-ambiguous) role classifications from the live catalog.
        let resolved_roles = crate::commands::bowties::extract_catalog_role_classifications(&catalog);
        for (key, rc) in resolved_roles {
            bowties.role_classifications.insert(key, rc);
        }

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
    bowties.validate()?;

    let manifest = LayoutManifest::new(
        layout_id.clone(),
        manifest_captured_at.clone(),
        chrono::Utc::now().to_rfc3339(),
        companion_dir,
    );

    // Collect CDI files from cache, skipping nodes that don't support CDI.
    let mut cdi_files: Vec<(String, std::path::PathBuf)> = Vec::new();
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data directory: {}", e))?;
    for snapshot in &snapshots {
        if snapshot.cdi_ref.fingerprint == "not_supported"
            || snapshot.cdi_ref.fingerprint == "missing"
        {
            continue;
        }
        let cdi_path = crate::layout::io::cdi_cache_path(&snapshot, &app_data_dir);
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
    let write_data = crate::layout::LayoutDirectoryWriteData {
        manifest,
        node_snapshots: snapshots,
        bowties,
        offline_changes,
        cdi_files,
    };

    crate::layout::save_capture(target, &write_data)?;

    let _ = crate::commands::bowties::set_recent_layout(path.clone(), app.clone()).await;

    let layout_node_ids = write_data.node_snapshots.iter().map(|s| s.node_id.clone()).collect();
    let context = ActiveLayoutContext {
        layout_id,
        root_path: path.clone(),
        mode: ActiveLayoutMode::OfflineFile,
        captured_at: Some(manifest_captured_at),
        pending_offline_change_count: write_data.offline_changes.len(),
        layout_node_ids,
    };
    *state.active_layout.write().await = Some(context);
    *state.offline_changes_cache.write().await = write_data.offline_changes.clone();

    Ok(SaveLayoutResult {
        manifest_path: target.to_string_lossy().to_string(),
        node_files_written: write_data.node_snapshots.len(),
        cdi_files_copied: cdi_files_count,
        warnings: partial_nodes,
        layout: write_data.bowties.clone(),
        persisted_node_ids: write_data
            .node_snapshots
            .iter()
            .map(|s| s.node_id.to_canonical())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use crate::layout::types::{apply_layout_deltas, LayoutEditDelta};

    #[test]
    fn apply_deltas_connector_selection_replaces_previous() {
        let mut layout = crate::layout::types::LayoutFile::default();
        layout.connector_selections.insert(
            "020157000001".to_string(),
            crate::layout::types::NodeHardwareSelectionSet {
                carrier_key: "old-carrier".to_string(),
                slot_selections: std::collections::BTreeMap::new(),
                updated_at: Some("2026-01-01T00:00:00Z".to_string()),
            },
        );

        let mut slot_selections = std::collections::BTreeMap::new();
        slot_selections.insert(
            "connector-a".to_string(),
            crate::layout::types::ConnectorSelectionRecord {
                selected_daughterboard_id: Some("BOD4-CP".to_string()),
                status: crate::layout::types::ConnectorSelectionStatus::Selected,
                source_profile_version: None,
            },
        );

        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::SetConnectorSelection {
                node_id: "020157000001".to_string(),
                selection: crate::layout::types::NodeHardwareSelectionSet {
                    carrier_key: "rr-cirkits::tower-lcc".to_string(),
                    slot_selections,
                    updated_at: Some("2026-05-02T12:00:00Z".to_string()),
                },
            },
        ]);

        let stored = layout.connector_selections.get("020157000001").unwrap();
        assert_eq!(stored.carrier_key, "rr-cirkits::tower-lcc");
        assert_eq!(stored.slot_selections.get("connector-a").unwrap().selected_daughterboard_id.as_deref(), Some("BOD4-CP"));
    }
}

#[tauri::command]
pub async fn open_layout_directory(
    path: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<OpenLayoutResult, String> {
    let input_path = std::path::Path::new(&path);
    let loaded = crate::layout::read_capture(input_path)?;
    let recent_path = input_path.to_string_lossy().to_string();

    let partial_nodes: Vec<String> = loaded
        .node_snapshots
        .iter()
        .filter(|n| n.capture_status == CaptureStatus::Partial)
        .map(|n| n.node_id.to_canonical())
        .collect();

    let _ = crate::commands::bowties::set_recent_layout(recent_path.clone(), app.clone()).await;

    // Load offline changes into cache
    *state.offline_changes_cache.write().await = loaded.offline_changes.clone();

    // Populate offline bowtie data from snapshots so offline catalog rebuilds
    // can discover event slots and merge saved role classifications.
    {
        let mut offline_data = crate::state::OfflineBowtieData::default();
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Cannot resolve app data directory: {}", e))?;

        for snapshot in &loaded.node_snapshots {
            if snapshot.cdi_ref.fingerprint == "not_supported"
                || snapshot.cdi_ref.fingerprint == "missing"
            {
                continue;
            }
            let dotted_id = snapshot.node_id.to_hex_string();

            // Load CDI XML
            let xml = match crate::layout::resolve_cdi_xml_for_snapshot(
                input_path,
                snapshot,
                &app_data_dir,
            ) {
                Ok(xml) => xml,
                Err(_) => continue,
            };
            offline_data.cdi_xml.insert(dotted_id.clone(), xml.clone());

            // Parse CDI, build minimal tree, merge snapshot values, extract event ID leaves
            let cdi = match lcc_rs::cdi::parser::parse_cdi(&xml) {
                Ok(cdi) => cdi,
                Err(_) => continue,
            };
            let mut tree = crate::node_tree::build_node_config_tree(&dotted_id, &cdi);
            crate::node_tree::merge_snapshot_path_values(&mut tree, &snapshot.config);

            let mut node_config: std::collections::HashMap<String, [u8; 8]> = std::collections::HashMap::new();
            for leaf in crate::node_tree::collect_event_id_leaves(&tree) {
                if let Some(bytes) = leaf.value {
                    node_config.insert(leaf.path.join("/"), bytes);
                }
            }
            if !node_config.is_empty() {
                offline_data.config_values.insert(dotted_id.clone(), node_config);
            }
        }

        // Convert saved role_classifications into profile_roles format
        for (key, rc) in &loaded.bowties.role_classifications {
            let role = match rc.role.as_str() {
                "Producer" => lcc_rs::EventRole::Producer,
                "Consumer" => lcc_rs::EventRole::Consumer,
                _ => continue,
            };
            offline_data.profile_roles.insert(key.clone(), role);
        }

        *state.offline_bowtie_data.write().await = offline_data;
    }

    let layout_node_ids = loaded.node_snapshots.iter().map(|s| s.node_id.clone()).collect();
    let context = ActiveLayoutContext {
        layout_id: loaded.manifest.layout_id.clone(),
        root_path: recent_path.clone(),
        mode: ActiveLayoutMode::OfflineFile,
        captured_at: Some(loaded.manifest.captured_at.clone()),
        pending_offline_change_count: loaded.offline_changes.len(),
        layout_node_ids,
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
            "recoveryOccurred": loaded.recovery_occurred,
        }),
    );

    Ok(OpenLayoutResult {
        layout_id: loaded.manifest.layout_id,
        captured_at: loaded.manifest.captured_at,
        layout: loaded.bowties,
        offline_mode: true,
        node_count: loaded.node_snapshots.len(),
        partial_nodes,
        pending_offline_change_count: loaded.offline_changes.len(),
        node_snapshots: loaded.node_snapshots,
        recovery_occurred: loaded.recovery_occurred,
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
    *state.offline_bowtie_data.write().await = Default::default();
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
        layout_node_ids: Vec::new(),
    };
    *state.active_layout.write().await = Some(context);
    *state.offline_changes_cache.write().await = Vec::new();

    Ok(NewLayoutResult {
        layout_id,
        created_at,
    })
}

/// Three-phase save: layout first, then bus writes (if connected), then reconcile.
///
/// Phase 1 — Persist layout to disk (with all resolved event roles from the live catalog).
/// Phase 2 — If connected and there are pending modified values, write them to bus nodes.
/// Phase 3 — If any writes succeeded, re-save the layout to clear succeeded offline changes.
/// Phase 4 — Rebuild the bowtie catalog.
///
/// Progress events (type `save-progress`) are emitted on the Tauri event bus before each phase.
#[tauri::command]
pub async fn save_layout_with_bus_writes(
    path: String,
    overwrite: bool,
    deltas: Vec<crate::layout::types::LayoutEditDelta>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SaveWithBusWriteResult, String> {
    use tauri::Emitter;

    let is_connected = state.connection.read().await.is_some();

    // Phase 1: Save layout (with resolved role persistence via save_layout_directory).
    let _ = app.emit("save-progress", serde_json::json!({ "phase": "saving-layout" }));
    let save_result = save_layout_directory(
        path.clone(),
        overwrite,
        deltas,
        app.clone(),
        state.clone(),
    ).await?;

    // Phase 2: Bus writes (only when connected).
    let bus_writes = if is_connected {
        let _ = app.emit("save-progress", serde_json::json!({
            "phase": "writing-config",
            "current": 0,
            "total": 0,
        }));
        let result = super::cdi::write_modified_values(state.clone(), app.clone()).await
            .unwrap_or_else(|_| super::cdi::WriteModifiedResult {
                total: 0,
                succeeded: 0,
                failed: 0,
                read_only_rejected: 0,
            });
        if result.total > 0 { Some(result) } else { None }
    } else {
        None
    };

    // Phase 3: Reconcile — re-save if any writes succeeded to clear applied offline changes.
    let reconciled = if let Some(ref writes) = bus_writes {
        if writes.succeeded > 0 {
            let _ = app.emit("save-progress", serde_json::json!({ "phase": "reconciling" }));
            // Reconcile re-save: no deltas needed, just re-persist current state.
            save_layout_directory(path.clone(), true, vec![], app.clone(), state.clone()).await?;
            true
        } else {
            false
        }
    } else {
        false
    };

    // Phase 4: Rebuild bowtie catalog with saved layout metadata so user-added
    // bowties, names, tags, and role classifications survive the rebuild.
    // Re-read the layout just written in Phase 1/3 to get the authoritative metadata.
    let final_read = crate::layout::read_capture(std::path::Path::new(&path)).ok();
    let saved_layout = final_read.as_ref().map(|loaded| loaded.bowties.clone());
    let catalog_rebuilt = crate::commands::bowties::build_bowtie_catalog_command(
        saved_layout,
        app.clone(),
        state.clone(),
    ).await.is_ok();

    // ADR-0002: return the persisted layout to the frontend.
    let persisted_layout = final_read
        .map(|loaded| loaded.bowties)
        .unwrap_or_else(|| save_result.layout.clone());

    let failed_count = bus_writes.as_ref().map(|w| w.failed).unwrap_or(0);
    let _ = app.emit("save-progress", serde_json::json!({
        "phase": "complete",
        "failedCount": failed_count,
    }));

    Ok(SaveWithBusWriteResult {
        layout_saved: true,
        bus_writes,
        reconciled,
        catalog_rebuilt,
        warnings: save_result.warnings,
        layout: persisted_layout,
        persisted_node_ids: save_result.persisted_node_ids,
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
    let snapshot = crate::layout::read_node_snapshot(base_file, &node_id)?;

    if snapshot.cdi_ref.fingerprint == "not_supported"
        || snapshot.cdi_ref.fingerprint == "missing"
    {
        return Err(format!(
            "Node {} does not provide CDI; offline configuration is not available",
            node_id
        ));
    }

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data directory: {}", e))?;
    let xml = crate::layout::resolve_cdi_xml_for_snapshot(base_file, &snapshot, &app_data_dir)
        .map_err(|e| format!("CDI not available for node {}: {}", node_id, e))?;

    let cdi = lcc_rs::cdi::parser::parse_cdi(&xml)
        .map_err(|e| format!("Cannot parse CDI for {}: {}", node_id, e))?;

    let parsed_nid = lcc_rs::NodeID::from_hex_string(&node_id)
        .map_err(|e| format!("Invalid node ID '{}': {}", node_id, e))?;
    let dotted_id = parsed_nid.to_hex_string();
    let mut tree = crate::node_tree::build_node_config_tree(&dotted_id, &cdi);
    crate::node_tree::merge_snapshot_path_values(&mut tree, &snapshot.config);

    if let Some(identity) = &cdi.identification {
        let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
        let model = identity.model.as_deref().unwrap_or("");
        if !manufacturer.is_empty() || !model.is_empty() {
            if let Some(profile) = crate::profile::load_profile(manufacturer, model, &cdi, &app, &state.profiles).await {
                let report = crate::profile::annotate_tree(&mut tree, &profile, &cdi);
                let shared_daughterboards = crate::profile::load_shared_daughterboards(&app).await;
                let connector_profile_outcome = crate::profile::build_connector_profile_with_diagnostics(
                    &dotted_id,
                    &profile,
                    shared_daughterboards.as_ref(),
                    &cdi,
                );
                tree.connector_profile = connector_profile_outcome.profile;
                tree.connector_profile_warning = connector_profile_outcome.warning;
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

    // ── Accumulate offline bowtie data for later catalog build ────────────────
    {
        let mut offline_data = state.offline_bowtie_data.write().await;

        // Store CDI XML for slot walking
        offline_data.cdi_xml.insert(dotted_id.clone(), xml);

        // Extract EventId config values from the tree
        let mut node_config: HashMap<String, [u8; 8]> = HashMap::new();
        for leaf in crate::node_tree::collect_event_id_leaves(&tree) {
            if let Some(role) = leaf.event_role {
                if role != lcc_rs::EventRole::Ambiguous {
                    let key = format!("{}:{}", dotted_id, leaf.path.join("/"));
                    offline_data.profile_roles.insert(key, role);
                }
            }
            if let Some(bytes) = leaf.value {
                node_config.insert(leaf.path.join("/"), bytes);
            }
        }
        if !node_config.is_empty() {
            offline_data.config_values.insert(dotted_id.clone(), node_config);
        }
    }

    Ok(tree)
}
