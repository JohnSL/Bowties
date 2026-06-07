//! Bowtie catalog types.
//!
//! These were originally in `app/src-tauri/src/state.rs` and have been moved
//! to bowties-core so the catalog builder can be unit-tested without Tauri.

use serde::{Deserialize, Serialize};

use crate::node_key::NodeKey;

/// Bowtie state reflecting current element membership.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BowtieState {
    Active,
    Incomplete,
    Planning,
}

/// A single classified event ID configuration field from one node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EventSlotEntry {
    /// Node key (canonical 12-hex for live nodes, placeholder:<uuid> for placeholders)
    pub node_key: NodeKey,
    /// Human-readable node name
    pub node_name: String,
    /// CDI element path from segment root
    pub element_path: Vec<String>,
    /// Full CDI <description> text for this slot (None when absent).
    /// Forwarded to the frontend so users can read the raw description when the
    /// role is Ambiguous and decide how to classify the slot.
    pub element_description: Option<String>,
    /// The 8-byte event ID stored in this slot
    pub event_id: [u8; 8],
    /// Classified role (only Producer or Consumer here; Ambiguous goes to ambiguous_entries)
    pub role: lcc_rs::EventRole,
}

/// One shared event ID with ≥1 confirmed producer and ≥1 confirmed consumer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BowtieCard {
    /// Dotted-hex event ID (unique key, default header)
    pub event_id_hex: String,
    /// Raw 8-byte event ID (for sorting/comparisons)
    pub event_id_bytes: [u8; 8],
    /// Confirmed producer slots (≥1)
    pub producers: Vec<EventSlotEntry>,
    /// Confirmed consumer slots (≥1)
    pub consumers: Vec<EventSlotEntry>,
    /// Slots whose role could not be determined
    pub ambiguous_entries: Vec<EventSlotEntry>,
    /// User-assigned name (None = unnamed, show event_id_hex as header)
    pub name: Option<String>,
    /// User-assigned tags from layout metadata
    #[serde(default)]
    pub tags: Vec<String>,
    /// Derived state based on element membership
    #[serde(default = "default_bowtie_state")]
    pub state: BowtieState,
}

fn default_bowtie_state() -> BowtieState {
    BowtieState::Active
}

/// Complete in-memory collection of discovered bowties for the current session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BowtieCatalog {
    /// Bowtie cards sorted by event_id_bytes
    pub bowties: Vec<BowtieCard>,
    /// ISO 8601 timestamp of when this catalog was built
    pub built_at: String,
    /// Number of nodes included in the catalog build
    pub source_node_count: usize,
    /// Total event slots scanned across all nodes
    pub total_slots_scanned: usize,
}
