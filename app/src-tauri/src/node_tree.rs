//! Re-export `bowties_core::node_tree` so existing `crate::node_tree` paths
//! continue to resolve during the incremental migration.

pub use bowties_core::node_tree::*;
