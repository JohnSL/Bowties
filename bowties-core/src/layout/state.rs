//! `LayoutState` — single in-memory owner of the open layout.
//!
//! Holds the three-layer projection (saved → captured → drafts) for one open
//! layout directory. The saved layer mirrors what is on disk; the captured
//! layer carries data freshly read from the bus but not yet persisted; the
//! drafts layer carries frontend-side edits that will be merged on the next
//! save.
//!
//! See ADR-0015 (`product/architecture/adr/0015-backend-layout-state-single-owner.md`)
//! for the single-owner decision and invariants. The save path consults
//! `cdi_xml(key)` / `config_tree(key)` (captured-over-saved precedence) when
//! building a `NodeSnapshot`, and the offline catalog rebuild derives its
//! projections from the saved layer.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::layout::channels::ChannelsDocument;
use crate::layout::facilities::FacilitiesDocument;
use crate::layout::io::LayoutDirectoryReadData;
use crate::layout::manifest::LayoutManifest;
use crate::layout::node_snapshot::NodeSnapshot;
use crate::layout::offline_changes::OfflineChange;
use crate::layout::types::LayoutFile;
use crate::node_key::NodeKey;
use crate::node_tree::NodeConfigTree;

/// Persisted-on-disk data for one node in the open layout.
///
/// The `snapshot` is the YAML round-trip shape. `cdi_xml` and `tree` are
/// the resolved derived data — `None` when the node's CDI is unavailable
/// (placeholder before reconstitute, or a real node whose CDI download
/// has not yet been replayed into memory).
#[derive(Debug, Clone)]
pub struct SavedNode {
    pub snapshot: NodeSnapshot,
    pub cdi_xml: Option<String>,
    pub tree: Option<NodeConfigTree>,
}

/// Freshly-captured live data for one node, not yet persisted.
///
/// Each field is `Option` so partial captures (e.g. SNIP arrived but
/// CDI download still in flight) are representable.
#[derive(Debug, Clone, Default)]
pub struct CapturedNode {
    pub snip: Option<lcc_rs::SNIPData>,
    pub pip_flags: Option<lcc_rs::ProtocolFlags>,
    pub cdi_xml: Option<String>,
    pub config_values: HashMap<String, [u8; 8]>,
    pub tree: Option<NodeConfigTree>,
}

/// Frontend-side draft edits mirrored into the backend so backend
/// read paths (Spec 018 / S6 composition and future consumers) can
/// observe a merged saved + drafts view without waiting for save.
///
/// The frontend collects `LayoutEditDelta`s from its own draft stores
/// and pushes them through `sync_layout_drafts`; the backend applies
/// them to a *clone* of the saved documents and stashes the result
/// here. The `saved` layer is never mutated by the drafts path.
///
/// Only the documents that today have consumers of the merged view
/// are materialised (`facilities`, `channels`). Bowtie metadata drafts
/// stay frontend-only until a read path needs them here.
#[derive(Debug, Clone, Default)]
pub struct DraftLayer {
    pub pending_facilities: Option<FacilitiesDocument>,
    pub pending_channels: Option<ChannelsDocument>,
}

/// Error surface of [`LayoutState::sync_drafts`]. Wraps the two
/// document-scoped apply-error types so callers get one seam to match
/// on regardless of which document rejected the incoming delta.
#[derive(Debug)]
pub enum DraftSyncError {
    Facility(crate::layout::facilities::FacilityApplyError),
    Channel(crate::layout::channels::ChannelApplyError),
}

impl std::fmt::Display for DraftSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DraftSyncError::Facility(e) => write!(f, "facility draft sync failed: {}", e),
            DraftSyncError::Channel(e) => write!(f, "channel draft sync failed: {}", e),
        }
    }
}

impl std::error::Error for DraftSyncError {}

/// Single in-memory owner of one open layout.
#[derive(Debug, Clone)]
pub struct LayoutState {
    root: PathBuf,
    manifest: LayoutManifest,
    saved: HashMap<NodeKey, SavedNode>,
    bowties: LayoutFile,
    channels: ChannelsDocument,
    facilities: FacilitiesDocument,
    offline_changes: Vec<OfflineChange>,
    captured: HashMap<NodeKey, CapturedNode>,
    drafts: DraftLayer,
    discovered_roles: BTreeMap<String, crate::layout::types::RoleClassification>,
}

impl LayoutState {
    /// Build a `LayoutState` from data already loaded by
    /// [`crate::layout::read_capture`], plus any per-node CDI XML / tree
    /// the caller has already resolved (today: from the loop in
    /// `open_layout_directory`).
    ///
    /// Snapshots whose `node_key` does not parse as a [`NodeKey`] are
    /// skipped — the read path already validates them, so this is a
    /// defense-in-depth filter only.
    pub fn from_loaded(
        root: PathBuf,
        loaded: LayoutDirectoryReadData,
        cdi_xml_by_key: HashMap<NodeKey, String>,
        trees_by_key: HashMap<NodeKey, NodeConfigTree>,
    ) -> Self {
        let LayoutDirectoryReadData {
            manifest,
            node_snapshots,
            bowties,
            offline_changes,
            recovery_occurred: _,
            channels,
            facilities,
            load_warnings: _,
        } = loaded;

        let mut saved: HashMap<NodeKey, SavedNode> = HashMap::with_capacity(node_snapshots.len());
        for snapshot in node_snapshots {
            let key = match NodeKey::parse(&snapshot.node_key) {
                Ok(k) => k,
                Err(_) => continue,
            };
            let cdi_xml = cdi_xml_by_key.get(&key).cloned();
            let tree = trees_by_key.get(&key).cloned();
            saved.insert(
                key,
                SavedNode {
                    snapshot,
                    cdi_xml,
                    tree,
                },
            );
        }

        LayoutState {
            root,
            manifest,
            saved,
            bowties,
            channels,
            facilities,
            offline_changes,
            captured: HashMap::new(),
            drafts: DraftLayer::default(),
            discovered_roles: BTreeMap::new(),
        }
    }

    // ── Queries (read paths) ───────────────────────────────────────────────

    /// Folder path of the open layout on disk.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// On-disk manifest.
    pub fn manifest(&self) -> &LayoutManifest {
        &self.manifest
    }

    /// Top-level bowtie / role-classification document.
    pub fn bowties(&self) -> &LayoutFile {
        &self.bowties
    }

    pub fn channels(&self) -> &ChannelsDocument {
        &self.channels
    }

    pub fn facilities(&self) -> &FacilitiesDocument {
        &self.facilities
    }

    /// Effective (drafts-over-saved) facilities view.
    ///
    /// Mirrors [`Self::cdi_xml`]'s captured-over-saved precedence for the
    /// drafts layer: any backend read path that must see frontend-side
    /// pending edits (Spec 018 / S6 bowtie composition, and future
    /// callers such as the offline catalog rebuild) consults this
    /// accessor instead of `facilities()`.
    pub fn effective_facilities(&self) -> &FacilitiesDocument {
        self.drafts
            .pending_facilities
            .as_ref()
            .unwrap_or(&self.facilities)
    }

    /// Effective (drafts-over-saved) channels view. See
    /// [`Self::effective_facilities`] for the precedence rationale.
    pub fn effective_channels(&self) -> &ChannelsDocument {
        self.drafts
            .pending_channels
            .as_ref()
            .unwrap_or(&self.channels)
    }

    pub fn offline_changes(&self) -> &[OfflineChange] {
        &self.offline_changes
    }

    /// Iterator over the keys of every node persisted in the open layout
    /// (the saved layer — captured-only nodes are *not* yet persistable).
    pub fn persisted_node_keys(&self) -> impl Iterator<Item = &NodeKey> {
        self.saved.keys()
    }

    /// Keys of every node that has a config tree in either saved or
    /// captured layer (deduplicated).
    pub fn all_tree_keys(&self) -> Vec<NodeKey> {
        let mut keys: std::collections::HashSet<NodeKey> = self
            .saved
            .iter()
            .filter(|(_, s)| s.tree.is_some())
            .map(|(k, _)| *k)
            .collect();
        for (k, c) in &self.captured {
            if c.tree.is_some() {
                keys.insert(*k);
            }
        }
        keys.into_iter().collect()
    }

    pub fn saved_node(&self, key: &NodeKey) -> Option<&SavedNode> {
        self.saved.get(key)
    }

    pub fn captured_node(&self, key: &NodeKey) -> Option<&CapturedNode> {
        self.captured.get(key)
    }

    /// Resolved CDI XML for `key`, preferring captured-fresh-from-bus over
    /// the saved-on-disk copy. Returns `None` when neither layer has it.
    pub fn cdi_xml(&self, key: &NodeKey) -> Option<&str> {
        if let Some(captured) = self.captured.get(key) {
            if let Some(xml) = &captured.cdi_xml {
                return Some(xml);
            }
        }
        self.saved
            .get(key)
            .and_then(|saved| saved.cdi_xml.as_deref())
    }

    /// Resolved config tree for `key`, preferring captured-fresh over saved.
    pub fn config_tree(&self, key: &NodeKey) -> Option<&NodeConfigTree> {
        if let Some(captured) = self.captured.get(key) {
            if let Some(tree) = &captured.tree {
                return Some(tree);
            }
        }
        self.saved
            .get(key)
            .and_then(|saved| saved.tree.as_ref())
    }

    /// Mutable access to the config tree for `key`, preferring
    /// captured-fresh over saved — same precedence as [`Self::config_tree`].
    pub fn config_tree_mut(&mut self, key: &NodeKey) -> Option<&mut NodeConfigTree> {
        if let Some(captured) = self.captured.get_mut(key) {
            if captured.tree.is_some() {
                return captured.tree.as_mut();
            }
        }
        self.saved
            .get_mut(key)
            .and_then(|saved| saved.tree.as_mut())
    }

    /// Store a config tree in the captured layer for `key`.
    ///
    /// Subsequent [`Self::config_tree`] calls return the stored tree
    /// (captured-over-saved precedence).
    pub fn set_config_tree(&mut self, key: NodeKey, tree: NodeConfigTree) {
        self.captured.entry(key).or_default().tree = Some(tree);
    }

    /// Update the saved-layer snapshot's config baseline for a single
    /// address. Called after `write_modified_values` successfully writes
    /// a leaf to the bus so the snapshot stays in sync with the committed
    /// tree values. Without this, a Phase 3 reconcile re-save would
    /// overwrite the disk snapshot with the stale pre-write baseline.
    pub fn update_snapshot_config(
        &mut self,
        key: &NodeKey,
        space: u8,
        offset_hex: &str,
        value_str: &str,
    ) {
        if let Some(saved) = self.saved.get_mut(key) {
            crate::layout::node_snapshot::update_snapshot_baseline(
                &mut saved.snapshot.config,
                space,
                offset_hex,
                value_str,
            );
        }
    }

    /// Snapshot to be written by the next save for `key`.
    ///
    /// Slice 1 returns the saved-layer snapshot verbatim — this is enough
    /// to round-trip "open → save with no edits" without losing nodes.
    /// Slice 2 will merge in captured + draft data here so that fresh
    /// CDI reads and frontend edits land on disk.
    pub fn snapshot_for_save(&self, key: &NodeKey) -> Option<NodeSnapshot> {
        self.saved.get(key).map(|saved| saved.snapshot.clone())
    }

    // ── Mutations (slice-2 fill-in points) ─────────────────────────────────

    /// Record freshly-captured live data for a node. Subsequent calls
    /// merge field-by-field so a SNIP-only call followed by a CDI-only
    /// call leaves both fields populated.
    pub fn record_captured(&mut self, key: NodeKey, captured: CapturedNode) {
        let entry = self.captured.entry(key).or_default();
        if let Some(v) = captured.snip {
            entry.snip = Some(v);
        }
        if let Some(v) = captured.pip_flags {
            entry.pip_flags = Some(v);
        }
        if let Some(v) = captured.cdi_xml {
            entry.cdi_xml = Some(v);
        }
        if let Some(v) = captured.tree {
            entry.tree = Some(v);
        }
        for (k, v) in captured.config_values {
            entry.config_values.insert(k, v);
        }
    }

    /// Replace the drafts layer with the materialised documents that
    /// result from applying `deltas` to a clone of the saved layer.
    ///
    /// The saved layer is not mutated — the frontend is still the
    /// draft-layer owner per ADR-0012; this method mirrors those drafts
    /// into `LayoutState` so [`Self::effective_facilities`] /
    /// [`Self::effective_channels`] can serve merged reads.
    ///
    /// Idempotent w.r.t. any given delta set: calling with the same
    /// deltas twice produces the same `drafts` layer. Callers therefore
    /// send the *complete* current delta set on every sync (not an
    /// incremental extension) — matches the frontend's
    /// `collectDeltas()` semantics.
    pub fn sync_drafts(
        &mut self,
        deltas: &[crate::layout::types::LayoutEditDelta],
    ) -> Result<(), DraftSyncError> {
        let mut facilities = self.facilities.clone();
        let mut channels = self.channels.clone();
        crate::layout::facilities::apply_facility_deltas(&mut facilities, deltas)
            .map_err(DraftSyncError::Facility)?;
        crate::layout::channels::apply_channel_deltas(&mut channels, deltas)
            .map_err(DraftSyncError::Channel)?;
        self.drafts.pending_facilities = Some(facilities);
        self.drafts.pending_channels = Some(channels);
        Ok(())
    }

    /// Drop the drafts layer. Called after save (drafts have been folded
    /// into the saved layer + written to disk) and on frontend Discard.
    pub fn clear_drafts(&mut self) {
        self.drafts = DraftLayer::default();
    }

    // ── Lifecycle (informational; no persisted-data effect) ────────────────

    /// Note that `key` is currently answering on the bus. Slice 1 stores
    /// nothing — included so callers can wire the call site now.
    pub fn note_node_present_on_bus(&mut self, _key: NodeKey, _alias: u16) {
        // Reserved for slice 2+: track live presence for UI status.
    }

    pub fn note_node_off_bus(&mut self, _key: &NodeKey) {
        // Reserved for slice 2+.
    }

    // ── Mutating helpers for slice-2 expansion ─────────────────────────────

    /// Direct mutable access to the saved bowties document for callers
    /// applying layout deltas. Slice 2 will replace this with a higher-
    /// level `apply_layout_deltas` method that owns the merge.
    pub fn bowties_mut(&mut self) -> &mut LayoutFile {
        &mut self.bowties
    }

    pub fn facilities_mut(&mut self) -> &mut FacilitiesDocument {
        &mut self.facilities
    }

    pub fn channels_mut(&mut self) -> &mut ChannelsDocument {
        &mut self.channels
    }

    pub fn set_offline_changes(&mut self, changes: Vec<OfflineChange>) {
        self.offline_changes = changes;
    }

    /// Record protocol-discovered role classifications.
    /// Called by catalog rebuild sites. Merges into the accumulated set
    /// (last-write-wins per key). Cleared after save.
    pub fn record_discovered_roles(
        &mut self,
        roles: BTreeMap<String, crate::layout::types::RoleClassification>,
    ) {
        for (key, rc) in roles {
            self.discovered_roles.insert(key, rc);
        }
    }

    /// Protocol-discovered role classifications accumulated since last save.
    pub fn discovered_roles(&self) -> &BTreeMap<String, crate::layout::types::RoleClassification> {
        &self.discovered_roles
    }

    /// Clear accumulated discovered roles (called after save).
    pub fn clear_discovered_roles(&mut self) {
        self.discovered_roles.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::node_snapshot::{
        CaptureStatus, CdiReference, NodeSnapshot, NodeSnapshotLifecycle, SnipSnapshot,
    };
    use lcc_rs::NodeID;
    use std::collections::BTreeMap;

    fn dummy_snip() -> SnipSnapshot {
        SnipSnapshot {
            user_name: String::new(),
            user_description: String::new(),
            manufacturer_name: "Acme".to_string(),
            model_name: "Widget".to_string(),
        }
    }

    fn dummy_cdi_ref() -> CdiReference {
        CdiReference {
            cache_key: "Acme_Widget".to_string(),
            version: "1.0".to_string(),
            fingerprint: "abc123".to_string(),
        }
    }

    fn live_snapshot(canonical_id: &str) -> NodeSnapshot {
        let node_id = NodeID::from_hex_string(canonical_id).expect("valid node id");
        NodeSnapshot {
            node_key: canonical_id.to_string(),
            node_id: Some(node_id),
            profile_stem: None,
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-06-28T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: dummy_snip(),
            cdi_ref: dummy_cdi_ref(),
            config: BTreeMap::new(),
            producer_identified_events: Vec::new(),
        }
    }

    fn loaded_with(snapshots: Vec<NodeSnapshot>) -> LayoutDirectoryReadData {
        LayoutDirectoryReadData {
            manifest: LayoutManifest {
                schema_version: 4,
                layout_id: "test".to_string(),
                captured_at: "2026-06-28T00:00:00Z".to_string(),
                last_saved_at: "2026-06-28T00:00:00Z".to_string(),
                active_mode: "offline_file".to_string(),
                match_thresholds: Default::default(),
                connections: Vec::new(),
            },
            node_snapshots: snapshots,
            bowties: LayoutFile::default(),
            offline_changes: Vec::new(),
            recovery_occurred: false,
            channels: ChannelsDocument::default(),
            facilities: FacilitiesDocument::default(),
            load_warnings: Vec::new(),
        }
    }

    #[test]
    fn from_loaded_indexes_every_snapshot_by_node_key() {
        let snapshots = vec![
            live_snapshot("0201570002D9"),
            live_snapshot("020157100997"),
            live_snapshot("06010000E427"),
        ];
        let loaded = loaded_with(snapshots);

        let state = LayoutState::from_loaded(
            PathBuf::from("/tmp/test-layout"),
            loaded,
            HashMap::new(),
            HashMap::new(),
        );

        let mut keys: Vec<String> = state
            .persisted_node_keys()
            .map(|k| k.to_string())
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "0201570002D9".to_string(),
                "020157100997".to_string(),
                "06010000E427".to_string(),
            ]
        );
    }

    #[test]
    fn snapshot_for_save_round_trips_each_persisted_node() {
        // The slice-1 invariant: if we open a layout and immediately ask
        // `snapshot_for_save` for every persisted key, we get back the
        // same snapshots we loaded — no nodes silently dropped.
        let snapshots = vec![
            live_snapshot("0201570002D9"),
            live_snapshot("020157100997"),
        ];
        let expected: Vec<String> = snapshots.iter().map(|s| s.node_key.clone()).collect();
        let loaded = loaded_with(snapshots);

        let state = LayoutState::from_loaded(
            PathBuf::from("/tmp/test-layout"),
            loaded,
            HashMap::new(),
            HashMap::new(),
        );

        let mut round_tripped: Vec<String> = state
            .persisted_node_keys()
            .map(|k| {
                state
                    .snapshot_for_save(k)
                    .expect("persisted key has a snapshot")
                    .node_key
            })
            .collect();
        round_tripped.sort();
        let mut expected_sorted = expected;
        expected_sorted.sort();
        assert_eq!(round_tripped, expected_sorted);
    }

    #[test]
    fn cdi_xml_prefers_captured_over_saved() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        let mut saved_cdi = HashMap::new();
        saved_cdi.insert(key, "<cdi version=\"saved\"/>".to_string());

        let mut state = LayoutState::from_loaded(
            PathBuf::from("/tmp"),
            loaded_with(vec![live_snapshot("0201570002D9")]),
            saved_cdi,
            HashMap::new(),
        );

        assert_eq!(state.cdi_xml(&key), Some("<cdi version=\"saved\"/>"));

        state.record_captured(
            key,
            CapturedNode {
                cdi_xml: Some("<cdi version=\"fresh\"/>".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(state.cdi_xml(&key), Some("<cdi version=\"fresh\"/>"));
    }

    #[test]
    fn record_captured_merges_fields_across_calls() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        let mut state = LayoutState::from_loaded(
            PathBuf::from("/tmp"),
            loaded_with(vec![live_snapshot("0201570002D9")]),
            HashMap::new(),
            HashMap::new(),
        );

        state.record_captured(
            key,
            CapturedNode {
                cdi_xml: Some("<cdi/>".to_string()),
                ..Default::default()
            },
        );
        state.record_captured(
            key,
            CapturedNode {
                snip: Some(lcc_rs::SNIPData {
                    manufacturer: "Acme".to_string(),
                    model: "Widget".to_string(),
                    hardware_version: String::new(),
                    software_version: "2.0".to_string(),
                    user_name: String::new(),
                    user_description: String::new(),
                }),
                ..Default::default()
            },
        );

        let captured = state.captured_node(&key).expect("captured present");
        assert_eq!(captured.cdi_xml.as_deref(), Some("<cdi/>"));
        assert_eq!(
            captured.snip.as_ref().map(|s| s.software_version.as_str()),
            Some("2.0")
        );
    }

    // ── Slice 2 behavior pins ──────────────────────────────────────────────
    //
    // R1 (open → reconnect → save with no edits silently dropped 4/5 nodes):
    // after `from_loaded` with the per-node CDI map populated, every persisted
    // node has a non-`None` `cdi_xml` lookup. `proxy_snapshot_data` in src-tauri
    // composes this with `proxy.cdi.is_none()` to produce a `Some(len)` for the
    // snapshot fingerprint. Without this guarantee, `cdi_ref.fingerprint`
    // becomes `"missing"` and the legacy `.retain` drops the snapshot at save.

    #[test]
    fn r1_every_persisted_node_resolves_cdi_xml_after_open() {
        let snapshots = vec![
            live_snapshot("0201570002D9"),
            live_snapshot("020157100997"),
            live_snapshot("06010000E427"),
            live_snapshot("060300000033"),
            live_snapshot("0900990501C0"),
        ];
        let mut cdi_map = HashMap::new();
        for snap in &snapshots {
            let key = NodeKey::parse(&snap.node_key).unwrap();
            cdi_map.insert(key, format!("<cdi name=\"{}\"/>", snap.node_key));
        }

        let state = LayoutState::from_loaded(
            PathBuf::from("/tmp/layout-r1"),
            loaded_with(snapshots),
            cdi_map,
            HashMap::new(),
        );

        for key in state.persisted_node_keys() {
            assert!(
                state.cdi_xml(key).is_some(),
                "every persisted node must resolve CDI XML after open; missing for {}",
                key,
            );
            assert!(
                state.cdi_xml(key).unwrap().len() > 0,
                "resolved CDI XML must be non-empty for {}",
                key,
            );
        }
    }

    // R2 (newly-added Tower-LCC dropped because proxy.cdi was None after the
    // bus successfully downloaded CDI): when `record_captured` is called with
    // CDI XML for a node not yet in the saved layer, `cdi_xml` resolves to the
    // captured bytes. This is the mechanism that lets the save path pick up
    // newly-added nodes even when the proxy has lost the data.

    #[test]
    fn r2_captured_cdi_resolves_for_unsaved_node() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        // Start with NO saved nodes — Tower-LCC is freshly added.
        let mut state = LayoutState::from_loaded(
            PathBuf::from("/tmp/layout-r2"),
            loaded_with(Vec::new()),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(
            state.cdi_xml(&key).is_none(),
            "no data anywhere yet",
        );

        // Simulate: CDI download for Tower-LCC completed, src-tauri called
        // record_captured.
        state.record_captured(
            key,
            CapturedNode {
                cdi_xml: Some(
                    "<cdi><identification><manufacturer>Tower</manufacturer></identification></cdi>"
                        .to_string(),
                ),
                ..Default::default()
            },
        );

        let resolved = state.cdi_xml(&key).expect("captured CDI must resolve");
        assert!(
            resolved.contains("Tower"),
            "captured CDI bytes must round-trip through cdi_xml lookup",
        );
    }

    // ── Draft-layer sync (Spec 018 / S6 bugfix: bowtie composition seam) ───
    //
    // `LayoutState.drafts` was sketched by ADR-0015 for exactly this class
    // of read: frontend edits that haven't been saved yet, but that a
    // backend read path (bowtie composition, offline catalog rebuild)
    // needs to see. `sync_drafts` materialises the pending documents
    // from the saved layer + the frontend's collected deltas;
    // `effective_facilities` / `effective_channels` serve the merged
    // view. Save then folds drafts into `saved` and calls `clear_drafts`.

    fn empty_state() -> LayoutState {
        LayoutState::from_loaded(
            PathBuf::from("/tmp/draft-sync"),
            loaded_with(Vec::new()),
            HashMap::new(),
            HashMap::new(),
        )
    }

    fn add_facility_delta(facility_id: &str, name: &str) -> crate::layout::types::LayoutEditDelta {
        crate::layout::types::LayoutEditDelta::AddFacility {
            facility: crate::layout::facilities::Facility {
                facility_id: facility_id.to_string(),
                template_id: "block-indicator".to_string(),
                name: name.to_string(),
                slot_bindings: BTreeMap::from([
                    ("input".to_string(), Vec::new()),
                    ("output".to_string(), Vec::new()),
                ]),
            },
        }
    }

    #[test]
    fn effective_facilities_falls_back_to_saved_when_no_drafts() {
        let state = empty_state();
        assert!(state.effective_facilities().facilities.is_empty());
        assert!(std::ptr::eq(
            state.effective_facilities(),
            state.facilities(),
        ));
    }

    #[test]
    fn sync_drafts_materialises_pending_facilities_from_saved_plus_deltas() {
        let mut state = empty_state();
        assert!(state.effective_facilities().facilities.is_empty());

        state
            .sync_drafts(&[add_facility_delta("fac-1", "Block 5")])
            .expect("sync_drafts succeeds");

        // saved untouched — the drafts path never mutates the saved layer.
        assert!(state.facilities().facilities.is_empty());
        // effective view carries the pending facility.
        assert_eq!(state.effective_facilities().facilities.len(), 1);
        assert_eq!(
            state.effective_facilities().facilities[0].facility_id,
            "fac-1"
        );
    }

    #[test]
    fn sync_drafts_is_idempotent_for_the_same_delta_set() {
        let mut state = empty_state();
        state
            .sync_drafts(&[add_facility_delta("fac-1", "Block 5")])
            .expect("first sync");
        state
            .sync_drafts(&[add_facility_delta("fac-1", "Block 5")])
            .expect("second sync with same deltas");
        // Each sync starts from `saved` and re-applies — no double-append.
        assert_eq!(state.effective_facilities().facilities.len(), 1);
    }

    #[test]
    fn sync_drafts_replaces_previous_pending_view() {
        let mut state = empty_state();
        state
            .sync_drafts(&[add_facility_delta("fac-1", "Block 5")])
            .expect("first sync");
        // Frontend then discards fac-1 and adds fac-2 instead. It resends
        // its full current delta set (which no longer contains fac-1).
        state
            .sync_drafts(&[add_facility_delta("fac-2", "Block 7")])
            .expect("second sync with a different set");
        let ids: Vec<&str> = state
            .effective_facilities()
            .facilities
            .iter()
            .map(|f| f.facility_id.as_str())
            .collect();
        assert_eq!(ids, vec!["fac-2"]);
    }

    #[test]
    fn clear_drafts_drops_pending_view_back_to_saved() {
        let mut state = empty_state();
        state
            .sync_drafts(&[add_facility_delta("fac-1", "Block 5")])
            .expect("sync");
        assert_eq!(state.effective_facilities().facilities.len(), 1);
        state.clear_drafts();
        assert!(state.effective_facilities().facilities.is_empty());
        assert!(std::ptr::eq(
            state.effective_facilities(),
            state.facilities(),
        ));
    }

    // ── Config tree mutation API ──────────────────────────────────────────

    fn minimal_tree(node_id: &str) -> NodeConfigTree {
        NodeConfigTree {
            node_id: node_id.to_string(),
            identity: None,
            connector_profile: None,
            connector_profile_warning: None,
            unknown_variants: Vec::new(),
            profile_applied: false,
            segments: Vec::new(),
        }
    }

    #[test]
    fn config_tree_mut_returns_captured_tree_for_mutation() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        let mut state = LayoutState::from_loaded(
            PathBuf::from("/tmp"),
            loaded_with(vec![live_snapshot("0201570002D9")]),
            HashMap::new(),
            HashMap::new(),
        );

        // Record a captured tree with no segments.
        state.record_captured(
            key,
            CapturedNode {
                tree: Some(minimal_tree("02.01.57.00.02.D9")),
                ..Default::default()
            },
        );
        assert!(state.config_tree(&key).unwrap().segments.is_empty());

        // Mutate through config_tree_mut: mark profile_applied = true.
        let tree_mut = state.config_tree_mut(&key).expect("captured tree exists");
        tree_mut.profile_applied = true;

        // Assert mutation is visible through the read accessor.
        assert!(state.config_tree(&key).unwrap().profile_applied);
    }

    #[test]
    fn config_tree_mut_falls_back_to_saved_tree() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        let trees = HashMap::from([(key, minimal_tree("02.01.57.00.02.D9"))]);
        let mut state = LayoutState::from_loaded(
            PathBuf::from("/tmp"),
            loaded_with(vec![live_snapshot("0201570002D9")]),
            HashMap::new(),
            trees,
        );

        // No captured tree — config_tree_mut should return saved tree.
        let tree_mut = state.config_tree_mut(&key);
        assert!(tree_mut.is_some());

        // Mutate and verify.
        tree_mut.unwrap().profile_applied = true;
        assert!(state.config_tree(&key).unwrap().profile_applied);
    }

    #[test]
    fn config_tree_mut_returns_none_when_no_tree_anywhere() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        let mut state = LayoutState::from_loaded(
            PathBuf::from("/tmp"),
            loaded_with(vec![live_snapshot("0201570002D9")]),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(state.config_tree_mut(&key).is_none());
    }

    #[test]
    fn set_config_tree_stores_in_captured_layer() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        let mut state = LayoutState::from_loaded(
            PathBuf::from("/tmp"),
            loaded_with(vec![live_snapshot("0201570002D9")]),
            HashMap::new(),
            HashMap::new(),
        );

        // Initially no tree.
        assert!(state.config_tree(&key).is_none());

        state.set_config_tree(key, minimal_tree("02.01.57.00.02.D9"));

        // Now config_tree returns it via captured-over-saved precedence.
        let tree = state.config_tree(&key).expect("tree stored");
        assert_eq!(tree.node_id, "02.01.57.00.02.D9");
    }

    // ── Discovered roles (catalog-merge elimination) ───────────────────────

    #[test]
    fn discovered_roles_accumulates_and_clears() {
        use crate::layout::types::RoleClassification;

        let mut state = empty_state();
        assert!(state.discovered_roles().is_empty());

        // First batch: one role.
        let mut batch1 = std::collections::BTreeMap::new();
        batch1.insert(
            "node:path1".to_string(),
            RoleClassification { role: "Producer".to_string() },
        );
        state.record_discovered_roles(batch1);
        assert_eq!(state.discovered_roles().len(), 1);
        assert_eq!(state.discovered_roles()["node:path1"].role, "Producer");

        // Second batch: a new key + overwrite of the first.
        let mut batch2 = std::collections::BTreeMap::new();
        batch2.insert(
            "node:path1".to_string(),
            RoleClassification { role: "Both".to_string() },
        );
        batch2.insert(
            "node:path2".to_string(),
            RoleClassification { role: "Consumer".to_string() },
        );
        state.record_discovered_roles(batch2);
        assert_eq!(state.discovered_roles().len(), 2);
        assert_eq!(state.discovered_roles()["node:path1"].role, "Both");
        assert_eq!(state.discovered_roles()["node:path2"].role, "Consumer");

        // Clear.
        state.clear_discovered_roles();
        assert!(state.discovered_roles().is_empty());
    }
}
