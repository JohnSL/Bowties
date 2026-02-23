//! Tauri commands for LCC operations
//!
//! This module organizes all Tauri commands by functional area.

pub mod discovery;
pub mod cdi;
pub mod bowties;

// Re-export all commands for easy registration
pub use discovery::*;
pub use cdi::*;
pub use bowties::*;
