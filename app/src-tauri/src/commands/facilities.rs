//! Tauri commands for facility operations (spec 018).

use bowties_core::layout::facilities::{Facility, NodeLogicPlacement};
use serde::Serialize;
use bowties_core::layout::{panel_migration::PanelMigration, state::LayoutState};

use crate::state::AppState;

/// The facilities document as the frontend hydrates it: `logicPlacement` is
/// always present (the file omits it when empty).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilitiesListing {
    pub facilities: Vec<Facility>,
    pub logic_placement: Vec<NodeLogicPlacement>,
    pub panel_migration: Option<PanelMigration>,
}

impl From<&LayoutState> for FacilitiesListing {
    fn from(layout: &LayoutState) -> Self {
        Self {
            facilities: layout.facilities().facilities.clone(),
            logic_placement: layout.facilities().logic_placement.clone(),
            panel_migration: layout.panel_migration().cloned(),
        }
    }
}

/// List all facilities for the currently active layout.
///
/// Returns the canonical saved aggregate seeded by open, including conversion
/// metadata. Listing never re-reads legacy disk data or applies drafts.
///
/// CRUD is handled through the draft-layer on the frontend per ADR-0012; the
/// backend never sees individual create / rename / delete calls. Save flows
/// the `FacilitiesDocument`-targeted variants of `LayoutEditDelta` through
/// `save_layout_directory`.
#[tauri::command]
pub async fn list_facilities(
    state: tauri::State<'_, AppState>,
) -> Result<FacilitiesListing, String> {
    let layout = state.layout_state.read().await;
    Ok(layout.as_ref().map(FacilitiesListing::from).unwrap_or_default())
}

/// Read-only: the edits that return every slot a facility claims to its unused
/// state (kind-agnostic), the board placement without its Logic groups, and the
/// claims they came from. Writes nothing; the caller stages the edits.
#[tauri::command]
pub async fn compose_facility_removal(
    state: tauri::State<'_, AppState>,
    facility_id: String,
) -> Result<crate::signaling::compose::ComposeFacilityRemovalResponse, String> {
    crate::signaling::compose::compose_removal(state.inner(), &facility_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use bowties_core::layout::facilities::FacilitiesDocument;
    use bowties_core::layout::{LayoutDirectoryReadData, manifest::LayoutManifest, panel_migration::PanelMigration, state::LayoutState};

    #[test]
    fn listing_uses_canonical_saved_aggregate_and_propagates_conversion() {
        let outcome = PanelMigration { indicators: 4, panels: 2, warnings: vec!["Dropped malformed indicator".into()] };
        let loaded = LayoutDirectoryReadData {
            manifest: LayoutManifest::new("id".into(), "t".into(), "t".into()),
            node_snapshots: vec![], bowties: Default::default(), pending_changes: vec![],
            recovery_occurred: false, channels: Default::default(),
            facilities: FacilitiesDocument::new(vec![Facility {
                facility_id: "panel".into(), template_id: "panel".into(), name: "Fascia".into(),
                slot_bindings: Default::default(), block_signals: None, panel: None,
            }]),
            load_warnings: outcome.warnings.clone(), panel_migration: Some(outcome.clone()),
        };
        let state = LayoutState::from_loaded(Default::default(), loaded, Default::default(), Default::default());
        let listing = FacilitiesListing::from(&state);
        assert_eq!(listing.facilities, state.facilities().facilities);
        assert_eq!(listing.panel_migration, Some(outcome));
    }
}
