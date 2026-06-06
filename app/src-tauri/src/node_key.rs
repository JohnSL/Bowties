//! Re-export `bowties_core::node_key` so existing `crate::node_key` paths
//! continue to resolve during the incremental migration.

pub use bowties_core::node_key::*;
