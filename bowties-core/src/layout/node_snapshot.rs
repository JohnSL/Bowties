//! Node snapshot types for layout directory persistence.

use std::collections::BTreeMap;
use lcc_rs::NodeID;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLeafValue {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SnapshotValueNode {
    Branch(BTreeMap<String, SnapshotValueNode>),
    Leaf(SnapshotLeafValue),
}

#[derive(Debug, Clone)]
pub struct SnapshotValueEntry {
    pub path: Vec<String>,
    pub leaf: SnapshotLeafValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiReference {
    pub cache_key: String,
    pub version: String,
    pub fingerprint: String,
}

/// S8.6: marker stored in `CdiReference::fingerprint` and `version` for
/// placeholder snapshots synthesized from a bundled profile. Centralizing
/// the sentinel makes `is_bundled()` self-evident and avoids stringly
/// branching scattered across the codebase.
pub const BUNDLED_CDI_MARKER: &str = "bundled";

/// Sanitize a single component for use as a CDI cache filename fragment.
/// Allows alphanumerics, `-`, and `_`; replaces everything else with `_`.
/// This rule has matched the on-disk `cdi_cache/` filenames since v0.x —
/// `CdiReference::from_snip` uses it so the stored `cache_key` resolves
/// directly to the historical filename without a parallel derivation.
fn sanitize_cache_fragment(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

impl CdiReference {
    /// S8.6: single mint formula for live-node `CdiReference`. The stored
    /// `cache_key` IS the filename basis used by `cdi_cache_path` and by
    /// the layout-companion `cdi/` directory — no parallel derivation
    /// elsewhere.
    pub fn from_snip(snip: &SnipSnapshot, version: impl Into<String>, fingerprint: impl Into<String>) -> Self {
        let version = version.into();
        let cache_key = format!(
            "{}_{}_{}",
            sanitize_cache_fragment(&snip.manufacturer_name),
            sanitize_cache_fragment(&snip.model_name),
            sanitize_cache_fragment(&version),
        );
        Self {
            cache_key,
            version,
            fingerprint: fingerprint.into(),
        }
    }

    /// S8.6: single mint formula for placeholder `CdiReference`. The
    /// `cache_key` is the profile stem verbatim (stems are already
    /// filename-safe per the bundle layout); `version` and `fingerprint`
    /// are both `"bundled"` so `is_bundled()` short-circuits cdi_cache
    /// lookups onto the bundled profiles resource directory.
    pub fn from_profile_stem(profile_stem: impl Into<String>) -> Self {
        Self {
            cache_key: profile_stem.into(),
            version: BUNDLED_CDI_MARKER.to_string(),
            fingerprint: BUNDLED_CDI_MARKER.to_string(),
        }
    }

    /// `true` when this CDI was synthesized from a bundled profile stem.
    /// Save flushes source the file from the bundled `profiles/` resource
    /// directory rather than `cdi_cache/`.
    pub fn is_bundled(&self) -> bool {
        self.fingerprint == BUNDLED_CDI_MARKER
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnipSnapshot {
    pub user_name: String,
    pub user_description: String,
    pub manufacturer_name: String,
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Complete,
    Partial,
}

/// Authoritative identity for a layout snapshot entry (Spec 014 / S8.5,
/// ADR-0008). For a real bus node this is the canonical dotted hex form
/// of its `NodeID` (e.g. `"05.01.01.01.03.01"`); for a placeholder board
/// it is `"placeholder:<uuidv4>"`. The string-typed surface lets the same
/// store and editor pipeline carry both kinds without wrapping/unwrapping.
pub const PLACEHOLDER_KEY_PREFIX: &str = "placeholder:";

/// Returns `true` when the given key identifies a placeholder board.
pub fn is_placeholder_key(node_key: &str) -> bool {
    node_key.starts_with(PLACEHOLDER_KEY_PREFIX)
}

/// Derive the filesystem-safe filename basis for a snapshot key.
///
/// Keys that contain characters illegal on Windows filesystems (`:`) have
/// those characters replaced with `_`. The in-memory and YAML `node_key`
/// retains the original form; only the on-disk filename uses the escaped
/// form.
pub fn filename_basis_for_key(node_key: &str) -> String {
    node_key.replace(':', "_")
}

/// Runtime lifecycle state for a `NodeSnapshot`. Not persisted — on disk the
/// snapshot is tautologically `Persisted`. Only the placeholder factory and
/// save path use `InMemory` to track snapshots that exist only in the
/// frontend stores and haven't been flushed to disk yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeSnapshotLifecycle {
    /// The snapshot has been read from or written to disk.
    Persisted,
    /// The snapshot exists only in memory (e.g. freshly-synthesized placeholder).
    InMemory,
}

impl Default for NodeSnapshotLifecycle {
    fn default() -> Self {
        Self::Persisted
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSnapshot {
    /// Authoritative identity for this snapshot.
    pub node_key: String,
    /// `Some` for real bus nodes, `None` for placeholder boards.
    #[serde(
        with = "crate::layout::serde_node_id::canonical_option",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub node_id: Option<NodeID>,
    /// Profile stem that sourced this snapshot's CDI (e.g.
    /// `"Mustangpeak-Engineering_TurnoutBoss"`). `Some` for placeholders
    /// (bundled CDI), `None` for real nodes (CDI read from device).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_stem: Option<String>,
    /// Runtime lifecycle — not serialized to disk. Defaults to `Persisted`
    /// on deserialize; the factory sets `InMemory` at creation time.
    #[serde(skip)]
    pub lifecycle: NodeSnapshotLifecycle,
    pub captured_at: String,
    pub capture_status: CaptureStatus,
    #[serde(default)]
    pub missing: Vec<String>,
    pub snip: SnipSnapshot,
    pub cdi_ref: CdiReference,
    /// Canonical path-centric value tree keyed by CDI element hierarchy.
    #[serde(default)]
    pub config: BTreeMap<String, SnapshotValueNode>,
    #[serde(default)]
    pub producer_identified_events: Vec<String>,
}

/// On-disk shape used during deserialization so we can backfill `node_key`
/// from a legacy snapshot that only carries `nodeId`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeSnapshotRepr {
    #[serde(default)]
    node_key: Option<String>,
    #[serde(
        with = "crate::layout::serde_node_id::canonical_option",
        default
    )]
    node_id: Option<NodeID>,
    #[serde(default)]
    profile_stem: Option<String>,
    captured_at: String,
    capture_status: CaptureStatus,
    #[serde(default)]
    missing: Vec<String>,
    snip: SnipSnapshot,
    cdi_ref: CdiReference,
    #[serde(default)]
    config: BTreeMap<String, SnapshotValueNode>,
    #[serde(default)]
    producer_identified_events: Vec<String>,
}

impl<'de> Deserialize<'de> for NodeSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let repr = NodeSnapshotRepr::deserialize(deserializer)?;
        let node_key = match (repr.node_key, &repr.node_id) {
            (Some(k), _) => k,
            (None, Some(id)) => id.to_canonical(),
            (None, None) => {
                return Err(serde::de::Error::custom(
                    "NodeSnapshot is missing both nodeKey and nodeId",
                ));
            }
        };
        Ok(NodeSnapshot {
            node_key,
            node_id: repr.node_id,
            profile_stem: repr.profile_stem,
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: repr.captured_at,
            capture_status: repr.capture_status,
            missing: repr.missing,
            snip: repr.snip,
            cdi_ref: repr.cdi_ref,
            config: repr.config,
            producer_identified_events: repr.producer_identified_events,
        })
    }
}

impl NodeSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.capture_status == CaptureStatus::Partial && self.missing.is_empty() {
            return Err("partial snapshots must include missing details".to_string());
        }
        if self.node_key.is_empty() {
            return Err("NodeSnapshot.node_key must not be empty".to_string());
        }
        // Typed invariant: real nodes must have a NodeID; placeholders must
        // have a profile_stem. No `is_placeholder_key` sniffing.
        if self.node_id.is_none() && self.profile_stem.is_none() {
            return Err(format!(
                "NodeSnapshot with node_key '{}' has neither node_id nor profile_stem",
                self.node_key
            ));
        }
        Ok(())
    }

    /// `true` when this snapshot represents a placeholder board (no live bus node).
    pub fn is_placeholder(&self) -> bool {
        self.node_id.is_none()
    }

    /// Filesystem-safe filename basis derived from `node_key`.
    pub fn filename_basis(&self) -> String {
        filename_basis_for_key(&self.node_key)
    }

    pub fn add_config_leaf(&mut self, path: &[String], leaf: SnapshotLeafValue) {
        insert_config_leaf(&mut self.config, path, leaf);
    }

    pub fn flattened_config_entries(&self) -> Vec<SnapshotValueEntry> {
        let mut out = Vec::new();
        flatten_config(&self.config, &mut Vec::new(), &mut out);
        out
    }
}

pub fn insert_config_leaf(
    root: &mut BTreeMap<String, SnapshotValueNode>,
    path: &[String],
    leaf: SnapshotLeafValue,
) {
    if path.is_empty() {
        return;
    }

    let key = path[0].clone();
    if path.len() == 1 {
        root.insert(key, SnapshotValueNode::Leaf(leaf));
        return;
    }

    let child = root
        .entry(key)
        .or_insert_with(|| SnapshotValueNode::Branch(BTreeMap::new()));

    match child {
        SnapshotValueNode::Branch(children) => {
            insert_config_leaf(children, &path[1..], leaf);
        }
        SnapshotValueNode::Leaf(_) => {
            let mut replacement = BTreeMap::new();
            insert_config_leaf(&mut replacement, &path[1..], leaf);
            *child = SnapshotValueNode::Branch(replacement);
        }
    }
}

fn flatten_config(
    root: &BTreeMap<String, SnapshotValueNode>,
    path: &mut Vec<String>,
    out: &mut Vec<SnapshotValueEntry>,
) {
    for (key, node) in root {
        path.push(key.clone());
        match node {
            SnapshotValueNode::Branch(children) => flatten_config(children, path, out),
            SnapshotValueNode::Leaf(leaf) => out.push(SnapshotValueEntry {
                path: path.clone(),
                leaf: leaf.clone(),
            }),
        }
        let _ = path.pop();
    }
}

pub fn capture_status_from_missing(missing: &[String]) -> CaptureStatus {
    if missing.is_empty() {
        CaptureStatus::Complete
    } else {
        CaptureStatus::Partial
    }
}

pub fn missing_detail(space: u8, offset_hex: &str, path: &[String]) -> String {
    format!(
        "space={} offset={} path={}",
        space,
        offset_hex,
        path.join("/")
    )
}

pub fn canonical_node_filename(node_id: &str) -> String {
    format!("{}.yaml", node_id.to_uppercase())
}

/// Update the baseline `value` for a leaf in the snapshot's config tree
/// matching the given (space, offset). Returns `true` if a leaf was updated.
pub fn update_snapshot_baseline(
    config: &mut BTreeMap<String, SnapshotValueNode>,
    space: u8,
    offset: &str,
    new_value: &str,
) -> bool {
    for node in config.values_mut() {
        match node {
            SnapshotValueNode::Branch(children) => {
                if update_snapshot_baseline(children, space, offset, new_value) {
                    return true;
                }
            }
            SnapshotValueNode::Leaf(leaf) => {
                if leaf.space == Some(space) && leaf.offset.as_deref() == Some(offset) {
                    leaf.value = new_value.to_string();
                    return true;
                }
            }
        }
    }
    false
}

pub fn update_snapshot_baseline_and_capture_time(
    snapshot: &mut NodeSnapshot,
    space: u8,
    offset: &str,
    new_value: &str,
    captured_at: &str,
) -> bool {
    let updated = update_snapshot_baseline(&mut snapshot.config, space, offset, new_value);
    if updated {
        snapshot.captured_at = captured_at.to_string();
    }
    updated
}

#[cfg(test)]
mod tests {
    use super::{
        update_snapshot_baseline_and_capture_time, CdiReference, CaptureStatus, NodeSnapshot,
        NodeSnapshotLifecycle, SnapshotLeafValue, SnapshotValueNode, SnipSnapshot,
    };
    use lcc_rs::NodeID;
    use std::collections::BTreeMap;

    fn make_snapshot() -> NodeSnapshot {
        let mut config = BTreeMap::new();
        config.insert(
            "Main".to_string(),
            SnapshotValueNode::Leaf(SnapshotLeafValue {
                value: "10".to_string(),
                space: Some(253),
                offset: Some("0x00000010".to_string()),
            }),
        );

        NodeSnapshot {
            node_key: "050201020300".to_string(),
            node_id: Some(NodeID::new([0x05, 0x02, 0x01, 0x02, 0x03, 0x00])),
            profile_stem: None,
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-04-20T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot::default(),
            cdi_ref: CdiReference {
                cache_key: "cache".to_string(),
                version: "1.0".to_string(),
                fingerprint: "fp".to_string(),
            },
            config,
            producer_identified_events: Vec::new(),
        }
    }

    #[test]
    fn updates_baseline_and_capture_time_when_leaf_matches() {
        let mut snapshot = make_snapshot();

        let updated = update_snapshot_baseline_and_capture_time(
            &mut snapshot,
            253,
            "0x00000010",
            "20",
            "2026-04-25T12:34:56Z",
        );

        assert!(updated);
        assert_eq!(snapshot.captured_at, "2026-04-25T12:34:56Z");

        match snapshot.config.get("Main") {
            Some(SnapshotValueNode::Leaf(leaf)) => assert_eq!(leaf.value, "20"),
            _ => panic!("expected matching snapshot leaf"),
        }
    }

    #[test]
    fn leaves_capture_time_unchanged_when_no_leaf_matches() {
        let mut snapshot = make_snapshot();

        let updated = update_snapshot_baseline_and_capture_time(
            &mut snapshot,
            253,
            "0x00000099",
            "20",
            "2026-04-25T12:34:56Z",
        );

        assert!(!updated);
        assert_eq!(snapshot.captured_at, "2026-04-20T00:00:00Z");
    }

    // ---- S8.5: NodeSnapshot identity widening ----

    fn real_node_snapshot(hex: &str) -> NodeSnapshot {
        let node_id = NodeID::from_hex_string(hex).unwrap();
        NodeSnapshot {
            node_key: node_id.to_canonical(),
            node_id: Some(node_id),
            profile_stem: None,
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-05-25T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot::default(),
            cdi_ref: CdiReference {
                cache_key: "c".to_string(),
                version: "v".to_string(),
                fingerprint: "f".to_string(),
            },
            config: BTreeMap::new(),
            producer_identified_events: Vec::new(),
        }
    }

    fn placeholder_snapshot(uuid: &str) -> NodeSnapshot {
        NodeSnapshot {
            node_key: format!("placeholder:{}", uuid),
            node_id: None,
            profile_stem: Some("Mustangpeak-Engineering_TurnoutBoss".to_string()),
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-05-25T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot {
                user_name: "My TurnoutBoss".to_string(),
                user_description: String::new(),
                manufacturer_name: "Mustangpeak Engineering".to_string(),
                model_name: "TurnoutBoss".to_string(),
            },
            cdi_ref: CdiReference {
                cache_key: "Mustangpeak-Engineering_TurnoutBoss".to_string(),
                version: "bundled".to_string(),
                fingerprint: "bundled".to_string(),
            },
            config: BTreeMap::new(),
            producer_identified_events: Vec::new(),
        }
    }

    #[test]
    fn real_snapshot_is_not_placeholder_and_has_some_node_id() {
        let snap = real_node_snapshot("050101010301");
        assert!(!snap.is_placeholder());
        assert!(snap.node_id.is_some());
        assert_eq!(snap.node_key, "050101010301");
    }

    #[test]
    fn placeholder_snapshot_is_placeholder_and_has_no_node_id() {
        let snap = placeholder_snapshot("11111111-2222-4333-8444-555555555555");
        assert!(snap.is_placeholder());
        assert!(snap.node_id.is_none());
        assert_eq!(
            snap.node_key,
            "placeholder:11111111-2222-4333-8444-555555555555"
        );
        assert!(snap.validate().is_ok());
    }

    #[test]
    fn placeholder_snapshot_round_trips_through_yaml() {
        let snap = placeholder_snapshot("11111111-2222-4333-8444-555555555555");
        let yaml = serde_yaml_ng::to_string(&snap).unwrap();
        let restored: NodeSnapshot = serde_yaml_ng::from_str(&yaml).unwrap();
        assert!(restored.is_placeholder());
        assert!(restored.node_id.is_none());
        assert_eq!(restored.node_key, snap.node_key);
        assert_eq!(restored.snip.manufacturer_name, "Mustangpeak Engineering");
    }

    #[test]
    fn real_snapshot_round_trips_through_yaml() {
        let snap = real_node_snapshot("050101010301");
        let yaml = serde_yaml_ng::to_string(&snap).unwrap();
        let restored: NodeSnapshot = serde_yaml_ng::from_str(&yaml).unwrap();
        assert!(!restored.is_placeholder());
        assert_eq!(restored.node_id.unwrap().to_canonical(), "050101010301");
        assert_eq!(restored.node_key, "050101010301");
    }

    #[test]
    fn legacy_snapshot_without_node_key_field_backfills_from_node_id() {
        // Snapshots written before S8.5 only have `nodeId`; loading them
        // must still produce a usable `node_key`.
        let legacy = r#"
nodeId: "050101011402"
capturedAt: "2026-01-01T00:00:00Z"
captureStatus: complete
snip:
  userName: ""
  userDescription: ""
  manufacturerName: ""
  modelName: ""
cdiRef:
  cacheKey: ""
  version: ""
  fingerprint: ""
"#;
        let snap: NodeSnapshot = serde_yaml_ng::from_str(legacy).unwrap();
        assert!(!snap.is_placeholder());
        assert_eq!(snap.node_key, "050101011402");
        assert_eq!(snap.node_id.unwrap().to_canonical(), "050101011402");
    }

    #[test]
    fn placeholder_filename_replaces_colon_with_underscore() {
        let snap = placeholder_snapshot("11111111-2222-4333-8444-555555555555");
        assert_eq!(
            snap.filename_basis(),
            "placeholder_11111111-2222-4333-8444-555555555555"
        );
    }

    #[test]
    fn real_node_filename_is_canonical_uppercase() {
        let snap = real_node_snapshot("050101010301");
        // Existing convention: uppercase canonical compact form.
        assert_eq!(snap.filename_basis(), "050101010301");
    }

    // ---- S8.6: single mint formula for CdiReference ----

    #[test]
    fn s8_6_from_snip_sanitizes_non_alphanumeric_in_every_fragment() {
        let snip = SnipSnapshot {
            user_name: String::new(),
            user_description: String::new(),
            manufacturer_name: "Mustangpeak Engineering".to_string(),
            model_name: "TurnoutBoss".to_string(),
        };
        let cdi_ref = CdiReference::from_snip(&snip, "5.14", "len:123");
        // Spaces → `_`; dots in the version fragment also → `_` so the
        // stored cache_key matches the on-disk `cdi_cache/` filename
        // (which has used the alphanumeric+`-`+`_` rule since v0.x).
        assert_eq!(cdi_ref.cache_key, "Mustangpeak_Engineering_TurnoutBoss_5_14");
        assert_eq!(cdi_ref.version, "5.14");
        assert_eq!(cdi_ref.fingerprint, "len:123");
        assert!(!cdi_ref.is_bundled());
    }

    #[test]
    fn s8_6_from_snip_preserves_hyphens_and_underscores() {
        let snip = SnipSnapshot {
            user_name: String::new(),
            user_description: String::new(),
            manufacturer_name: "RR-CirKits".to_string(),
            model_name: "Tower_LCC".to_string(),
        };
        let cdi_ref = CdiReference::from_snip(&snip, "1.0", "len:50");
        assert_eq!(cdi_ref.cache_key, "RR-CirKits_Tower_LCC_1_0");
    }

    #[test]
    fn s8_6_from_profile_stem_uses_stem_verbatim_and_marks_bundled() {
        let cdi_ref = CdiReference::from_profile_stem("Mustangpeak-Engineering_TurnoutBoss");
        assert_eq!(cdi_ref.cache_key, "Mustangpeak-Engineering_TurnoutBoss");
        assert_eq!(cdi_ref.version, "bundled");
        assert_eq!(cdi_ref.fingerprint, "bundled");
        assert!(cdi_ref.is_bundled());
    }
}
