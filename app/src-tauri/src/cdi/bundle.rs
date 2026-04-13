//! CDI import/export command skeletons.
//!
//! Phase 1 scaffolding only. Implementations are added in later phases.

#[tauri::command]
pub async fn export_cdi_bundle(output_path: String) -> Result<(), String> {
    let _ = output_path;
    Err("export_cdi_bundle is not implemented yet".to_string())
}

#[tauri::command]
pub async fn import_cdi_bundle(bundle_path: String) -> Result<(), String> {
    let _ = bundle_path;
    Err("import_cdi_bundle is not implemented yet".to_string())
}
