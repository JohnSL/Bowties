//! Tauri commands for logic compilation (Spec 020 / S1).

use bowties_core::logic_adapter::{
    compile_facility, get_capacity, CompiledLogicPlan, LogicCapacity,
};
use std::path::Path;

use crate::state::AppState;

/// Compile the logic for a facility on a target node.
///
/// Reads the facility's template from the registry and existing allocations
/// from the facilities document. Returns a `CompiledLogicPlan` with the
/// allocation record and CDI field writes for draft staging.
#[tauri::command]
pub async fn compile_logic_for_facility(
    facility_id: String,
    target_node_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<CompiledLogicPlan, String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Err("No layout is open".to_string());
    };
    if context.root_path.is_empty() {
        return Err("No layout is open".to_string());
    }
    let layout_dir = Path::new(&context.root_path);
    let facilities_doc: bowties_core::layout::facilities::FacilitiesDocument =
        bowties_core::layout::read_facilities(layout_dir)?;

    // Find the facility to get its template id.
    let facility = facilities_doc
        .facilities
        .iter()
        .find(|f| f.facility_id == facility_id)
        .ok_or_else(|| format!("Unknown facility: {facility_id}"))?;

    compile_facility(
        &facility.template_id,
        &facility_id,
        &target_node_key,
        &facilities_doc.logic_allocations,
    )
    .map_err(|e| e.to_string())
}

/// Query the logic capacity of a target node.
///
/// Returns the total and used conditional lines for the given node based
/// on existing allocations in the facilities document.
#[tauri::command]
pub async fn get_logic_capacity(
    target_node_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<LogicCapacity, String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Ok(LogicCapacity {
            total_lines: bowties_core::logic_adapter::MAX_CONDITIONAL_LINES,
            used_lines: 0,
        });
    };
    if context.root_path.is_empty() {
        return Ok(LogicCapacity {
            total_lines: bowties_core::logic_adapter::MAX_CONDITIONAL_LINES,
            used_lines: 0,
        });
    }
    let layout_dir = Path::new(&context.root_path);
    let facilities_doc: bowties_core::layout::facilities::FacilitiesDocument =
        bowties_core::layout::read_facilities(layout_dir)?;

    Ok(get_capacity(&target_node_key, &facilities_doc.logic_allocations))
}
