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
use std::path::Path;

/// List all information channels for the currently active layout.
///
/// Reads `channels.yaml` from the active layout directory via the
/// intent-shaped `read_channels` API (ADR-0005). Returns an empty vec
/// when no layout is open or the file is missing (pre-015 layouts).
#[tauri::command]
pub async fn list_channels(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<InformationChannel>, String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Ok(Vec::new());
    };
    if context.root_path.is_empty() {
        return Ok(Vec::new());
    }
    let layout_dir = Path::new(&context.root_path);
    let doc = bowties_core::layout::read_channels(layout_dir)?;
    Ok(doc.channels)
}
