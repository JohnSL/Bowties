//! Tauri commands for LCC operations
//!
//! This module organizes all Tauri commands by functional area.

pub mod discovery;
pub mod cdi;
pub mod bowties;
pub mod behavior_templates;
pub mod channel_events;
pub mod channels;
pub mod connector_profiles;
pub mod connection;
pub mod facilities;
pub mod facility_bowties;
pub mod layout_capture;
pub mod layout_drafts;
pub mod placeholders;
pub mod startup;
pub mod sync_panel;

// Re-export all commands for easy registration
pub use discovery::*;
pub use cdi::*;
pub use bowties::*;
pub use behavior_templates::*;
pub use channels::*;
pub use connector_profiles::*;
pub use connection::*;
pub use facilities::*;
pub use facility_bowties::*;
pub use layout_capture::*;
pub use layout_drafts::*;
pub use placeholders::*;
pub use startup::*;
pub use sync_panel::*;
