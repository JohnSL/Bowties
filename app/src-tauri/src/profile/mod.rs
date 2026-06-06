//! Re-export `bowties_core::profile` so existing `crate::profile` paths
//! continue to resolve during the incremental migration.
//!
//! `loader` remains in this crate because it depends on `tauri::AppHandle`
//! for resource-directory resolution.

pub mod loader;

pub use bowties_core::profile::*;
pub use bowties_core::profile::types;
pub use bowties_core::profile::resolver;

pub use loader::{
    list_bundled_profiles, list_bundled_profiles_in_dirs, load_profile,
    load_shared_daughterboards, parse_profile_yaml, BundledProfileSummary,
};
