//! Offline layout capture/open commands.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::{Emitter, Manager};

use crate::layout::node_snapshot::{
    CaptureStatus, NodeSnapshot,
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
    /// Node snapshots written to disk. Frontend caches these so the disconnect
    /// transition can rehydrate the offline view without re-opening the layout.
    pub node_snapshots: Vec<NodeSnapshot>,
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
    /// Human-readable warnings from load-time schema normalization
    /// (e.g. facility slot bindings referencing channels absent from
    /// `channels.yaml`). The frontend surfaces these as a toast on
    /// open; the cleaned documents reach disk on the next save.
    pub load_warnings: Vec<String>,
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
    /// Node snapshots written to disk. Frontend caches these so the disconnect
    /// transition can rehydrate the offline view without re-opening the layout.
    pub node_snapshots: Vec<NodeSnapshot>,
}


use bowties_core::layout::capture::ProxySnapshotData;
use bowties_core::layout::state::LayoutState;

/// Fetch pre-computed data from a `NodeProxyHandle` and build the
/// `ProxySnapshotData` struct needed by the pure bowties-core snapshot builder.
///
/// When `layout_state` is provided and the proxy has no in-memory CDI data,
/// the CDI XML length is filled in from `LayoutState` (saved or captured
/// layer). This is the fix for the regression where a freshly-spawned
/// proxy after reconnect produced a snapshot with `fingerprint == "missing"`
/// even though the bytes were on disk or had just been downloaded
/// (see ADR-0015).
async fn proxy_snapshot_data(
    handle: &crate::node_proxy::NodeProxyHandle,
    layout_state: Option<&LayoutState>,
) -> Result<ProxySnapshotData, String> {
    let discovered = handle.get_snapshot().await?;
    // Config tree: prefer LayoutState (captured-over-saved) when available.
    let tree = if let Some(ls) = layout_state {
        if let Ok(key) = crate::node_key::NodeKey::parse(&handle.node_key()) {
            ls.config_tree(&key).cloned()
        } else {
            handle.get_config_tree().await?
        }
    } else {
        handle.get_config_tree().await?
    };

    let (is_synthesized, synthesized_node_key, profile_stem) = match handle {
        crate::node_proxy::NodeProxyHandle::Synthesized(synth) => {
            (true, Some(synth.node_key.clone()), Some(synth.profile_stem.clone()))
        }
        _ => (false, None, None),
    };

    let mut cdi_xml_len = discovered.cdi.as_ref().map(|c| c.xml_content.len());
    if cdi_xml_len.is_none() {
        if let Some(ls) = layout_state {
            if let Ok(key) = crate::node_key::NodeKey::parse(&handle.node_key()) {
                cdi_xml_len = ls.cdi_xml(&key).map(|s| s.len());
            }
        }
    }

    Ok(ProxySnapshotData {
        is_synthesized,
        synthesized_node_key,
        profile_stem,
        node_id: if is_synthesized { None } else { Some(discovered.node_id) },
        snip_data: discovered.snip_data,
        cdi_xml_len,
        pip_status: discovered.pip_status,
        pip_cdi_flag: discovered.pip_flags.as_ref().is_some_and(|f| f.cdi),
        config_tree: tree,
    })
}

/// Thin wrapper: fetch proxy data, call bowties-core snapshot builder,
/// relay log messages via `bwlog!`.
async fn build_node_snapshot(
    handle: &crate::node_proxy::NodeProxyHandle,
    captured_at: &str,
    producer_events: Vec<String>,
    state: &AppState,
    layout_state: Option<&LayoutState>,
) -> Result<NodeSnapshot, String> {
    let data = proxy_snapshot_data(handle, layout_state).await?;
    let (snapshot, log_messages) =
        bowties_core::layout::capture::build_node_snapshot(&data, captured_at, producer_events)?;
    for msg in &log_messages {
        crate::bwlog!(state, "{}", msg);
    }
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
                let key = producer.node_key.to_string();
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

    let layout_state_snapshot = state.layout_state.read().await.clone();
    for handle in handles {
        let node_key = handle.node_key();
        let producer_events = producer_events_by_node
            .get(&node_key)
            .cloned()
            .unwrap_or_default();
        let snap = build_node_snapshot(
            &handle,
            &captured_at,
            producer_events,
            state.inner(),
            layout_state_snapshot.as_ref(),
        )
        .await?;
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
    if target.is_file() {
        return Err(format!(
            "Save target must be a layout directory path, not a file: {}",
            target.display()
        ));
    }
    if target.exists() && !overwrite {
        return Err(format!("Target layout directory already exists: {}", target.display()));
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
                .map(|s| s.node_key.clone())
                .collect()
        })
        .unwrap_or_default();
    // S8.11: `AddNode { node_key }` is the single delta variant for both
    // real nodes and placeholders.  All promoted node keys are unified.
    let added_node_keys: std::collections::BTreeSet<String> = deltas
        .iter()
        .filter_map(|d| d.as_add_node().map(|s| s.to_string()))
        .collect();
    // Symmetric to AddNode: a RemoveNode delta drops a previously-persisted
    // node from the permitted-write set. The companion nodes/ prune step
    // in `write_layout_capture` will then delete its `<key>.yaml` file.
    let removed_node_keys: std::collections::BTreeSet<String> = deltas
        .iter()
        .filter_map(|d| d.as_remove_node().map(|s| s.to_string()))
        .collect();
    let permitted_node_keys: std::collections::BTreeSet<String> = previous_node_ids
        .union(&added_node_keys)
        .filter(|k| !removed_node_keys.contains(*k))
        .cloned()
        .collect();

    let handles = state.node_registry.get_all_handles().await;
    let layout_state_snapshot = state.layout_state.read().await.clone();
    let mut snapshots = if !handles.is_empty() {
        let mut producer_events_by_node: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if let Some(catalog) = state.bowties_catalog.read().await.clone() {
            for bowtie in &catalog.bowties {
                for producer in &bowtie.producers {
                    let key = producer.node_key.to_string();
                    producer_events_by_node
                        .entry(key)
                        .or_default()
                        .push(bowtie.event_id_hex.clone());
                }
            }
        }

        let mut out = Vec::new();
        let mut covered: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        // Build a lookup from previous snapshots for fallback comparison.
        let previous_snap_by_key: std::collections::BTreeMap<String, &NodeSnapshot> = previous
            .as_ref()
            .map(|p| p.node_snapshots.iter().map(|s| (s.node_key.clone(), s)).collect())
            .unwrap_or_default();
        for handle in &handles {
            let node_key = handle.node_key();
            // S8 / ADR-0011 (2026-05-31 extension): the layout is the single
            // source of truth for which nodes belong to it. Every handle must
            // be either already saved in this layout or explicitly promoted
            // via an `AddNode` delta — including the brand-new-layout case.
            // The previous "no persisted layout ⇒ save every handle"
            // exception leaked stale registry entries (notably placeholders
            // surviving close_layout) into freshly-created layouts.
            if !permitted_node_keys.contains(&node_key) {
                continue;
            }
            let producer_events = producer_events_by_node
                .get(&node_key)
                .cloned()
                .unwrap_or_default();
            let fresh = build_node_snapshot(
                handle,
                &captured_at,
                producer_events,
                state.inner(),
                layout_state_snapshot.as_ref(),
            )
            .await?;

            // Never persist a partial snapshot when a more-complete previous
            // snapshot exists. This prevents data loss when a previously-saved
            // node is on the bus but hasn't had its config re-read this session.
            if fresh.capture_status == CaptureStatus::Partial {
                if let Some(prev) = previous_snap_by_key.get(&node_key) {
                    if prev.capture_status == CaptureStatus::Complete {
                        crate::bwlog!(
                            state.inner(),
                            "[layout save] keeping previous Complete snapshot for {} (fresh is Partial)",
                            node_key,
                        );
                        out.push((*prev).clone());
                        covered.insert(node_key);
                        continue;
                    }
                }
            }

            out.push(fresh);
            covered.insert(node_key);
        }
        // S8: preserve previously-saved snapshots for permitted nodes that are
        // not currently on the bus (saved + off-bus case). Without this, a
        // re-save while one of the layout's nodes is offline would silently
        // drop that node from the layout.
        if let Some(ref prev) = previous {
            for snap in &prev.node_snapshots {
                let key = snap.node_key.clone();
                if permitted_node_keys.contains(&key) && !covered.contains(&key) {
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

    // After the proxy walk, any snapshot still carrying `fingerprint == "missing"`
    // means neither the proxy nor `LayoutState` has CDI bytes for this node —
    // the user has nothing meaningful to persist for it yet. Log and drop so
    // the save stays "no data ⇒ no snapshot" without silently destroying
    // previously-saved data (`LayoutState` fallback in `proxy_snapshot_data`
    // already covers the saved + captured cases; see ADR-0015).
    let dropped_for_missing: Vec<String> = snapshots
        .iter()
        .filter(|s| s.cdi_ref.fingerprint == "missing")
        .map(|s| s.node_key.clone())
        .collect();
    if !dropped_for_missing.is_empty() {
        crate::bwlog!(
            state.inner(),
            "[layout save] {} snapshot(s) had no CDI data in proxy or LayoutState; not persisted: {:?}",
            dropped_for_missing.len(),
            dropped_for_missing,
        );
    }

    // Filter out nodes that do not support CDI — they cannot be usefully saved
    // and cause "(Not captured)" banners on re-open. The "missing" branch is
    // still filtered as a safety net for the truly-no-data case above.
    snapshots.retain(|s| {
        s.cdi_ref.fingerprint != "not_supported" && s.cdi_ref.fingerprint != "missing"
    });

    let partial_nodes: Vec<String> = snapshots
        .iter()
        .filter(|s| s.capture_status == CaptureStatus::Partial)
        .map(|s| s.node_key.clone())
        .collect();

    let mut manifest_captured_at = snapshots
        .first()
        .map(|s| s.captured_at.clone())
        .unwrap_or_else(|| captured_at.clone());
    // Layout ID defaults to the folder name.
    let mut layout_id = target
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("layout")
        .to_string();

    // ADR-0002: read disk-authoritative layout, apply frontend deltas.
    let mut bowties = previous.as_ref().map(|p| p.bowties.clone()).unwrap_or_default();
    let mut facilities = previous.as_ref().map(|p| p.facilities.clone()).unwrap_or_default();
    let mut channels_doc = previous
        .as_ref()
        .map(|p| p.channels.clone())
        .unwrap_or_default();
    crate::layout::facilities::apply_facility_deltas(&mut facilities, &deltas)
        .map_err(|e| format!("Failed to apply facility deltas: {}", e))?;
    crate::layout::channels::apply_channel_deltas(&mut channels_doc, &deltas)
        .map_err(|e| format!("Failed to apply channel deltas: {}", e))?;
    crate::layout::types::apply_layout_deltas(&mut bowties, deltas);
    let mut offline_changes = Vec::<OfflineChange>::new();

    if let Some(prev) = previous.as_ref() {
        layout_id = prev.manifest.layout_id.clone();
        manifest_captured_at = prev.manifest.captured_at.clone();
        offline_changes = prev.offline_changes.clone();
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

    // Merge protocol-discovered role classifications from LayoutState.
    // These were recorded by the catalog rebuild sites via
    // LayoutState::record_discovered_roles(), not from a stale catalog cache.
    // or_insert_with ensures user-explicit classifications (from deltas or
    // saved baseline) are NOT overwritten by protocol discoveries.
    {
        let layout_guard = state.layout_state.read().await;
        if let Some(layout_state) = layout_guard.as_ref() {
            for (key, rc) in layout_state.discovered_roles() {
                bowties.role_classifications.entry(key.clone()).or_insert_with(|| rc.clone());
            }
        }
    }
    bowties.validate()?;

    let manifest = crate::layout::manifest::build_save_manifest(
        previous.as_ref().map(|p| &p.manifest),
        layout_id.clone(),
        manifest_captured_at.clone(),
        chrono::Utc::now().to_rfc3339(),
    );

    // Collect CDI files, skipping nodes that don't support CDI.
    //
    // S8.6: per snapshot, the CDI source path depends on provenance:
    //   * Bundled placeholder (`fingerprint == "bundled"`) — source from
    //     the bundled `profiles/` resource directory; never expected in
    //     `cdi_cache/`.
    //   * Live node — source from `cdi_cache/{cache_key}.cdi.xml`.
    //
    // Both paths flow through `CdiReference::from_snip` /
    // `CdiReference::from_profile_stem` at mint time, so the lookup here
    // reads `cache_key` directly without re-deriving from SNIP.
    let mut cdi_files: Vec<(String, std::path::PathBuf)> = Vec::new();
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data directory: {}", e))?;
    let bundled_search_dirs = crate::commands::cdi::bundled_cdi_search_dirs(&app);
    for snapshot in &snapshots {
        if snapshot.cdi_ref.fingerprint == "not_supported"
            || snapshot.cdi_ref.fingerprint == "missing"
        {
            continue;
        }
        let cdi_path = if snapshot.cdi_ref.is_bundled() {
            let file_name = format!("{}.cdi.xml", snapshot.cdi_ref.cache_key);
            bundled_search_dirs
                .iter()
                .map(|dir| dir.join(&file_name))
                .find(|candidate| candidate.exists())
                .ok_or_else(|| {
                    format!(
                        "Bundled CDI not found for placeholder {}: '{}' missing from every bundled-profiles directory",
                        snapshot.node_key, file_name
                    )
                })?
        } else {
            let path = crate::layout::io::cdi_cache_path(&snapshot, &app_data_dir);
            if !path.exists() {
                return Err(format!(
                    "CDI file not found in cache for node {}: expected at {} (cache key: {})",
                    snapshot.node_key,
                    path.display(),
                    snapshot.cdi_ref.cache_key
                ));
            }
            path
        };
        cdi_files.push((snapshot.cdi_ref.cache_key.clone(), cdi_path));
    }

    let cdi_files_count = cdi_files.len();
    let write_data = crate::layout::LayoutDirectoryWriteData {
        manifest,
        node_snapshots: snapshots,
        bowties,
        offline_changes,
        cdi_files,
        channels: channels_doc,
        facilities,
    };

    crate::layout::save_capture(target, &write_data)?;

    let _ = crate::commands::bowties::set_recent_layout(path.clone(), app.clone()).await;

    // Spec 018 / S6 bugfix — refresh `LayoutState.saved` to match what we
    // just wrote to disk, and drop the drafts layer. Otherwise subsequent
    // reads through `effective_facilities()` / `effective_channels()`
    // would base their merge on a stale saved layer (the pre-save state
    // captured at open time) — see ADR-0015 §"Draft-layer sync".
    {
        let mut layout_guard = state.layout_state.write().await;
        if let Some(layout_state) = layout_guard.as_mut() {
            *layout_state.facilities_mut() = write_data.facilities.clone();
            *layout_state.channels_mut() = write_data.channels.clone();
            *layout_state.bowties_mut() = write_data.bowties.clone();
            layout_state.set_offline_changes(write_data.offline_changes.clone());
            layout_state.clear_drafts();
            layout_state.clear_discovered_roles();
        }
    }

    let layout_node_keys: Vec<crate::node_key::NodeKey> = write_data
        .node_snapshots
        .iter()
        .filter_map(|s| crate::node_key::NodeKey::parse(&s.node_key).ok())
        .collect();
    let context = ActiveLayoutContext {
        layout_id,
        root_path: path.clone(),
        mode: ActiveLayoutMode::OfflineFile,
        captured_at: Some(manifest_captured_at),
        pending_offline_change_count: write_data.offline_changes.len(),
        layout_node_keys,
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
            .map(|s| s.node_key.clone())
            .collect(),
        node_snapshots: write_data.node_snapshots,
    })
}

#[cfg(test)]
mod tests {
    // Connector-selection deltas were removed in Spec 014. The replacement
    // (placeholder boards + `node_mode_selections`) is exercised by
    // `layout::types::tests::s3_*`.

    use super::*;

    /// Regression: a delta-deleted bowtie must not be resurrected.
    /// After the catalog merge elimination, the save flow only reads
    /// discovered roles from LayoutState (using or_insert_with, which
    /// won't re-add deleted bowtie entries). This test pins the invariant
    /// at the LayoutFile level: once DeleteBowtie removes an entry,
    /// role-classification merges via or_insert_with on
    /// `role_classifications` cannot create a bowtie entry.
    #[test]
    fn delta_deleted_bowtie_not_resurrected_by_catalog_merge() {
        use crate::layout::types::{BowtieMetadata, LayoutEditDelta, RoleClassification};

        let mut bowties = bowties_core::layout::types::LayoutFile::default();

        // Simulate a delta-created bowtie with facility provenance.
        bowties.bowties.insert(
            "01.02.03.04.05.06.07.08".to_string(),
            BowtieMetadata {
                name: Some("Signal A".to_string()),
                tags: vec!["yard".to_string()],
                created_by_facility: Some("f-1".to_string()),
            },
        );

        // User deletes the bowtie via a delta.
        let deltas = vec![LayoutEditDelta::DeleteBowtie {
            event_id_hex: "01.02.03.04.05.06.07.08".to_string(),
        }];
        bowties_core::layout::types::apply_layout_deltas(&mut bowties, deltas);
        assert!(
            bowties.bowties.get("01.02.03.04.05.06.07.08").is_none(),
            "delete delta must remove the bowtie"
        );

        // Simulate what the save flow does: merge discovered role
        // classifications using or_insert_with. This must NOT recreate
        // a bowtie entry — role_classifications is a separate map.
        let mut discovered = std::collections::BTreeMap::new();
        discovered.insert(
            "01.02.03.04.05.06.07.08:some/path".to_string(),
            RoleClassification { role: "Producer".to_string() },
        );
        for (key, rc) in &discovered {
            bowties.role_classifications.entry(key.clone()).or_insert_with(|| rc.clone());
        }

        // The bowtie entry must still be absent — role classification
        // merge must not create bowtie metadata entries.
        assert!(
            bowties.bowties.get("01.02.03.04.05.06.07.08").is_none(),
            "role classification merge must not resurrect a delta-deleted bowtie"
        );
        // Role classification IS present (separate map, fine).
        assert!(bowties.role_classifications.contains_key("01.02.03.04.05.06.07.08:some/path"));
    }

    #[tokio::test]
    async fn s9_build_node_snapshot_placeholder_uses_bundled_cdi_ref() {
        // Synthesized placeholders must produce a CdiReference with
        // from_profile_stem (fingerprint = "bundled") so the save flow
        // resolves CDI from the bundled profiles directory, not cdi_cache.
        let proxy = crate::node_proxy::SynthesizedNodeProxy {
            node_key: "placeholder:test-uuid".to_string(),
            profile_stem: "Mustangpeak-Engineering_TurnoutBoss".to_string(),
            snip: Some(lcc_rs::SNIPData {
                manufacturer: "Mustangpeak Engineering".to_string(),
                model: "TurnoutBoss".to_string(),
                hardware_version: String::new(),
                software_version: "1.0".to_string(),
                user_name: String::new(),
                user_description: String::new(),
            }),
            cdi_data: Some(lcc_rs::CdiData {
                xml_content: "<cdi/>".to_string(),
                retrieved_at: chrono::Utc::now(),
            }),
            cdi_parsed: None,
            config_tree: None,
            producer_identified_events: Vec::new(),
        };
        let handle = crate::node_proxy::NodeProxyHandle::Synthesized(proxy);
        let state = AppState::new();

        let snap = build_node_snapshot(&handle, "2026-05-31T00:00:00Z", vec![], &state, None)
            .await
            .expect("build_node_snapshot should succeed for placeholder");

        // node_key must be the placeholder key, not a canonical NodeID
        assert_eq!(snap.node_key, "placeholder:test-uuid");
        // node_id must be None for placeholders
        assert!(snap.node_id.is_none(), "placeholder snapshot must have node_id = None");
        // profile_stem must be set
        assert_eq!(
            snap.profile_stem.as_deref(),
            Some("Mustangpeak-Engineering_TurnoutBoss")
        );
        // CdiReference must use from_profile_stem, not from_snip
        assert!(snap.cdi_ref.is_bundled(), "placeholder CdiReference must be bundled");
        assert_eq!(snap.cdi_ref.cache_key, "Mustangpeak-Engineering_TurnoutBoss");
        assert_eq!(snap.cdi_ref.version, "bundled");
        assert_eq!(snap.cdi_ref.fingerprint, "bundled");
    }

    // Step 4d behavior pin: a fully-populated config tree produces a
    // `Complete` capture with an empty `missing` list. Pre-migration the
    // registry-miss bug left leaves with `value: None`, which surfaced as
    // "missing value" log spam and a `Partial` capture status.
    #[tokio::test]
    async fn populated_tree_yields_complete_capture_with_no_missing_values() {
        use crate::node_tree::{
            ConfigNode, ConfigValue, LeafNode, LeafType, NodeConfigTree, SegmentNode,
        };

        let tree = NodeConfigTree {
            node_id: "02.01.57.00.02.D9".into(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            profile_applied: false,
            unknown_variants: Vec::new(),
            segments: vec![SegmentNode {
                name: "Config".into(),
                description: None,
                origin: 0,
                space: 0xFD,
                children: vec![ConfigNode::Leaf(LeafNode {
                    name: "Event ID".into(),
                    description: None,
                    element_type: LeafType::EventId,
                    address: 0,
                    size: 8,
                    space: 0xFD,
                    path: vec!["seg:0".into(), "elem:0".into()],
                    value: Some(ConfigValue::EventId {
                        bytes: [1, 2, 3, 4, 5, 6, 7, 8],
                        hex: "01.02.03.04.05.06.07.08".into(),
                    }),
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
                })],
            }],
        };

        let proxy = crate::node_proxy::SynthesizedNodeProxy {
            node_key: "placeholder:populated-uuid".to_string(),
            profile_stem: "TestProfile".to_string(),
            snip: Some(lcc_rs::SNIPData {
                manufacturer: "Test".into(),
                model: "Node".into(),
                hardware_version: String::new(),
                software_version: "1.0".into(),
                user_name: String::new(),
                user_description: String::new(),
            }),
            cdi_data: Some(lcc_rs::CdiData {
                xml_content: "<cdi/>".into(),
                retrieved_at: chrono::Utc::now(),
            }),
            cdi_parsed: None,
            config_tree: Some(tree),
            producer_identified_events: Vec::new(),
        };
        let handle = crate::node_proxy::NodeProxyHandle::Synthesized(proxy);
        let state = AppState::new();

        let snap = build_node_snapshot(&handle, "2026-05-31T00:00:00Z", vec![], &state, None)
            .await
            .expect("build_node_snapshot");

        assert!(
            snap.missing.is_empty(),
            "populated tree must not surface 'missing value' entries, got: {:?}",
            snap.missing
        );
        assert_eq!(snap.capture_status, CaptureStatus::Complete);
        assert_eq!(snap.config.len(), 1, "one populated leaf must round-trip into config map");
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
        .map(|n| n.node_key.clone())
        .collect();

    let _ = crate::commands::bowties::set_recent_layout(recent_path.clone(), app.clone()).await;

    // Load offline changes into cache
    *state.offline_changes_cache.write().await = loaded.offline_changes.clone();

    // Build the per-node CDI XML + profile-annotated tree maps that feed
    // `LayoutState` (ADR-0015). Trees carry `event_role` annotations and
    // `profile_applied = true` so channel resolution works without a CDI
    // read (Spec 017 / S3 contract).
    let mut cdi_xml_by_key: std::collections::HashMap<crate::node_key::NodeKey, String> =
        std::collections::HashMap::new();
    let mut saved_trees_for_state: std::collections::HashMap<crate::node_key::NodeKey, crate::node_tree::NodeConfigTree> =
        std::collections::HashMap::new();
    {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Cannot resolve app data directory: {}", e))?;

        for snapshot in &loaded.node_snapshots {
            if snapshot.is_placeholder() {
                // Placeholders are excluded from the bowties catalog
                // (FR-015 — no binding enumeration for placeholder eventids).
                continue;
            }
            if snapshot.cdi_ref.fingerprint == "not_supported"
                || snapshot.cdi_ref.fingerprint == "missing"
            {
                continue;
            }
            let dotted_id = snapshot
                .node_id
                .as_ref()
                .expect("real-node snapshot has Some(node_id)")
                .to_hex_string();
            let nk = crate::node_key::NodeKey::from_node_id(
                *snapshot.node_id.as_ref().unwrap()
            );

            // Load CDI XML
            let xml = match crate::layout::resolve_cdi_xml_for_snapshot(
                input_path,
                snapshot,
                &app_data_dir,
            ) {
                Ok(xml) => xml,
                Err(_) => continue,
            };
            cdi_xml_by_key.insert(nk, xml.clone());

            // Parse CDI, build minimal tree, merge snapshot values, extract event ID leaves
            let cdi = match lcc_rs::cdi::parser::parse_cdi(&xml) {
                Ok(cdi) => cdi,
                Err(_) => continue,
            };
            let mut tree = crate::node_tree::build_node_config_tree(&dotted_id, &cdi);
            crate::node_tree::merge_snapshot_path_values(&mut tree, &snapshot.config);

            // Spec 017 / S3: apply profile annotations so the seeded saved tree
            // carries `event_role` on producer leaves + `profile_applied = true`.
            // Without this, `bowties_core::channel_events::resolve_channel_event_ids`
            // (which filters by `event_role == Some(Producer)`) returns empty for
            // every channel on a saved node, and indicators stay at 'no-config'
            // until the user forces a CDI read on the node. Mode selections come
            // from `loaded.bowties` directly because `state.active_layout` is not
            // yet set at this point in the open flow.
            if let Some(identity) = &cdi.identification {
                let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
                let model = identity.model.as_deref().unwrap_or("");
                if !manufacturer.is_empty() || !model.is_empty() {
                    if let Some(profile) = crate::profile::load_profile(
                        manufacturer,
                        model,
                        &cdi,
                        &app,
                        &state.profiles,
                    ).await {
                        let shared_daughterboards = crate::profile::load_shared_daughterboards(&app).await;
                        let selections = loaded.bowties.selections_for_node(&snapshot.node_key);
                        crate::commands::cdi::apply_profile_metadata_to_tree(
                            &mut tree,
                            &dotted_id,
                            &profile,
                            shared_daughterboards.as_ref(),
                            &cdi,
                            &selections,
                        );
                    }
                }
            }

            // Cache the fully-populated tree so it can seed the live proxy
            // when this node is rediscovered on the bus.
            saved_trees_for_state.insert(nk, tree);
        }
    }

    // Build the single-owner `LayoutState` from the same loaded data + per-node
    // CDI/tree maps assembled above. This is the in-memory home consulted by
    // the save flow's `proxy_snapshot_data` fallback and the offline catalog
    // rebuild (ADR-0015).
    {
        let layout_state = bowties_core::layout::state::LayoutState::from_loaded(
            input_path.to_path_buf(),
            loaded.clone(),
            cdi_xml_by_key,
            saved_trees_for_state,
        );
        *state.layout_state.write().await = Some(layout_state);
    }

    // S9: Reconstitute placeholder nodes into the registry so they're
    // available for tree queries and sidebar display during offline mode.
    for snapshot in &loaded.node_snapshots {
        if let Some(ref stem) = snapshot.profile_stem {
            match crate::placeholder::reconstitute(
                &snapshot.node_key,
                stem,
                &app,
                &state,
            )
            .await
            {
                Ok(proxy) => {
                    let parsed = match crate::node_key::NodeKey::parse(&snapshot.node_key) {
                        Ok(k) => k,
                        Err(e) => {
                            eprintln!(
                                "[layout open] invalid placeholder node_key {}: {}",
                                snapshot.node_key, e
                            );
                            continue;
                        }
                    };
                    state
                        .node_registry
                        .insert(
                            parsed,
                            crate::node_proxy::NodeProxyHandle::Synthesized(proxy),
                        )
                        .await;
                }
                Err(e) => {
                    eprintln!(
                        "[layout open] failed to reconstitute placeholder {}: {}",
                        snapshot.node_key, e
                    );
                }
            }
        }
    }

    let layout_node_keys: Vec<crate::node_key::NodeKey> = loaded
        .node_snapshots
        .iter()
        .filter_map(|s| crate::node_key::NodeKey::parse(&s.node_key).ok())
        .collect();
    let context = ActiveLayoutContext {
        layout_id: loaded.manifest.layout_id.clone(),
        root_path: recent_path.clone(),
        mode: ActiveLayoutMode::OfflineFile,
        captured_at: Some(loaded.manifest.captured_at.clone()),
        pending_offline_change_count: loaded.offline_changes.len(),
        layout_node_keys,
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
        load_warnings: loaded.load_warnings,
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
    *state.layout_state.write().await = None;
    state.node_registry.clear_layout_scope().await;
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
        layout_node_keys: Vec::new(),
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
        node_snapshots: save_result.node_snapshots,
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
                let selections = crate::commands::cdi::active_node_mode_selections(&state, &node_id).await;
                let report = crate::profile::annotate_tree(&mut tree, &profile, &selections, &cdi);
                let shared_daughterboards = crate::profile::load_shared_daughterboards(&app).await;
                let connector_profile_outcome = crate::profile::build_connector_profile_with_diagnostics(
                    &dotted_id,
                    &profile,
                    shared_daughterboards.as_ref(),
                    &cdi,
                );
                tree.connector_profile = connector_profile_outcome.profile;
                tree.connector_profile_warning = connector_profile_outcome.warning;
                tree.unknown_variants = report.unknown_variants.clone();
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
