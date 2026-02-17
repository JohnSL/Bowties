//! Tauri commands for LCC operations
//!
//! This module organizes all Tauri commands by functional area.

pub mod discovery;

// Re-export all commands for easy registration
pub use discovery::*;
