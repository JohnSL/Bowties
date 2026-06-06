//! Tauri commands for the placeholder picker, unified `node_mode_selections`,
//! and the bundled-CDI fetch IPC used by the placeholder add-flow.
//!
//! S8.5 reframed placeholders as `NodeSnapshot` files in the companion
//! `nodes/` directory rather than a parallel `LayoutFile.placeholder_boards`
//! shape, and the previous per-edit IPCs
//! (`add_placeholder_board`, `delete_placeholder_board`,
//! `set_placeholder_config_value`, `rename_placeholder_board`) were deleted:
//! their writes violated ADR-0002 by flushing the layout file on every edit,
//! and they bypassed the in-memory snapshot pipeline that already exists for
//! real-node discovery. Placeholders now mutate only frontend in-memory state
//! until Save, at which point the `AddPlaceholderBoard` delta carries the
//! synthesized snapshot fields through the existing `save_layout_directory`
//! path.

use crate::layout::types::{is_placeholder, validate_node_key, LayoutEditDelta};
use crate::profile::{list_bundled_profiles, BundledProfileSummary};
use crate::state::{ActiveLayoutMode, AppState};

use super::layout_capture::{save_layout_directory, SaveLayoutResult};

async fn active_offline_layout_path(state: &tauri::State<'_, AppState>) -> Result<String, String> {
    let active = state.active_layout.read().await.clone();
    let ctx = active
        .filter(|c| c.mode == ActiveLayoutMode::OfflineFile)
        .ok_or_else(|| "No offline layout is active".to_string())?;
    if ctx.root_path.is_empty() {
        return Err("Active layout has no on-disk path yet".to_string());
    }
    Ok(ctx.root_path)
}

async fn apply_single_delta(
    delta: LayoutEditDelta,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SaveLayoutResult, String> {
    let path = active_offline_layout_path(&state).await?;
    save_layout_directory(path, true, vec![delta], app, state).await
}

#[tauri::command]
pub async fn set_node_mode_selection(
    node_key: String,
    mode_id: String,
    variant_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SaveLayoutResult, String> {
    validate_node_key(&node_key)?;
    if mode_id.trim().is_empty() {
        return Err("EmptyModeId".to_string());
    }
    if variant_id.trim().is_empty() {
        return Err("EmptyVariantId".to_string());
    }
    // If the caller targets a placeholder key, ensure it parses as one even
    // though `validate_node_key` already accepts both shapes — this keeps the
    // error message specific when the prefix is present but malformed.
    if node_key.starts_with("placeholder:") {
        debug_assert!(is_placeholder(&node_key));
    }
    apply_single_delta(
        LayoutEditDelta::SetNodeModeSelection {
            node_key,
            mode_id,
            variant_id,
        },
        app,
        state,
    )
    .await
}

/// Return picker-ready summaries of every bundled board-model profile.
///
/// IPC entry point for the "Add board" placeholder picker (Spec 014 / S8).
/// Scans the same `profiles/` search directories used by the profile loader
/// and the bundled-CDI loader, parses each `*.profile.yaml`'s `nodeType`
/// block, and returns `{stem, manufacturer, model}` triples sorted by
/// `(manufacturer, model)`. Malformed entries are silently skipped — listing
/// must never fail because one bundle entry is broken.
#[tauri::command]
pub fn list_bundled_profiles_command(
    app: tauri::AppHandle,
) -> Vec<BundledProfileSummary> {
    list_bundled_profiles(&app)
}

/// Add a placeholder board by synthesizing it from a bundled profile.
///
/// Calls the placeholder factory to mint a `placeholder:<uuid>` key, load
/// the bundled CDI, walk EventId leaves, build the config tree, and produce
/// a `SynthesizedNodeProxy`. The proxy is inserted into the node registry
/// so that subsequent `get_node_tree` / `get_snip` / save-flow calls
/// dispatch through `NodeProxyHandle` uniformly.
///
/// Returns the minted `node_key` so the frontend can route to it.
///
/// Spec 014 / S8.10.
#[tauri::command]
pub async fn add_placeholder_board(
    profile_stem: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AddPlaceholderResult, String> {
    if profile_stem.trim().is_empty() {
        return Err("EmptyProfileStem".to_string());
    }
    let (node_key, proxy) =
        crate::placeholder::synthesize(&profile_stem, &app, &state).await?;
    let parsed = crate::node_key::NodeKey::parse(&node_key)
        .map_err(|e| format!("InternalError: synthesized invalid NodeKey '{}': {}", node_key, e))?;
    state
        .node_registry
        .insert(
            parsed,
            crate::node_proxy::NodeProxyHandle::Synthesized(proxy),
        )
        .await;
    Ok(AddPlaceholderResult { node_key })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPlaceholderResult {
    pub node_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_node_mode_selection_validates_both_node_key_shapes() {
        assert!(validate_node_key("050101010001").is_ok());
        assert!(
            validate_node_key("placeholder:11111111-2222-4333-8444-555555555555").is_ok()
        );
        assert!(validate_node_key("garbage").is_err());
    }
}
