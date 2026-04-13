//! Tauri commands for LCC operations
//!
//! This module organizes all Tauri commands by functional area.

pub mod discovery;
pub mod cdi;
pub mod bowties;
pub mod connection;
pub mod layout_capture;
pub mod sync_panel;

// Re-export all commands for easy registration
pub use discovery::*;
pub use cdi::*;
pub use bowties::*;
pub use connection::*;
pub use layout_capture::*;
pub use sync_panel::*;
