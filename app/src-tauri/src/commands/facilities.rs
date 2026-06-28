//! Tauri commands for facility operations (spec 018).

use bowties_core::layout::facilities::{Facility, FacilitiesDocument};
use std::path::Path;

use crate::state::AppState;

/// List all facilities for the currently active layout.
///
/// Reads `facilities.yaml` via the intent-shaped `read_facilities` API.
/// Returns an empty vec when no layout is open or the file is missing
/// (pre-018 layouts).
///
/// CRUD is handled through the draft-layer on the frontend per ADR-0012; the
/// backend never sees individual create / rename / delete calls. Save flows
/// the `FacilitiesDocument`-targeted variants of `LayoutEditDelta` through
/// `save_layout_directory`.
#[tauri::command]
pub async fn list_facilities(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Facility>, String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Ok(Vec::new());
    };
    if context.root_path.is_empty() {
        return Ok(Vec::new());
    }
    let layout_dir = Path::new(&context.root_path);
    let doc: FacilitiesDocument = bowties_core::layout::read_facilities(layout_dir)?;
    Ok(doc.facilities)
}
