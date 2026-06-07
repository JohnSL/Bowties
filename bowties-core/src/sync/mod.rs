//! Sync domain logic — pure business logic for the offline sync panel.
//!
//! Extracted from `app/src-tauri/src/commands/sync_panel.rs` so that scoring,
//! classification, CDI field resolution, and change-set helpers can be
//! unit-tested with `cargo test` (no Tauri DLL dependency).

pub mod changes;
pub mod classifier;
pub mod field_meta;
