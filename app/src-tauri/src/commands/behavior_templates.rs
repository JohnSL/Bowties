//! Tauri commands for the behavior template registry (spec 018).

use bowties_core::behavior_templates::{registered_templates, BehaviorTemplate};

/// List every registered behavior template.
///
/// The registry is code-level (hardcoded in `bowties-core::behavior_templates`),
/// so this command is stateless and infallible. Called once on app start by the
/// frontend `behaviorTemplatesStore`.
#[tauri::command]
pub fn list_behavior_templates() -> Vec<BehaviorTemplate> {
    registered_templates().to_vec()
}
