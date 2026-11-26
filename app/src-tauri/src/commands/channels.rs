//! Tauri commands for information channel operations.
//!
//! Since the atomic-save fold (ADR-0002 follow-up), all channel mutations —
//! create, rename, delete — travel as `LayoutEditDelta`s through
//! `save_layout_directory`. This module retains only the read-side
//! `list_channels` IPC; the write commands were removed to eliminate the
//! parallel post-save write path that was silently dropping user-owned
//! channel drafts.

use bowties_core::layout::channels::InformationChannel;
use crate::state::AppState;

/// List all information channels for the currently active layout.
///
/// Uses the canonical saved aggregate, including migrated lamp names.
#[tauri::command]
pub async fn list_channels(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<InformationChannel>, String> {
    let layout = state.layout_state.read().await;
    Ok(layout.as_ref().map(|l| l.channels().channels.clone()).unwrap_or_default())
}
