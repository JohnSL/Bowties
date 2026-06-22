//! Layout file types for YAML persistence.
//!
//! These types define the structure of user-managed `.bowties.yaml` layout files.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

/// Current schema version for layout files.
///
/// Bowties has not yet shipped a release whose layout files persist
/// daughterboard / placeholder / configuration-mode selections, so the
/// new field added in Spec 014 (`node_mode_selections`) is introduced
/// under the existing `"1.0"` schema rather than behind a version bump.
/// Old files load with the new field defaulting to empty; the now-removed
/// `connector_selections` and `placeholder_boards` fields are silently
/// dropped (placeholder boards moved to `NodeSnapshot` files per S8.5).
/// The next version bump waits until a real on-disk shape change requires it. The next
/// version bump waits until a real on-disk shape change requires it.
pub const SCHEMA_VERSION: &str = "1.0";

/// Reserved variant id meaning "this slot is intentionally empty" — used by
/// Configuration Modes that declare `allowNoneInstalled: true`.
pub const RESERVED_VARIANT_NONE: &str = "__none__";

// ── Connection configuration ──────────────────────────────────────────────────
//
// These types describe a saved connection (TCP / GridConnect serial / SLCAN
// serial). They live here in the layout module — rather than in
// `commands/connection.rs` — because they are persisted both in the global
// `connections.json` app preferences file AND, since Spec 013 / S4, inside
// the layout manifest under the `connections` field. Centralising the type
// here keeps a single serde shape across both persistence sites.
//
// `commands/connection.rs` re-exports these names so existing imports
// (`use crate::commands::ConnectionConfig;`) continue to work.

/// The transport/protocol variant for a connection.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AdapterType {
    /// Network hub (JMRI, standalone TCP/IP bridge). Default LCC port 12021.
    Tcp,
    /// GridConnect framing over USB serial.
    /// Compatible: RR-Cirkits Buffer LCC, SPROG USB-LCC, CAN2USBINO.
    GridConnectSerial,
    /// MERG CAN-RS / CANUSB4 GridConnect framing over USB serial.
    /// Uses non-standard header encoding where the 29-bit CAN ID is sent as
    /// `<11-bit SID><0><1><0><18-bit EID>`.
    MergGridConnectSerial,
    /// SLCAN (Lawicel) framing over USB serial.
    /// Compatible: Canable, Lawicel CANUSB, any slcand-compatible adapter.
    SlcanSerial,
}

/// Hardware flow control mode for serial connections.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum FlowControl {
    /// No hardware flow control (default for most adapters).
    #[default]
    None,
    /// RTS/CTS hardware flow control (required by SPROG USB-LCC / PI-LCC).
    RtsCts,
}

/// A saved connection configuration entry.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    /// Unique identifier (UUID v4)
    pub id: String,
    /// User-visible label for this connection
    pub name: String,
    /// Protocol / adapter type
    pub adapter_type: AdapterType,
    /// TCP hostname or IP (TCP only)
    pub host: Option<String>,
    /// TCP port number (TCP only, default 12021)
    pub port: Option<u16>,
    /// Serial port path, e.g. "COM3" or "/dev/ttyUSB0" (serial only)
    pub serial_port: Option<String>,
    /// Serial baud rate (serial only; USB CDC devices use this for host-side
    /// configuration, though the adapters themselves typically ignore it)
    pub baud_rate: Option<u32>,
    /// Hardware flow control mode (GridConnect serial only).
    /// Defaults to None when absent (backward-compatible with older saved configs).
    #[serde(default)]
    pub flow_control: FlowControl,
}

/// Root structure for the YAML layout file.
///
/// Example YAML:
/// ```yaml
/// schemaVersion: "1.0"
/// bowties:
///   "05.01.01.01.FF.00.00.01":
///     name: "Yard Entry Signal"
///     tags: ["yard", "signals"]
/// roleClassifications:
///   "05.02.01.02.03.00:Port I/O/Line #1/Event Produced":
///     role: "Producer"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFile {
    pub schema_version: String,
    #[serde(default)]
    pub bowties: BTreeMap<String, BowtieMetadata>,
    #[serde(default)]
    pub role_classifications: BTreeMap<String, RoleClassification>,
    /// Configuration-mode variant selections per node (Spec 014, ADR-0008).
    /// Outer key is a `NodeKey` (canonical NodeID or `placeholder:<uuidv4>`);
    /// inner key is the `ConfigurationMode` id.
    #[serde(default)]
    pub node_mode_selections: BTreeMap<String, BTreeMap<String, String>>,
}

impl Default for LayoutFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            bowties: BTreeMap::new(),
            role_classifications: BTreeMap::new(),
            node_mode_selections: BTreeMap::new(),
        }
    }
}

/// Metadata for a single bowtie, stored in layout YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BowtieMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// User-provided role classification for an ambiguous event slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleClassification {
    pub role: String,
}

/// A single edit operation to apply to a layout file.
///
/// The frontend sends a list of these deltas instead of a full `LayoutFile`.
/// The backend reads the current layout from disk, applies the deltas in order,
/// and writes the result (ADR-0002).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum LayoutEditDelta {
    /// Create a new bowtie entry (or no-op if it already exists).
    #[serde(rename_all = "camelCase")]
    CreateBowtie {
        event_id_hex: String,
        #[serde(default)]
        name: Option<String>,
    },
    /// Delete a bowtie entry.
    #[serde(rename_all = "camelCase")]
    DeleteBowtie { event_id_hex: String },
    /// Rename an existing bowtie (creates if absent).
    #[serde(rename_all = "camelCase")]
    RenameBowtie {
        event_id_hex: String,
        new_name: String,
    },
    /// Add a tag to a bowtie.
    #[serde(rename_all = "camelCase")]
    AddTag { event_id_hex: String, tag: String },
    /// Remove a tag from a bowtie.
    #[serde(rename_all = "camelCase")]
    RemoveTag { event_id_hex: String, tag: String },
    /// Classify an event slot's role.
    ClassifyRole { key: String, role: String },
    /// Upsert one Configuration Mode variant selection for a node.
    /// `node_key` may be a canonical NodeID or a `placeholder:<uuidv4>`.
    #[serde(rename_all = "camelCase")]
    SetNodeModeSelection {
        node_key: String,
        mode_id: String,
        variant_id: String,
    },
    /// Adopt a new event ID — move bowtie metadata from old key to new key.
    #[serde(rename_all = "camelCase")]
    AdoptEventId {
        old_event_id_hex: String,
        new_event_id_hex: String,
    },
    /// Promote a node into the layout's saved node roster (S8).
    ///
    /// `node_key` is a canonical NodeID (uppercase hex, no dots) for real
    /// nodes or `"placeholder:<uuid>"` for synthesized placeholders.
    ///
    /// `apply_layout_deltas` is a no-op for this variant — node membership
    /// is tracked by snapshot file presence in the companion `nodes/`
    /// directory, not inside `LayoutFile`. The save command interprets this
    /// delta as "include this node key in the permitted-write set".
    #[serde(rename_all = "camelCase")]
    AddNode { node_key: String },
    /// Remove a previously-persisted node from the layout's saved roster.
    ///
    /// Symmetric to `AddNode`. Like `AddNode`, this is a no-op for
    /// `apply_layout_deltas` — node membership lives in the companion
    /// `nodes/` directory. The save command interprets this delta as
    /// "drop this node key from the permitted-write set", which causes
    /// `write_layout_capture`'s nodes-dir prune step to delete the
    /// corresponding `<key>.yaml` file.
    #[serde(rename_all = "camelCase")]
    RemoveNode { node_key: String },
}

impl LayoutEditDelta {
    /// If this delta promotes a node (real or placeholder), return its
    /// `NodeKey` string. Otherwise return `None`.
    pub fn as_add_node(&self) -> Option<&str> {
        match self {
            LayoutEditDelta::AddNode { node_key } => Some(node_key.as_str()),
            _ => None,
        }
    }

    /// If this delta removes a previously-persisted node, return its
    /// `NodeKey` string. Otherwise return `None`.
    pub fn as_remove_node(&self) -> Option<&str> {
        match self {
            LayoutEditDelta::RemoveNode { node_key } => Some(node_key.as_str()),
            _ => None,
        }
    }
}

/// Apply a sequence of edit deltas to a layout file, mutating it in place.
pub fn apply_layout_deltas(layout: &mut LayoutFile, deltas: Vec<LayoutEditDelta>) {
    for delta in deltas {
        match delta {
            LayoutEditDelta::CreateBowtie {
                event_id_hex,
                name,
            } => {
                layout
                    .bowties
                    .entry(event_id_hex)
                    .or_insert_with(|| BowtieMetadata {
                        name,
                        tags: vec![],
                    });
            }
            LayoutEditDelta::DeleteBowtie { event_id_hex } => {
                layout.bowties.remove(&event_id_hex);
            }
            LayoutEditDelta::RenameBowtie {
                event_id_hex,
                new_name,
            } => {
                let entry =
                    layout
                        .bowties
                        .entry(event_id_hex)
                        .or_insert_with(|| BowtieMetadata {
                            name: None,
                            tags: vec![],
                        });
                entry.name = Some(new_name);
            }
            LayoutEditDelta::AddTag { event_id_hex, tag } => {
                if let Some(entry) = layout.bowties.get_mut(&event_id_hex) {
                    if !entry.tags.contains(&tag) {
                        entry.tags.push(tag);
                    }
                }
            }
            LayoutEditDelta::RemoveTag { event_id_hex, tag } => {
                if let Some(entry) = layout.bowties.get_mut(&event_id_hex) {
                    entry.tags.retain(|t| t != &tag);
                }
            }
            LayoutEditDelta::ClassifyRole { key, role } => {
                layout
                    .role_classifications
                    .insert(key, RoleClassification { role });
            }
            LayoutEditDelta::SetNodeModeSelection {
                node_key,
                mode_id,
                variant_id,
            } => {
                layout
                    .node_mode_selections
                    .entry(node_key)
                    .or_default()
                    .insert(mode_id, variant_id);
            }
            LayoutEditDelta::AdoptEventId {
                old_event_id_hex,
                new_event_id_hex,
            } => {
                if let Some(meta) = layout.bowties.remove(&old_event_id_hex) {
                    layout.bowties.insert(new_event_id_hex, meta);
                }
            }
            LayoutEditDelta::AddNode { .. } => {
                // Node membership lives outside LayoutFile (snapshot files in
                // the companion nodes/ dir). Handled in save_layout_directory.
            }
            LayoutEditDelta::RemoveNode { .. } => {
                // Same as AddNode — node membership lives outside LayoutFile.
                // Handled in save_layout_directory by subtracting from the
                // permitted-write set.
            }
        }
    }
}

/// Recent layout file reference, stored in app data dir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentLayout {
    pub path: String,
    pub last_opened: String,
}

/// Check if a string is a valid dotted-hex event ID (e.g. "05.01.01.01.FF.00.00.01").
fn is_valid_event_id_hex(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 8 {
        return false;
    }
    parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Check if a bowtie key is acceptable: either a valid dotted-hex event ID or
/// a planning placeholder of the form "planning-<digits>".
fn is_valid_bowtie_key(s: &str) -> bool {
    is_valid_event_id_hex(s)
        || (s.starts_with("planning-") && s[9..].chars().all(|c| c.is_ascii_digit()))
}

/// Validate a `placeholder:<uuidv4>` id (Spec 014).
///
/// Lives in `layout/types.rs` so commands, deltas, and load-time validation
/// share one rule. Returns the documented `InvalidPlaceholderId` error
/// message on rejection.
pub fn validate_placeholder_id(id: &str) -> Result<(), String> {
    let Some(rest) = id.strip_prefix("placeholder:") else {
        return Err(format!(
            "InvalidPlaceholderId: '{id}' must start with 'placeholder:'"
        ));
    };
    if !is_uuid_v4(rest) {
        return Err(format!(
            "InvalidPlaceholderId: '{id}' must be 'placeholder:<uuidv4>'"
        ));
    }
    Ok(())
}

/// Validate a `NodeKey` \u2014 a canonical 12-hex-char LCC NodeID OR
/// `placeholder:<uuidv4>` (Spec 014, ADR-0008).
pub fn validate_node_key(key: &str) -> Result<(), String> {
    if key.starts_with("placeholder:") {
        return validate_placeholder_id(key)
            .map_err(|e| e.replace("InvalidPlaceholderId", "InvalidNodeKey"));
    }
    if key.len() == 12 && key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(());
    }
    Err(format!(
        "InvalidNodeKey: '{key}' must be a 12-hex-char NodeID or 'placeholder:<uuidv4>'"
    ))
}

/// True if `node_key` refers to a placeholder board (binding-enumeration gate).
pub fn is_placeholder(node_key: &str) -> bool {
    node_key.starts_with("placeholder:")
}

/// Returns true when `s` is a lowercase UUID v4 (8-4-4-4-12, version nibble
/// `4`, variant nibble in `8`/`9`/`a`/`b`).
fn is_uuid_v4(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let dash_at = |i: usize| bytes[i] == b'-';
    if !(dash_at(8) && dash_at(13) && dash_at(18) && dash_at(23)) {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            continue;
        }
        if !b.is_ascii_hexdigit() || b.is_ascii_uppercase() {
            return false;
        }
    }
    // Version nibble (index 14) must be '4'.
    if bytes[14] != b'4' {
        return false;
    }
    // Variant nibble (index 19) must be 8/9/a/b.
    if !matches!(bytes[19], b'8' | b'9' | b'a' | b'b') {
        return false;
    }
    true
}

impl LayoutFile {
    /// Validate schema version and basic structure.
    /// Returns Ok(()) if valid, Err with description otherwise.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "Unsupported layout schema version: {} (expected {})",
                self.schema_version, SCHEMA_VERSION
            ));
        }

        // Validate bowtie keys match event ID hex format or planning placeholder format
        for key in self.bowties.keys() {
            if !is_valid_bowtie_key(key) {
                return Err(format!(
                    "Invalid bowtie key '{}': must be dotted hex (e.g. 05.01.01.01.FF.00.00.01) or a planning placeholder (e.g. planning-1234567890)",
                    key
                ));
            }
        }

        // Validate role classification values
        for (key, rc) in &self.role_classifications {
            if rc.role != "Producer" && rc.role != "Consumer" {
                return Err(format!(
                    "Invalid role '{}' for classification '{}': must be 'Producer' or 'Consumer'",
                    rc.role, key
                ));
            }
        }

        // Validate node_mode_selections keys.
        for key in self.node_mode_selections.keys() {
            validate_node_key(key)?;
        }

        Ok(())
    }

    /// Return the saved Configuration Mode selections for a single node.
    ///
    /// `node_key` may be a canonical 12-hex-char NodeID or
    /// `placeholder:<uuidv4>` (Spec 014, ADR-0008). Lookup is exact — the
    /// caller is responsible for passing a canonical key (the frontend
    /// normalizes via `normalizeNodeKey` before any IPC round-trip, and the
    /// `SetNodeModeSelection` delta applier likewise stores by the canonical
    /// form supplied by the command surface).
    ///
    /// Returns an empty map (not `None`) when the node has no selections so
    /// callers can `annotate_tree(&tree, &profile, &layout.selections_for_node(key), &cdi)`
    /// uniformly regardless of whether the user has chosen any variants.
    pub fn selections_for_node(&self, node_key: &str) -> std::collections::BTreeMap<String, String> {
        self.node_mode_selections
            .get(node_key)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_validates() {
        let layout = LayoutFile::default();
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn invalid_schema_version() {
        let layout = LayoutFile {
            schema_version: "3.0".to_string(),
            ..Default::default()
        };
        assert!(layout.validate().unwrap_err().contains("Unsupported"));
    }

    #[test]
    fn invalid_bowtie_key() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("not-hex".to_string(), BowtieMetadata {
            name: None,
            tags: vec![],
        });
        assert!(layout.validate().unwrap_err().contains("Invalid bowtie key"));
    }

    #[test]
    fn planning_bowtie_key_is_valid() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("planning-1774043332542".to_string(), BowtieMetadata {
            name: Some("My planning bowtie".to_string()),
            tags: vec![],
        });
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn invalid_role_classification() {
        let mut layout = LayoutFile::default();
        layout.role_classifications.insert(
            "05.02.01.02.03.00:path".to_string(),
            RoleClassification { role: "Both".to_string() },
        );
        assert!(layout.validate().unwrap_err().contains("Invalid role"));
    }

    #[test]
    fn valid_roundtrip() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert(
            "05.01.01.01.FF.00.00.01".to_string(),
            BowtieMetadata {
                name: Some("Test Bowtie".to_string()),
                tags: vec!["yard".to_string()],
            },
        );
        layout.role_classifications.insert(
            "05.02.01.02.03.00:Port/Line/Event".to_string(),
            RoleClassification { role: "Producer".to_string() },
        );
        assert!(layout.validate().is_ok());

        let yaml = serde_yaml_ng::to_string(&layout).unwrap();
        let parsed: LayoutFile = serde_yaml_ng::from_str(&yaml).unwrap();
        assert!(parsed.validate().is_ok());
        assert_eq!(parsed.bowties.len(), 1);
        assert_eq!(parsed.role_classifications.len(), 1);
    }

    #[test]
    fn connector_selection_roundtrip_removed_in_v2() {
        // Connector-selection persistence was removed in Spec 014. The
        // replacement seam is `node_mode_selections` + `placeholder_boards`,
        // exercised by the s3_* tests below.
    }

    #[test]
    fn connector_selection_preserves_unknown_records_removed_in_v2() {
        // See `connector_selection_roundtrip_removed_in_v2`.
    }

    // ── apply_layout_deltas tests (ADR-0002) ─────────────────────────────

    #[test]
    fn apply_deltas_create_bowtie() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Yard Entry".to_string()),
            },
        ]);
        assert_eq!(layout.bowties.len(), 1);
        let entry = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(entry.name.as_deref(), Some("Yard Entry"));
        assert!(entry.tags.is_empty());
    }

    #[test]
    fn apply_deltas_create_bowtie_noop_if_exists() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Original".to_string()),
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Overwrite Attempt".to_string()),
            },
        ]);
        // Should not overwrite
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().name.as_deref(), Some("Original"));
    }

    #[test]
    fn apply_deltas_delete_bowtie() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Doomed".to_string()),
            tags: vec![],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::DeleteBowtie { event_id_hex: "05.01.01.01.FF.00.00.01".to_string() },
        ]);
        assert!(layout.bowties.is_empty());
    }

    #[test]
    fn apply_deltas_rename_bowtie() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Old Name".to_string()),
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::RenameBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                new_name: "New Name".to_string(),
            },
        ]);
        let entry = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(entry.name.as_deref(), Some("New Name"));
        // Tags preserved
        assert_eq!(entry.tags, vec!["yard"]);
    }

    #[test]
    fn apply_deltas_rename_creates_if_absent() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::RenameBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                new_name: "Created via rename".to_string(),
            },
        ]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().name.as_deref(), Some("Created via rename"));
    }

    #[test]
    fn apply_deltas_add_and_remove_tag() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: None,
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::AddTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "signals".to_string(),
            },
            LayoutEditDelta::RemoveTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "yard".to_string(),
            },
        ]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().tags, vec!["signals"]);
    }

    #[test]
    fn apply_deltas_add_tag_deduplicates() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: None,
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::AddTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "yard".to_string(),
            },
        ]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().tags, vec!["yard"]);
    }

    #[test]
    fn apply_deltas_classify_role() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::ClassifyRole {
                key: "05.02.01.02.03.00:Port/Line/Event".to_string(),
                role: "Producer".to_string(),
            },
        ]);
        assert_eq!(layout.role_classifications.get("05.02.01.02.03.00:Port/Line/Event").unwrap().role, "Producer");
    }

    #[test]
    fn apply_deltas_set_connector_selection_removed_in_v2() {
        // The `SetConnectorSelection` delta variant was removed in Spec 014.
        // Configuration-mode selections (which subsumed it) are exercised by
        // `s3_placeholder_full_roundtrip_with_yaml` below.
    }

    #[test]
    fn apply_deltas_adopt_event_id() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("planning-123".to_string(), BowtieMetadata {
            name: Some("My Bowtie".to_string()),
            tags: vec!["yard".to_string()],
        });
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::AdoptEventId {
                old_event_id_hex: "planning-123".to_string(),
                new_event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
            },
        ]);
        assert!(!layout.bowties.contains_key("planning-123"));
        let moved = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(moved.name.as_deref(), Some("My Bowtie"));
        assert_eq!(moved.tags, vec!["yard"]);
    }

    #[test]
    fn apply_deltas_preserves_existing_roles_not_in_deltas() {
        let mut layout = LayoutFile::default();
        layout.role_classifications.insert(
            "existing:path".to_string(),
            RoleClassification { role: "Consumer".to_string() },
        );
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::ClassifyRole {
                key: "new:path".to_string(),
                role: "Producer".to_string(),
            },
        ]);
        // Existing role preserved
        assert_eq!(layout.role_classifications.get("existing:path").unwrap().role, "Consumer");
        // New role added
        assert_eq!(layout.role_classifications.get("new:path").unwrap().role, "Producer");
    }

    #[test]
    fn apply_deltas_multiple_operations_in_sequence() {
        let mut layout = LayoutFile::default();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Signal A".to_string()),
            },
            LayoutEditDelta::AddTag {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                tag: "yard".to_string(),
            },
            LayoutEditDelta::RenameBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                new_name: "Signal A (renamed)".to_string(),
            },
            LayoutEditDelta::ClassifyRole {
                key: "node:path".to_string(),
                role: "Producer".to_string(),
            },
        ]);
        let entry = layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap();
        assert_eq!(entry.name.as_deref(), Some("Signal A (renamed)"));
        assert_eq!(entry.tags, vec!["yard"]);
        assert_eq!(layout.role_classifications.len(), 1);
    }

    #[test]
    fn apply_deltas_empty_vec_is_noop() {
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Untouched".to_string()),
            tags: vec![],
        });
        apply_layout_deltas(&mut layout, vec![]);
        assert_eq!(layout.bowties.get("05.01.01.01.FF.00.00.01").unwrap().name.as_deref(), Some("Untouched"));
    }

    #[test]
    fn layout_edit_delta_json_roundtrip() {
        let deltas = vec![
            LayoutEditDelta::CreateBowtie {
                event_id_hex: "05.01.01.01.FF.00.00.01".to_string(),
                name: Some("Test".to_string()),
            },
            LayoutEditDelta::ClassifyRole {
                key: "node:path".to_string(),
                role: "Producer".to_string(),
            },
        ];
        let json = serde_json::to_string(&deltas).unwrap();
        let parsed: Vec<LayoutEditDelta> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
    }

    /// Verify that the exact JSON the frontend sends (camelCase, name omitted)
    /// deserializes correctly. This catches serde rename/default issues.
    #[test]
    fn layout_edit_delta_from_frontend_json() {
        // Frontend sends camelCase field names; name may be undefined (omitted)
        let json = r#"[
            {"type":"createBowtie","eventIdHex":"05.01.01.01.FF.00.00.01"},
            {"type":"createBowtie","eventIdHex":"05.01.01.01.FF.00.00.02","name":"Yard"},
            {"type":"createBowtie","eventIdHex":"05.01.01.01.FF.00.00.03","name":null},
            {"type":"deleteBowtie","eventIdHex":"05.01.01.01.FF.00.00.04"},
            {"type":"renameBowtie","eventIdHex":"05.01.01.01.FF.00.00.05","newName":"Signal B"},
            {"type":"addTag","eventIdHex":"05.01.01.01.FF.00.00.06","tag":"yard"},
            {"type":"removeTag","eventIdHex":"05.01.01.01.FF.00.00.07","tag":"yard"},
            {"type":"classifyRole","key":"node:path","role":"Producer"}
        ]"#;
        let parsed: Vec<LayoutEditDelta> = serde_json::from_str(json)
            .expect("frontend JSON should deserialize");
        assert_eq!(parsed.len(), 8);
        // name omitted → None
        match &parsed[0] {
            LayoutEditDelta::CreateBowtie { name, .. } => assert_eq!(*name, None),
            _ => panic!("expected CreateBowtie"),
        }
        // name present
        match &parsed[1] {
            LayoutEditDelta::CreateBowtie { name, .. } => assert_eq!(name.as_deref(), Some("Yard")),
            _ => panic!("expected CreateBowtie"),
        }
        // name: null → None
        match &parsed[2] {
            LayoutEditDelta::CreateBowtie { name, .. } => assert_eq!(*name, None),
            _ => panic!("expected CreateBowtie"),
        }
    }

    #[test]
    fn apply_deltas_add_node_is_noop_on_layout_file() {
        // S8: AddNode does not modify LayoutFile (node membership lives in
        // the companion nodes/ directory, not in bowties.yaml). It is
        // interpreted by save_layout_directory as a permitted-set hint.
        let mut layout = LayoutFile::default();
        layout.bowties.insert("05.01.01.01.FF.00.00.01".to_string(), BowtieMetadata {
            name: Some("Existing".to_string()),
            tags: vec!["yard".to_string()],
        });
        let bowties_before: Vec<String> = layout.bowties.keys().cloned().collect();
        let roles_before = layout.role_classifications.len();
        apply_layout_deltas(&mut layout, vec![
            LayoutEditDelta::AddNode { node_key: "020157000001".to_string() },
        ]);
        let bowties_after: Vec<String> = layout.bowties.keys().cloned().collect();
        assert_eq!(bowties_after, bowties_before);
        assert_eq!(layout.role_classifications.len(), roles_before);
        // Existing bowtie unchanged.
        let bw = layout.bowties.get("05.01.01.01.FF.00.00.01").expect("bowtie kept");
        assert_eq!(bw.name.as_deref(), Some("Existing"));
        assert_eq!(bw.tags, vec!["yard".to_string()]);
    }

    #[test]
    fn add_node_delta_as_add_node_returns_id() {
        let d = LayoutEditDelta::AddNode { node_key: "020157000001".to_string() };
        assert_eq!(d.as_add_node(), Some("020157000001"));

        let other = LayoutEditDelta::DeleteBowtie { event_id_hex: "05.01.01.01.FF.00.00.01".to_string() };
        assert_eq!(other.as_add_node(), None);
    }

    #[test]
    fn add_node_delta_from_frontend_json() {
        // S8.11: frontend now sends `nodeKey` instead of `nodeIdHex`.
        let json = r#"[{"type":"addNode","nodeKey":"020157000001"}]"#;
        let parsed: Vec<LayoutEditDelta> = serde_json::from_str(json)
            .expect("addNode JSON should deserialize");
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            LayoutEditDelta::AddNode { node_key } => assert_eq!(node_key, "020157000001"),
            _ => panic!("expected AddNode"),
        }
    }

    // ── S3: Layout file v2 — placeholders + nodeModeSelections ───────────

    const VALID_PLACEHOLDER_ID: &str =
        "placeholder:11111111-2222-4333-8444-555555555555";
    const ALT_PLACEHOLDER_ID: &str =
        "placeholder:aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

    #[test]
    fn s3_default_layout_has_empty_node_mode_selections() {
        let layout = LayoutFile::default();
        assert_eq!(layout.schema_version, SCHEMA_VERSION);
        assert!(layout.node_mode_selections.is_empty());
        assert!(layout.validate().is_ok());
    }

    #[test]
    fn s8_5_add_placeholder_board_is_noop_on_layout_file() {
        // Per S8.11: placeholders and real nodes both use AddNode. The
        // delta is a no-op on LayoutFile; the save command builds the
        // snapshot from the registry.
        let mut layout = LayoutFile::default();
        apply_layout_deltas(
            &mut layout,
            vec![LayoutEditDelta::AddNode {
                node_key: VALID_PLACEHOLDER_ID.to_string(),
            }],
        );
        // LayoutFile body is unchanged.
        assert!(layout.bowties.is_empty());
        assert!(layout.node_mode_selections.is_empty());
    }

    #[test]
    fn s8_11_add_node_accepts_placeholder_key() {
        let d = LayoutEditDelta::AddNode {
            node_key: VALID_PLACEHOLDER_ID.to_string(),
        };
        assert_eq!(d.as_add_node(), Some(VALID_PLACEHOLDER_ID));
        let real = LayoutEditDelta::AddNode { node_key: "020157000001".to_string() };
        assert_eq!(real.as_add_node(), Some("020157000001"));
    }

    #[test]
    fn s3_validate_placeholder_id_accepts_v4_uuid() {
        assert!(validate_placeholder_id(VALID_PLACEHOLDER_ID).is_ok());
        assert!(validate_placeholder_id(ALT_PLACEHOLDER_ID).is_ok());
    }

    // ── S6: selections_for_node accessor ──────────────────────────────────
    #[test]
    fn s6_selections_for_node_returns_saved_entries() {
        let mut layout = LayoutFile::default();
        let mut entries = BTreeMap::new();
        entries.insert("connector-a".to_string(), "BOD4-CP".to_string());
        entries.insert("connector-b".to_string(), "BOD-8".to_string());
        layout
            .node_mode_selections
            .insert("020157000001".to_string(), entries);

        let got = layout.selections_for_node("020157000001");
        assert_eq!(got.len(), 2);
        assert_eq!(got.get("connector-a").map(String::as_str), Some("BOD4-CP"));
        assert_eq!(got.get("connector-b").map(String::as_str), Some("BOD-8"));
    }

    #[test]
    fn s6_selections_for_node_returns_empty_when_missing() {
        let layout = LayoutFile::default();
        assert!(layout.selections_for_node("020157000001").is_empty());
        assert!(layout.selections_for_node(VALID_PLACEHOLDER_ID).is_empty());
    }

    #[test]
    fn s3_validate_placeholder_id_rejects_malformed() {
        // Missing prefix.
        assert!(validate_placeholder_id("11111111-2222-4333-8444-555555555555").is_err());
        // Wrong UUID version (v1, not v4).
        assert!(
            validate_placeholder_id("placeholder:11111111-2222-1333-8444-555555555555").is_err()
        );
        // Wrong variant nibble.
        assert!(
            validate_placeholder_id("placeholder:11111111-2222-4333-7444-555555555555").is_err()
        );
        // Empty.
        assert!(validate_placeholder_id("placeholder:").is_err());
        // Non-hex characters.
        assert!(
            validate_placeholder_id("placeholder:zzzzzzzz-2222-4333-8444-555555555555").is_err()
        );
    }

    #[test]
    fn s3_validate_node_key_accepts_canonical_node_id_and_placeholder() {
        assert!(validate_node_key("020157000001").is_ok());
        assert!(validate_node_key(VALID_PLACEHOLDER_ID).is_ok());
    }

    #[test]
    fn s3_validate_node_key_rejects_malformed() {
        assert!(validate_node_key("not-a-node").is_err());
        assert!(validate_node_key("placeholder:not-a-uuid").is_err());
    }
}
