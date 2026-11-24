//! Tauri commands for facility operations (spec 018).

use bowties_core::layout::facilities::{Facility, FacilitiesDocument, NodeLogicPlacement};
use serde::Serialize;
use std::path::Path;

use crate::state::AppState;

/// The facilities document as the frontend hydrates it: `logicPlacement` is
/// always present (the file omits it when empty).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilitiesListing {
    pub facilities: Vec<Facility>,
    pub logic_placement: Vec<NodeLogicPlacement>,
}

/// List all facilities for the currently active layout.
///
/// Reads `facilities.yaml` via the intent-shaped `read_facilities` API and
/// returns the whole document (`facilities` and per-board `logicPlacement`,
/// one read of one snapshot). Returns an empty document when no layout is
/// open or the file is missing (pre-018 layouts).
///
/// CRUD is handled through the draft-layer on the frontend per ADR-0012; the
/// backend never sees individual create / rename / delete calls. Save flows
/// the `FacilitiesDocument`-targeted variants of `LayoutEditDelta` through
/// `save_layout_directory`.
#[tauri::command]
pub async fn list_facilities(
    state: tauri::State<'_, AppState>,
) -> Result<FacilitiesListing, String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Ok(FacilitiesListing::default());
    };
    if context.root_path.is_empty() {
        return Ok(FacilitiesListing::default());
    }
    let layout_dir = Path::new(&context.root_path);
    let doc: FacilitiesDocument = bowties_core::layout::read_facilities(layout_dir)?;
    Ok(FacilitiesListing { facilities: doc.facilities, logic_placement: doc.logic_placement })
}
