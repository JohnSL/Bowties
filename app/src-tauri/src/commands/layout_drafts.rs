//! Tauri commands for LayoutState draft-layer sync (Spec 018 / S6 bugfix).
//!
//! Bridges the frontend's collected `LayoutEditDelta`s into
//! `LayoutState.drafts` so backend read paths (today: facility bowtie
//! composition) can observe frontend-side pending edits before save.
//!
//! See ADR-0015 (`0015-backend-layout-state-single-owner.md`) — the
//! drafts layer was designed for exactly this seam; S6 activated it.
//!
//! Contract:
//! * `sync_layout_drafts(deltas, configDrafts)` — the frontend sends its
//!   *complete* current delta set (matches `collectDeltas()` semantics) and
//!   its complete unsaved configuration edits; the backend clones the saved
//!   documents and applies the deltas into the drafts layer, and keeps the
//!   config drafts for `LayoutState::effective_config_tree`. Idempotent
//!   w.r.t. any given input. `configDrafts` may be omitted (empty).
//! * `clear_layout_drafts()` — called on Discard and after Save (the
//!   save flow also folds drafts into the saved layer as part of the
//!   same write).

use crate::state::AppState;

#[tauri::command]
pub async fn sync_layout_drafts(
    deltas: Vec<crate::layout::types::LayoutEditDelta>,
    config_drafts: Option<Vec<bowties_core::signaling::slots::DraftEdit>>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Pending offline Config rows live in the app cache; mirror them so the
    // effective configuration view lays them under the drafts.
    let offline_changes = state.offline_changes_cache.read().await.clone();
    let mut layout_guard = state.layout_state.write().await;
    let layout_state = layout_guard
        .as_mut()
        .ok_or_else(|| "no layout is open".to_string())?;
    layout_state
        .sync_drafts(&deltas)
        .map_err(|e| e.to_string())?;
    layout_state.sync_config_drafts(config_drafts.unwrap_or_default());
    layout_state.set_offline_changes(offline_changes);
    Ok(())
}

#[tauri::command]
pub async fn clear_layout_drafts(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut layout_guard = state.layout_state.write().await;
    if let Some(layout_state) = layout_guard.as_mut() {
        layout_state.clear_drafts();
    }
    Ok(())
}
