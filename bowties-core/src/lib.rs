//! Bowties core domain logic.
//!
//! Pure business logic for the Bowties LCC configuration tool, free of any
//! Tauri or UI framework dependency.  Extracted so that domain modules can be
//! unit-tested with plain `cargo test`.

pub mod bowtie;
pub mod node_key;
pub mod layout;
pub mod node_tree;
pub mod node_proxy;
pub mod node_registry;
pub mod placeholder;
pub mod profile;
pub mod sync;
