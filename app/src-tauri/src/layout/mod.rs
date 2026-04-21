//! Layout file persistence for bowtie metadata.
//!
//! Manages loading and saving user-managed YAML layout files that store
//! bowtie metadata (names, tags) and role classifications for ambiguous
//! event slots.

pub mod types;
pub mod io;
pub mod manifest;
pub mod node_snapshot;
pub mod offline_changes;
pub mod serde_node_id;
