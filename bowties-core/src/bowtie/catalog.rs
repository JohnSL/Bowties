//! Bowtie catalog builder — pure domain logic for catalog construction.
//!
//! ## Data flow
//! 1. `read_all_config_values` completes for all nodes.
//! 2. `query_event_roles` sends `IdentifyEventsAddressed` to each node (125 ms apart)
//!    and collects `ProducerIdentified` / `ConsumerIdentified` replies for 500 ms.
//! 3. `build_bowtie_catalog` groups the resulting `NodeRoles` map into `BowtieCard`s.
//! 4. The catalog is stored in `AppState.bowties_catalog` and emitted as `cdi-read-complete`.
//! 5. `get_bowties` Tauri command lets the frontend retrieve the catalog on demand.
//!
//! All functions in this module are pure (no Tauri / AppState / network deps).

use std::collections::HashMap;

use crate::bowtie::types::*;
use crate::node_key::NodeKey;
use crate::node_tree::NodeRoles;

// ── Well-known event IDs ──────────────────────────────────────────────────────

/// Standard LCC well-known event IDs from the OpenLCB Event Identifiers Standard.
///
/// These events are handled at the protocol level by nodes such as command stations
/// and throttles — those nodes respond to `IdentifyEventsAddressed` for them even
/// when no user has configured a CDI slot to one of these values.  Therefore bowtie
/// cards for well-known event IDs are built exclusively from `config_value_cache`
/// (CDI slots the user explicitly set to these values), not from the protocol exchange.
pub const WELL_KNOWN_EVENT_IDS: &[([u8; 8], &str)] = &[
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF], "Emergency Off"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFE], "Clear Emergency Off"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFD], "Emergency Stop"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFC], "Clear Emergency Stop"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xF8], "New Log Entry"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFE, 0x00], "Ident Button Pressed"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFD, 0x01], "Link Error 1"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFD, 0x02], "Link Error 2"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFD, 0x03], "Link Error 3"),
    ([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFD, 0x04], "Link Error 4"),
    ([0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x01], "Duplicate Node ID"),
    ([0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x03, 0x03], "Is Train"),
    ([0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x03, 0x04], "Is Traction Proxy"),
];

/// Returns `true` for any event ID whose first byte is 0x00.
///
/// Per the LCC Unique Identifiers Standard (S-9.7.0.3 §5.2), the leading-zero range
/// is reserved / "uninitialized or non-standard".  Nodes commonly store a value in
/// this range (e.g. 00.00.00.00.00.00.00.FF) in CDI slots that have never been
/// configured by the user.  Such IDs must never appear as bowtie catalog entries.
#[inline]
pub fn is_placeholder_event_id(b: &[u8; 8]) -> bool {
    b[0] == 0x00
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return a human-readable node name using the SNIP priority chain:
/// user_name → "{mfg} — {model}" → node_id_hex.
pub fn node_display_name(node: &lcc_rs::DiscoveredNode) -> String {
    if let Some(snip) = &node.snip_data {
        if !snip.user_name.is_empty() {
            return snip.user_name.clone();
        }
        if !snip.manufacturer.is_empty() || !snip.model.is_empty() {
            let mfg = snip.manufacturer.trim();
            let mdl = snip.model.trim();
            if !mfg.is_empty() && !mdl.is_empty() {
                return format!("{mfg} — {mdl}");
            } else if !mdl.is_empty() {
                return mdl.to_string();
            }
        }
    }
    node.node_id.to_hex_string()
}

/// Slot metadata gathered from a single CDI walk.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SlotInfo {
    pub node_key: NodeKey,
    pub node_name: String,
    pub element_path: Vec<String>,
    /// Raw CDI <description> text, preserved for forwarding to frontend.
    pub element_description: Option<String>,
    pub heuristic_role: lcc_rs::EventRole,
}

/// Pre-walk all CDI event slots for a node and return a list of slot infos.
///
/// If the node has no CDI data (or parsing fails) an empty vec is returned.
pub fn walk_cdi_slots(node: &lcc_rs::DiscoveredNode) -> Vec<SlotInfo> {
    let cdi_xml = match node.cdi.as_ref().map(|d| d.xml_content.as_str()) {
        Some(xml) if !xml.is_empty() => xml,
        _ => return Vec::new(),
    };

    let cdi = match lcc_rs::cdi::parser::parse_cdi(cdi_xml) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut slots = Vec::new();
    let node_key = NodeKey::from_node_id(node.node_id);
    let node_name = node_display_name(node);

    lcc_rs::walk_event_slots(&cdi, |element, ancestor_names, path| {
        let role = lcc_rs::classify_event_slot(element, ancestor_names);
        slots.push(SlotInfo {
            node_key,
            node_name: node_name.clone(),
            element_path: path.to_vec(),
            element_description: element.description.clone(),
            heuristic_role: role,
        });
    });

    slots
}

/// Find the first slot from `slots` whose heuristic role matches `expected_role`,
/// falling back to any slot if none matches.
pub fn best_slot<'a>(slots: &'a [SlotInfo], expected_role: lcc_rs::EventRole) -> Option<&'a SlotInfo> {
    slots
        .iter()
        .find(|s| s.heuristic_role == expected_role)
        .or_else(|| slots.first())
}

/// Find the slot for a specific event ID by first checking the config value cache
/// (precise match on which slot actually holds that event ID), then falling back
/// to the heuristic `best_slot` if no cache entry exists for this slot.
///
/// `config_cache` is keyed by NodeKey → (path → bytes).
pub fn slot_for_event_id<'a>(
    slots: &'a [SlotInfo],
    node_key: &NodeKey,
    event_id_bytes: &[u8; 8],
    config_cache: &std::collections::HashMap<NodeKey, std::collections::HashMap<String, [u8; 8]>>,
    fallback_role: lcc_rs::EventRole,
) -> Option<&'a SlotInfo> {
    // Precise lookup: which slot on this node was actually configured with this event ID?
    if let Some(node_cache) = config_cache.get(node_key) {
        for slot in slots {
            let path_key = slot.element_path.join("/");
            if let Some(&cached_bytes) = node_cache.get(&path_key) {
                if &cached_bytes == event_id_bytes {
                    return Some(slot);
                }
            }
        }
    }
    // Fallback: heuristic role classification.
    best_slot(slots, fallback_role)
}

/// Parse an event ID hex string (dotted or contiguous) into 8 bytes.
pub fn parse_event_id_hex(hex: &str) -> Option<[u8; 8]> {
    crate::node_tree::parse_event_id_hex(hex)
}

// ── Core builder ─────────────────────────────────────────────────────────────

/// Build the complete bowtie catalog from discovered nodes and the protocol-level
/// event roles returned by `query_event_roles`.
///
/// **Algorithm**
/// 1. Pre-walk every node's CDI to collect slot metadata keyed by node ID.
/// 2. For each event ID where the Identify Events exchange found ≥1 node producing AND ≥1 consuming:
///    a. Pure producers (only in producer set) → `EventSlotEntry { role: Producer }`.
///    b. Pure consumers (only in consumer set) → `EventSlotEntry { role: Consumer }`.
///    c. Same-node entries (node replied both ProducerIdentified and ConsumerIdentified):
///       - If `profile_group_roles` contains an entry for a matching slot, route that slot
///         directly to `producers` or `consumers` (Producer/Consumer role) without ambiguity.
///       - Otherwise, if `config_value_cache` has slots for this node that match this event ID,
///         emit one `EventSlotEntry` per matching slot as Ambiguous.
///       - Otherwise (cache empty or no hits), fall back to a CDI vote-tally across all slots
///         and emit one entry for the node.
/// 3. Emit a `BowtieCard` when `producers + consumers + ambiguous_entries ≥ 2` total entries.
///    A single-entry event carries no connection information and is silently excluded.
///    Cards with only ambiguous entries are valid — the protocol confirmed the node is on both
///    sides; the user can clarify roles in a future flow.
/// 4. Sort bowties by `event_id_bytes`.
///
/// `profile_group_roles` — optional map keyed by `"{node_id}:{element_path.join("/")}"` →
/// `EventRole`.  Built from the annotated `NodeConfigTree`s after `annotate_tree` runs.
/// Pass `None` when no profiles are loaded or in unit-test contexts.
pub fn build_bowtie_catalog(
    nodes: &[lcc_rs::DiscoveredNode],
    event_roles: &HashMap<[u8; 8], NodeRoles>,
    config_value_cache: &HashMap<NodeKey, HashMap<String, [u8; 8]>>,
    profile_group_roles: Option<&HashMap<String, lcc_rs::EventRole>>,
) -> BowtieCatalog {
    let build_start = std::time::Instant::now();
    let built_at = chrono::Utc::now().to_rfc3339();
    let source_node_count = nodes.len();

    // Pre-walk: build a map from NodeKey → Vec<SlotInfo>
    let mut slot_map: HashMap<NodeKey, Vec<SlotInfo>> = HashMap::new();
    for node in nodes {
        let slots = walk_cdi_slots(node);
        if !slots.is_empty() {
            slot_map.insert(NodeKey::from_node_id(node.node_id), slots);
        }
    }

    let total_slots_scanned: usize = slot_map.values().map(|v| v.len()).sum();

    let mut bowties: Vec<BowtieCard> = Vec::new();

    // Build a set of well-known event IDs for O(1) skip in the protocol loop.
    let well_known_set: std::collections::HashSet<[u8; 8]> =
        WELL_KNOWN_EVENT_IDS.iter().map(|&(bytes, _)| bytes).collect();

    // ── Config-primary event discovery ───────────────────────────────────────────
    let mut config_slot_map: HashMap<[u8; 8], Vec<(NodeKey, String)>> = HashMap::new();
    for (node_key, node_cache) in config_value_cache {
        for (path_key, &event_bytes) in node_cache {
            if is_placeholder_event_id(&event_bytes) || well_known_set.contains(&event_bytes) {
                continue;
            }
            config_slot_map
                .entry(event_bytes)
                .or_default()
                .push((*node_key, path_key.clone()));
        }
    }

    // Step 2 — union: include protocol-only events not already in config_slot_map.
    let mut all_event_ids: std::collections::HashSet<[u8; 8]> =
        config_slot_map.keys().copied().collect();
    for event_id_bytes in event_roles.keys() {
        if !is_placeholder_event_id(event_id_bytes) && !well_known_set.contains(event_id_bytes) {
            all_event_ids.insert(*event_id_bytes);
        }
    }

    // Step 3 — build a BowtieCard for each eligible event.
    for event_id_bytes in &all_event_ids {
        let config_slots = config_slot_map.get(event_id_bytes);
        let config_slot_count = config_slots.map(|v| v.len()).unwrap_or(0);
        let roles_opt = event_roles.get(event_id_bytes);

        let event_id_hex = format!(
            "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
            event_id_bytes[0], event_id_bytes[1], event_id_bytes[2], event_id_bytes[3],
            event_id_bytes[4], event_id_bytes[5], event_id_bytes[6], event_id_bytes[7]
        );

        let mut producers: Vec<EventSlotEntry> = Vec::new();
        let mut consumers: Vec<EventSlotEntry> = Vec::new();
        let mut ambiguous_entries: Vec<EventSlotEntry> = Vec::new();

        if config_slot_count > 0 {
            // ── Config-primary path ───────────────────────────────────────────
            let slot_refs = config_slots.unwrap();

            let mut node_slot_count: HashMap<NodeKey, usize> = HashMap::new();
            for (node_key, _) in slot_refs {
                *node_slot_count.entry(*node_key).or_insert(0) += 1;
            }

            for (node_key, path_key) in slot_refs {
                let slots = slot_map.get(node_key).map(|s| s.as_slice()).unwrap_or(&[]);
                let node_name = nodes
                    .iter()
                    .find(|n| NodeKey::from_node_id(n.node_id) == *node_key)
                    .map(node_display_name)
                    .unwrap_or_else(|| node_key.to_string());

                let slot = slots.iter().find(|s| s.element_path.join("/") == *path_key);
                let (ep, ed, heuristic) = slot
                    .map(|s| (
                        s.element_path.clone(),
                        s.element_description.clone(),
                        s.heuristic_role,
                    ))
                    .unwrap_or_else(|| (
                        path_key.split('/').map(|s| s.to_string()).collect(),
                        None,
                        lcc_rs::EventRole::Ambiguous,
                    ));

                let multi_slot_node =
                    node_slot_count.get(node_key).copied().unwrap_or(1) > 1;
                let node_in_producers = !multi_slot_node
                    && roles_opt.map(|r| r.producers.contains(node_key)).unwrap_or(false);
                let node_in_consumers = !multi_slot_node
                    && roles_opt.map(|r| r.consumers.contains(node_key)).unwrap_or(false);

                let resolved_role = match (node_in_producers, node_in_consumers) {
                    (true, false) => lcc_rs::EventRole::Producer,
                    (false, true) => lcc_rs::EventRole::Consumer,
                    _ => {
                        let profile_key = format!("{}:{}", node_key, path_key);
                        profile_group_roles
                            .and_then(|map| map.get(&profile_key))
                            .copied()
                            .unwrap_or(heuristic)
                    }
                };

                let entry = EventSlotEntry {
                    node_key: *node_key,
                    node_name,
                    element_path: ep,
                    element_description: ed,
                    event_id: *event_id_bytes,
                    role: resolved_role,
                };
                match resolved_role {
                    lcc_rs::EventRole::Producer => producers.push(entry),
                    lcc_rs::EventRole::Consumer => consumers.push(entry),
                    lcc_rs::EventRole::Ambiguous => ambiguous_entries.push(entry),
                }
            }

            // Include protocol-confirmed nodes that have no config slot for this event.
            let config_nodes: std::collections::HashSet<&NodeKey> =
                slot_refs.iter().map(|(nk, _)| nk).collect();

            if let Some(roles) = roles_opt {
                for node_key in roles.producers.difference(&roles.consumers) {
                    if config_nodes.contains(node_key) { continue; }
                    let slots = slot_map.get(node_key).map(|s| s.as_slice()).unwrap_or(&[]);
                    let slot = slot_for_event_id(
                        slots, node_key, event_id_bytes, config_value_cache,
                        lcc_rs::EventRole::Producer,
                    );
                    let (ep, ed) = slot
                        .map(|s| (s.element_path.clone(), s.element_description.clone()))
                        .unwrap_or_else(|| (vec![], None));
                    let node_name = nodes
                        .iter()
                        .find(|n| NodeKey::from_node_id(n.node_id) == *node_key)
                        .map(node_display_name)
                        .unwrap_or_else(|| node_key.to_string());
                    producers.push(EventSlotEntry {
                        node_key: *node_key,
                        node_name,
                        element_path: ep,
                        element_description: ed,
                        event_id: *event_id_bytes,
                        role: lcc_rs::EventRole::Producer,
                    });
                }
                for node_key in roles.consumers.difference(&roles.producers) {
                    if config_nodes.contains(node_key) { continue; }
                    let slots = slot_map.get(node_key).map(|s| s.as_slice()).unwrap_or(&[]);
                    let slot = slot_for_event_id(
                        slots, node_key, event_id_bytes, config_value_cache,
                        lcc_rs::EventRole::Consumer,
                    );
                    let (ep, ed) = slot
                        .map(|s| (s.element_path.clone(), s.element_description.clone()))
                        .unwrap_or_else(|| (vec![], None));
                    let node_name = nodes
                        .iter()
                        .find(|n| NodeKey::from_node_id(n.node_id) == *node_key)
                        .map(node_display_name)
                        .unwrap_or_else(|| node_key.to_string());
                    consumers.push(EventSlotEntry {
                        node_key: *node_key,
                        node_name,
                        element_path: ep,
                        element_description: ed,
                        event_id: *event_id_bytes,
                        role: lcc_rs::EventRole::Consumer,
                    });
                }
                for node_key in roles.producers.intersection(&roles.consumers) {
                    if config_nodes.contains(node_key) { continue; }
                    let slots = slot_map.get(node_key).map(|s| s.as_slice()).unwrap_or(&[]);
                    let node_name = nodes
                        .iter()
                        .find(|n| NodeKey::from_node_id(n.node_id) == *node_key)
                        .map(node_display_name)
                        .unwrap_or_else(|| node_key.to_string());
                    let slot = slots.first();
                    let (ep, ed) = slot
                        .map(|s| (s.element_path.clone(), s.element_description.clone()))
                        .unwrap_or_else(|| (vec![], None));
                    ambiguous_entries.push(EventSlotEntry {
                        node_key: *node_key,
                        node_name,
                        element_path: ep,
                        element_description: ed,
                        event_id: *event_id_bytes,
                        role: lcc_rs::EventRole::Ambiguous,
                    });
                }
            }

            let total_entries =
                producers.len() + consumers.len() + ambiguous_entries.len();
            if total_entries == 0 {
                continue;
            }
        } else {
            // ── Protocol-only path ────────────────────────────────────────────
            let roles = match roles_opt {
                Some(r) if !r.producers.is_empty() && !r.consumers.is_empty() => r,
                _ => continue,
            };

            let both: std::collections::HashSet<&NodeKey> =
                roles.producers.intersection(&roles.consumers).collect();

            for node_key in roles.producers.difference(&roles.consumers) {
                let slots = slot_map.get(node_key).map(|s| s.as_slice()).unwrap_or(&[]);
                let slot = slot_for_event_id(
                    slots, node_key, event_id_bytes, config_value_cache,
                    lcc_rs::EventRole::Producer,
                );
                let (ep, ed) = slot
                    .map(|s| (s.element_path.clone(), s.element_description.clone()))
                    .unwrap_or_else(|| (vec![], None));
                let node_name = nodes
                    .iter()
                    .find(|n| NodeKey::from_node_id(n.node_id) == *node_key)
                    .map(node_display_name)
                    .unwrap_or_else(|| node_key.to_string());
                producers.push(EventSlotEntry {
                    node_key: *node_key,
                    node_name,
                    element_path: ep,
                    element_description: ed,
                    event_id: *event_id_bytes,
                    role: lcc_rs::EventRole::Producer,
                });
            }

            for node_key in roles.consumers.difference(&roles.producers) {
                let slots = slot_map.get(node_key).map(|s| s.as_slice()).unwrap_or(&[]);
                let slot = slot_for_event_id(
                    slots, node_key, event_id_bytes, config_value_cache,
                    lcc_rs::EventRole::Consumer,
                );
                let (ep, ed) = slot
                    .map(|s| (s.element_path.clone(), s.element_description.clone()))
                    .unwrap_or_else(|| (vec![], None));
                let node_name = nodes
                    .iter()
                    .find(|n| NodeKey::from_node_id(n.node_id) == *node_key)
                    .map(node_display_name)
                    .unwrap_or_else(|| node_key.to_string());
                consumers.push(EventSlotEntry {
                    node_key: *node_key,
                    node_name,
                    element_path: ep,
                    element_description: ed,
                    event_id: *event_id_bytes,
                    role: lcc_rs::EventRole::Consumer,
                });
            }

            for node_key in &both {
                let slots = slot_map.get(*node_key).map(|s| s.as_slice()).unwrap_or(&[]);
                let node_name = nodes
                    .iter()
                    .find(|n| NodeKey::from_node_id(n.node_id) == **node_key)
                    .map(node_display_name)
                    .unwrap_or_else(|| node_key.to_string());
                let slot = slots.first();
                let (ep, ed) = slot
                    .map(|s| (s.element_path.clone(), s.element_description.clone()))
                    .unwrap_or_else(|| (vec![], None));
                let profile_key = slot
                    .map(|s| format!("{}:{}", node_key, s.element_path.join("/")))
                    .unwrap_or_else(|| node_key.to_string());
                let resolved = profile_group_roles
                    .and_then(|map| map.get(&profile_key))
                    .copied()
                    .unwrap_or(lcc_rs::EventRole::Ambiguous);
                let entry = EventSlotEntry {
                    node_key: **node_key,
                    node_name,
                    element_path: ep,
                    element_description: ed,
                    event_id: *event_id_bytes,
                    role: resolved,
                };
                match resolved {
                    lcc_rs::EventRole::Producer => producers.push(entry),
                    lcc_rs::EventRole::Consumer => consumers.push(entry),
                    lcc_rs::EventRole::Ambiguous => ambiguous_entries.push(entry),
                }
            }

            let total_entries = producers.len() + consumers.len() + ambiguous_entries.len();
            if total_entries < 2 {
                continue;
            }
        }

        // Shared card-emission code (both paths reach here).
        let state = if !producers.is_empty() && !consumers.is_empty() {
            BowtieState::Active
        } else if !producers.is_empty() || !consumers.is_empty() || !ambiguous_entries.is_empty() {
            BowtieState::Incomplete
        } else {
            BowtieState::Planning
        };

        bowties.push(BowtieCard {
            event_id_hex,
            event_id_bytes: *event_id_bytes,
            producers,
            consumers,
            ambiguous_entries,
            name: None,
            tags: Vec::new(),
            state,
        });
    }

    // Well-known events: build cards solely from config_value_cache.
    for &(wk_bytes, wk_name) in WELL_KNOWN_EVENT_IDS {
        let event_id_hex = format!(
            "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
            wk_bytes[0], wk_bytes[1], wk_bytes[2], wk_bytes[3],
            wk_bytes[4], wk_bytes[5], wk_bytes[6], wk_bytes[7]
        );

        let mut wk_producers: Vec<EventSlotEntry> = Vec::new();
        let mut wk_consumers: Vec<EventSlotEntry> = Vec::new();
        let mut wk_ambiguous: Vec<EventSlotEntry> = Vec::new();

        for (node_key, node_cache) in config_value_cache {
            let slots = slot_map.get(node_key).map(|s| s.as_slice()).unwrap_or(&[]);
            let node_name = nodes
                .iter()
                .find(|n| NodeKey::from_node_id(n.node_id) == *node_key)
                .map(node_display_name)
                .unwrap_or_else(|| node_key.to_string());

            for (path_key, &cached_bytes) in node_cache {
                if cached_bytes != wk_bytes {
                    continue;
                }

                let slot = slots.iter().find(|s| s.element_path.join("/") == *path_key);
                let (ep, ed, heuristic) = slot
                    .map(|s| (
                        s.element_path.clone(),
                        s.element_description.clone(),
                        s.heuristic_role,
                    ))
                    .unwrap_or_else(|| (
                        path_key.split('/').map(|s| s.to_string()).collect(),
                        None,
                        lcc_rs::EventRole::Ambiguous,
                    ));

                let profile_key = format!("{}:{}", node_key, path_key);
                let resolved_role = profile_group_roles
                    .and_then(|map| map.get(&profile_key))
                    .copied()
                    .unwrap_or(heuristic);

                match resolved_role {
                    lcc_rs::EventRole::Producer => {
                        wk_producers.push(EventSlotEntry {
                            node_key: *node_key,
                            node_name: node_name.clone(),
                            element_path: ep,
                            element_description: ed,
                            event_id: wk_bytes,
                            role: lcc_rs::EventRole::Producer,
                        });
                    }
                    lcc_rs::EventRole::Consumer => {
                        wk_consumers.push(EventSlotEntry {
                            node_key: *node_key,
                            node_name: node_name.clone(),
                            element_path: ep,
                            element_description: ed,
                            event_id: wk_bytes,
                            role: lcc_rs::EventRole::Consumer,
                        });
                    }
                    lcc_rs::EventRole::Ambiguous => {
                        wk_ambiguous.push(EventSlotEntry {
                            node_key: *node_key,
                            node_name: node_name.clone(),
                            element_path: ep,
                            element_description: ed,
                            event_id: wk_bytes,
                            role: lcc_rs::EventRole::Ambiguous,
                        });
                    }
                }
            }
        }

        let wk_total = wk_producers.len() + wk_consumers.len() + wk_ambiguous.len();
        if wk_total == 0 {
            continue;
        }

        let wk_state = if !wk_producers.is_empty() && !wk_consumers.is_empty() {
            BowtieState::Active
        } else if !wk_producers.is_empty() || !wk_consumers.is_empty() || !wk_ambiguous.is_empty() {
            BowtieState::Incomplete
        } else {
            BowtieState::Planning
        };

        bowties.push(BowtieCard {
            event_id_hex,
            event_id_bytes: wk_bytes,
            producers: wk_producers,
            consumers: wk_consumers,
            ambiguous_entries: wk_ambiguous,
            name: Some(wk_name.to_string()),
            tags: Vec::new(),
            state: wk_state,
        });
    }

    // Sort by event_id_bytes for stable output.
    bowties.sort_by_key(|b| b.event_id_bytes);

    let elapsed_ms = build_start.elapsed().as_millis();
    eprintln!(
        "[bowties][INFO] catalog built in {}ms: {} bowties from {} nodes, {} slots scanned",
        elapsed_ms,
        bowties.len(),
        source_node_count,
        total_slots_scanned
    );

    BowtieCatalog {
        bowties,
        built_at,
        source_node_count,
        total_slots_scanned,
    }
}

/// Merge layout file metadata onto an existing bowtie catalog.
///
/// This enriches discovered bowties with user-assigned names, tags, and role
/// classifications from the layout YAML. It also creates planning-state cards
/// for layout entries that don't match any discovered event ID.
pub fn merge_layout_metadata(
    catalog: &mut BowtieCatalog,
    layout: &crate::layout::types::LayoutFile,
) {
    // Build a lookup for fast event-ID → card-index matching
    let mut hex_to_idx: HashMap<String, usize> = HashMap::new();
    for (i, card) in catalog.bowties.iter().enumerate() {
        hex_to_idx.insert(card.event_id_hex.clone(), i);
    }

    // Merge names and tags onto matching cards
    for (event_id_hex, meta) in &layout.bowties {
        if let Some(&idx) = hex_to_idx.get(event_id_hex) {
            let card = &mut catalog.bowties[idx];
            if meta.name.is_some() {
                card.name = meta.name.clone();
            }
            if !meta.tags.is_empty() {
                card.tags = meta.tags.clone();
            }
        } else {
            // Create planning-state card for unmatched layout entry.
            let upper = event_id_hex.to_uppercase();
            let (hex_used, bytes) = if let Some(b) = parse_event_id_hex(&upper) {
                (upper, b)
            } else {
                (event_id_hex.clone(), [0u8; 8])
            };
            catalog.bowties.push(BowtieCard {
                event_id_hex: hex_used,
                event_id_bytes: bytes,
                producers: Vec::new(),
                consumers: Vec::new(),
                ambiguous_entries: Vec::new(),
                name: meta.name.clone(),
                tags: meta.tags.clone(),
                state: BowtieState::Planning,
            });
        }
    }

    // Merge role classifications: reclassify ambiguous entries based on user input.
    for card in &mut catalog.bowties {
        let mut still_ambiguous = Vec::new();
        for entry in card.ambiguous_entries.drain(..) {
            let class_key = format!("{}:{}", entry.node_key, entry.element_path.join("/"));
            if let Some(rc) = layout.role_classifications.get(&class_key) {
                let mut classified = entry;
                match rc.role.as_str() {
                    "Producer" => {
                        classified.role = lcc_rs::EventRole::Producer;
                        card.producers.push(classified);
                    }
                    "Consumer" => {
                        classified.role = lcc_rs::EventRole::Consumer;
                        card.consumers.push(classified);
                    }
                    _ => still_ambiguous.push(classified),
                }
            } else {
                still_ambiguous.push(entry);
            }
        }
        card.ambiguous_entries = still_ambiguous;

        // Recompute state after reclassification
        card.state = if !card.producers.is_empty() && !card.consumers.is_empty() {
            BowtieState::Active
        } else if !card.producers.is_empty() || !card.consumers.is_empty() || !card.ambiguous_entries.is_empty() {
            BowtieState::Incomplete
        } else {
            BowtieState::Planning
        };
    }

    // Re-sort after adding planning cards
    catalog.bowties.sort_by_key(|b| b.event_id_bytes);
}

/// Extract all non-ambiguous event role classifications from a live catalog for
/// persistence into the layout file's `role_classifications` map.
///
/// Produces a `"{nodeId}:{element_path_joined_by_/}"` key for every producer
/// and consumer entry.  Ambiguous entries are intentionally excluded — they are
/// not persisted and will remain ambiguous on reopen until the user resolves them.
pub fn extract_catalog_role_classifications(
    catalog: &BowtieCatalog,
) -> std::collections::BTreeMap<String, crate::layout::types::RoleClassification> {
    let mut map = std::collections::BTreeMap::new();
    for card in &catalog.bowties {
        for entry in &card.producers {
            let key = format!("{}:{}", entry.node_key, entry.element_path.join("/"));
            map.insert(key, crate::layout::types::RoleClassification { role: "Producer".to_string() });
        }
        for entry in &card.consumers {
            let key = format!("{}:{}", entry.node_key, entry.element_path.join("/"));
            map.insert(key, crate::layout::types::RoleClassification { role: "Consumer".to_string() });
        }
        // ambiguous_entries — intentionally not extracted
    }
    map
}

// ── Payload ───────────────────────────────────────────────────────────────────

/// Payload emitted with the `cdi-read-complete` Tauri event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiReadCompletePayload {
    /// Freshly-built catalog.
    pub catalog: BowtieCatalog,
    /// Number of nodes that were included in the build.
    pub node_count: usize,
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ── Helpers ──────────────────────────────────────────────────────────────

    const EVENT_A: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    const EVENT_B: [u8; 8] = [0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];

    fn make_nodes(count: usize) -> Vec<lcc_rs::DiscoveredNode> {
        (0..count)
            .map(|i| lcc_rs::DiscoveredNode {
                node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, i as u8]),
                alias: lcc_rs::NodeAlias::new(0x100 + i as u16).unwrap(),
                snip_data: None,
                snip_status: lcc_rs::types::SNIPStatus::Unknown,
                connection_status: lcc_rs::types::ConnectionStatus::Unknown,
                last_verified: None,
                last_seen: chrono::Utc::now(),
                cdi: None,
                pip_flags: None,
                pip_status: lcc_rs::types::PIPStatus::Unknown,
            })
            .collect()
    }

    fn node_key(node_index: usize) -> NodeKey {
        NodeKey::from_node_id(lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, node_index as u8]))
    }

    fn roles(producers: &[usize], consumers: &[usize]) -> NodeRoles {
        NodeRoles {
            producers: producers.iter().map(|&i| node_key(i)).collect(),
            consumers: consumers.iter().map(|&i| node_key(i)).collect(),
        }
    }

    // ── T006 test cases ───────────────────────────────────────────────────────

    #[test]
    fn t006a_one_producer_one_consumer_gives_one_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.source_node_count, 2);
        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.event_id_bytes, EVENT_A);
        assert_eq!(card.producers.len(), 1);
        assert_eq!(card.consumers.len(), 1);
        assert!(card.ambiguous_entries.is_empty());
        assert_eq!(card.producers[0].role, lcc_rs::EventRole::Producer);
        assert_eq!(card.consumers[0].role, lcc_rs::EventRole::Consumer);
    }

    #[test]
    fn t006b_two_producers_one_consumer() {
        let nodes = make_nodes(3);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0, 1], &[2]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.producers.len(), 2);
        assert_eq!(card.consumers.len(), 1);
        assert!(card.ambiguous_entries.is_empty());
    }

    #[test]
    fn t006c_same_node_both_sides_heuristic_resolves() {
        let nodes = make_nodes(3);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[0, 1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert!(card.producers.is_empty());
        assert_eq!(card.consumers.len(), 1);
        assert_eq!(card.ambiguous_entries.len(), 1);
    }

    #[test]
    fn t006c_same_node_resolved_by_cdi_heuristic() {
        use lcc_rs::types::{SNIPStatus, ConnectionStatus};

        let cdi_xml = r#"<?xml version="1.0"?>
<cdi xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="http://openlcb.org/schema/cdi/1/1/cdi.xsd">
  <segment space="253" origin="0">
    <group>
      <name>Producers</name>
      <eventid><name>Output A</name></eventid>
      <eventid><name>Output B</name></eventid>
    </group>
    <group>
      <name>Consumers</name>
      <eventid><name>Input A</name></eventid>
    </group>
  </segment>
</cdi>"#;

        let node0 = lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x00]),
            alias: lcc_rs::NodeAlias::new(0x100).unwrap(),
            snip_data: None,
            snip_status: SNIPStatus::Unknown,
            connection_status: ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: Some(lcc_rs::types::CdiData {
                xml_content: cdi_xml.to_string(),
                retrieved_at: chrono::Utc::now(),
            }),
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        };
        let node1 = lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x01]),
            alias: lcc_rs::NodeAlias::new(0x101).unwrap(),
            snip_data: None,
            snip_status: SNIPStatus::Unknown,
            connection_status: ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: None,
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        };

        let nodes = vec![node0, node1];
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, NodeRoles {
            producers: [node_key(0)].iter().cloned().collect(),
            consumers: [node_key(0), node_key(1)].iter().cloned().collect(),
        });

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert!(card.producers.is_empty());
        assert_eq!(card.consumers.len(), 1);
        assert_eq!(card.ambiguous_entries.len(), 1);
    }

    #[test]
    fn t006d_same_node_heuristic_inconclusive_goes_to_ambiguous() {
        use lcc_rs::types::{SNIPStatus, ConnectionStatus};

        let cdi_xml = r#"<?xml version="1.0"?>
<cdi xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="http://openlcb.org/schema/cdi/1/1/cdi.xsd">
  <segment space="253" origin="0">
    <group>
      <name>Producers</name>
      <eventid><name>Output A</name></eventid>
    </group>
    <group>
      <name>Consumers</name>
      <eventid><name>Input A</name></eventid>
    </group>
  </segment>
</cdi>"#;

        let node0 = lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x00]),
            alias: lcc_rs::NodeAlias::new(0x100).unwrap(),
            snip_data: None,
            snip_status: SNIPStatus::Unknown,
            connection_status: ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: Some(lcc_rs::types::CdiData {
                xml_content: cdi_xml.to_string(),
                retrieved_at: chrono::Utc::now(),
            }),
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        };
        let node1 = lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x01]),
            alias: lcc_rs::NodeAlias::new(0x101).unwrap(),
            snip_data: None,
            snip_status: SNIPStatus::Unknown,
            connection_status: ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: None,
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        };

        let nodes = vec![node0, node1];
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, NodeRoles {
            producers: [node_key(0)].iter().cloned().collect(),
            consumers: [node_key(0), node_key(1)].iter().cloned().collect(),
        });

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert!(card.producers.is_empty());
        assert_eq!(card.consumers.len(), 1);
        assert_eq!(card.ambiguous_entries.len(), 1);
        assert_eq!(card.ambiguous_entries[0].role, lcc_rs::EventRole::Ambiguous);
    }

    #[test]
    fn catalog_includes_event_confirmed_by_config_cache_when_identify_events_returns_no_roles() {
        let nodes = make_nodes(2);
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        let mut config_cache: HashMap<NodeKey, HashMap<String, [u8; 8]>> = HashMap::new();
        let mut node0_cache: HashMap<String, [u8; 8]> = HashMap::new();
        node0_cache.insert("seg:0/elem:0".to_string(), EVENT_A);
        config_cache.insert(node_key(0), node0_cache);
        let mut node1_cache: HashMap<String, [u8; 8]> = HashMap::new();
        node1_cache.insert("seg:0/elem:1".to_string(), EVENT_A);
        config_cache.insert(node_key(1), node1_cache);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1);
        assert_eq!(catalog.bowties[0].event_id_bytes, EVENT_A);
        let total = catalog.bowties[0].producers.len()
            + catalog.bowties[0].consumers.len()
            + catalog.bowties[0].ambiguous_entries.len();
        assert_eq!(total, 2);
    }

    #[test]
    fn t006e_producers_only_no_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0, 1], &[]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 0);
    }

    #[test]
    fn t006f_zero_nodes_empty_catalog() {
        let nodes: Vec<lcc_rs::DiscoveredNode> = Vec::new();
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.source_node_count, 0);
        assert_eq!(catalog.bowties.len(), 0);
        assert_eq!(catalog.total_slots_scanned, 0);
    }

    #[test]
    fn t006g_same_event_id_exactly_one_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        let unique_event_ids: HashSet<[u8; 8]> =
            catalog.bowties.iter().map(|c| c.event_id_bytes).collect();
        assert_eq!(unique_event_ids.len(), catalog.bowties.len());
    }

    #[test]
    fn t006_bowties_sorted_by_event_id() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_B, roles(&[0], &[1]));
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 2);
        assert!(catalog.bowties[0].event_id_bytes <= catalog.bowties[1].event_id_bytes);
    }

    #[test]
    fn t006_entry_role_invariant() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);
        for card in &catalog.bowties {
            for entry in &card.producers {
                assert_ne!(entry.role, lcc_rs::EventRole::Ambiguous);
            }
            for entry in &card.consumers {
                assert_ne!(entry.role, lcc_rs::EventRole::Ambiguous);
            }
        }
    }

    #[test]
    fn t006_single_ambiguous_entry_filtered() {
        let nodes = make_nodes(1);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, NodeRoles {
            producers: [node_key(0)].iter().cloned().collect(),
            consumers: [node_key(0)].iter().cloned().collect(),
        });

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 0);
    }

    #[test]
    fn single_config_slot_creates_card_for_role_persistence() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[]));

        let mut config_cache: HashMap<NodeKey, HashMap<String, [u8; 8]>> = HashMap::new();
        let mut node0_cache: HashMap<String, [u8; 8]> = HashMap::new();
        node0_cache.insert("seg:0/elem:0".to_string(), EVENT_A);
        config_cache.insert(node_key(0), node0_cache);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.producers.len(), 1);
        assert_eq!(card.consumers.len(), 0);
        assert_eq!(card.state, BowtieState::Incomplete);
    }

    #[test]
    fn build_bowtie_catalog_uses_profile_roles() {
        use lcc_rs::types::{SNIPStatus, ConnectionStatus};

        let cdi_xml = r#"<?xml version="1.0"?>
<cdi xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
     xsi:noNamespaceSchemaLocation="http://openlcb.org/schema/cdi/1/1/cdi.xsd">
  <segment space="253" origin="0">
    <group>
      <name>Output</name>
      <eventid><name>Trigger Event</name></eventid>
    </group>
  </segment>
</cdi>"#;

        let node0 = lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x00]),
            alias: lcc_rs::NodeAlias::new(0x100).unwrap(),
            snip_data: None,
            snip_status: SNIPStatus::Unknown,
            connection_status: ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: Some(lcc_rs::types::CdiData {
                xml_content: cdi_xml.to_string(),
                retrieved_at: chrono::Utc::now(),
            }),
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        };
        let node1 = lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x01]),
            alias: lcc_rs::NodeAlias::new(0x101).unwrap(),
            snip_data: None,
            snip_status: SNIPStatus::Unknown,
            connection_status: ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: None,
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        };

        let nodes = vec![node0, node1];
        let mut event_roles_map = HashMap::new();
        event_roles_map.insert(EVENT_A, NodeRoles {
            producers: [node_key(0)].iter().cloned().collect(),
            consumers: [node_key(0), node_key(1)].iter().cloned().collect(),
        });

        let slot_path = "seg:0/elem:0/elem:0".to_string();
        let mut node_cache = HashMap::new();
        node_cache.insert(slot_path.clone(), EVENT_A);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_key(0), node_cache);

        let profile_key = format!("{}:{}", node_key(0), slot_path);
        let mut profile_roles: HashMap<String, lcc_rs::EventRole> = HashMap::new();
        profile_roles.insert(profile_key, lcc_rs::EventRole::Producer);

        let catalog = build_bowtie_catalog(&nodes, &event_roles_map, &config_cache, Some(&profile_roles));

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.producers.len(), 1);
        assert_eq!(card.consumers.len(), 1);
        assert!(card.ambiguous_entries.is_empty());
        assert_eq!(card.producers[0].role, lcc_rs::EventRole::Producer);
        assert_eq!(card.producers[0].node_key, node_key(0));
    }

    // ── node_display_name ──────────────────────────────────────────────────────

    fn make_snip(user_name: &str, manufacturer: &str, model: &str) -> lcc_rs::SNIPData {
        lcc_rs::SNIPData {
            manufacturer: manufacturer.to_string(),
            model: model.to_string(),
            hardware_version: String::new(),
            software_version: String::new(),
            user_name: user_name.to_string(),
            user_description: String::new(),
        }
    }

    fn make_node_with_snip(index: usize, snip: lcc_rs::SNIPData) -> lcc_rs::DiscoveredNode {
        lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, index as u8]),
            alias: lcc_rs::NodeAlias::new(0x100 + index as u16).unwrap(),
            snip_data: Some(snip),
            snip_status: lcc_rs::types::SNIPStatus::Complete,
            connection_status: lcc_rs::types::ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: None,
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        }
    }

    #[test]
    fn node_display_name_user_name_wins() {
        let node = make_node_with_snip(0, make_snip("My Node", "Acme", "XYZ-100"));
        assert_eq!(node_display_name(&node), "My Node");
    }

    #[test]
    fn node_display_name_mfg_and_model() {
        let node = make_node_with_snip(0, make_snip("", "Acme", "Widget-100"));
        assert_eq!(node_display_name(&node), "Acme — Widget-100");
    }

    #[test]
    fn node_display_name_model_only() {
        let node = make_node_with_snip(0, make_snip("", "", "Widget-100"));
        assert_eq!(node_display_name(&node), "Widget-100");
    }

    #[test]
    fn node_display_name_no_snip_uses_node_id() {
        let node = make_nodes(1).into_iter().next().unwrap();
        let expected = lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x00]).to_hex_string();
        assert_eq!(node_display_name(&node), expected);
    }

    #[test]
    fn node_display_name_empty_user_name_falls_through_to_mfg_model() {
        let node = make_node_with_snip(0, make_snip("", "Mfg Co", "Model X"));
        assert_eq!(node_display_name(&node), "Mfg Co — Model X");
    }

    // ── best_slot ──────────────────────────────────────────────────────────────

    fn make_slot(path: &str, role: lcc_rs::EventRole) -> SlotInfo {
        SlotInfo {
            node_key: node_key(0),
            node_name: "Test Node".to_string(),
            element_path: vec![path.to_string()],
            element_description: None,
            heuristic_role: role,
        }
    }

    #[test]
    fn best_slot_exact_role_match_returned() {
        let slots = vec![
            make_slot("input", lcc_rs::EventRole::Consumer),
            make_slot("output", lcc_rs::EventRole::Producer),
        ];
        let result = best_slot(&slots, lcc_rs::EventRole::Producer);
        assert_eq!(result.unwrap().element_path, vec!["output"]);
    }

    #[test]
    fn best_slot_fallback_to_first_when_no_match() {
        let slots = vec![
            make_slot("input-a", lcc_rs::EventRole::Consumer),
            make_slot("input-b", lcc_rs::EventRole::Consumer),
        ];
        let result = best_slot(&slots, lcc_rs::EventRole::Producer);
        assert_eq!(result.unwrap().element_path, vec!["input-a"]);
    }

    #[test]
    fn best_slot_empty_slice_returns_none() {
        let result = best_slot(&[], lcc_rs::EventRole::Producer);
        assert!(result.is_none());
    }

    // ── slot_for_event_id ──────────────────────────────────────────────────────

    fn make_slot_with_path(path_str: &str, role: lcc_rs::EventRole) -> SlotInfo {
        SlotInfo {
            node_key: node_key(0),
            node_name: "Test Node".to_string(),
            element_path: path_str.split('/').map(|s| s.to_string()).collect(),
            element_description: None,
            heuristic_role: role,
        }
    }

    const SLOT_EVENT: [u8; 8] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22];

    #[test]
    fn slot_for_event_id_cache_hit_returns_precise_slot() {
        let slots = vec![
            make_slot_with_path("seg:0/elem:0", lcc_rs::EventRole::Consumer),
            make_slot_with_path("seg:0/elem:1", lcc_rs::EventRole::Producer),
        ];
        let mut node_cache = HashMap::new();
        node_cache.insert("seg:0/elem:1".to_string(), SLOT_EVENT);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_key(0), node_cache);

        let result = slot_for_event_id(&slots, &node_key(0), &SLOT_EVENT, &config_cache, lcc_rs::EventRole::Consumer);
        assert_eq!(result.unwrap().element_path, vec!["seg:0", "elem:1"]);
    }

    #[test]
    fn slot_for_event_id_node_not_in_cache_uses_heuristic() {
        let slots = vec![
            make_slot_with_path("seg:0/elem:0", lcc_rs::EventRole::Consumer),
            make_slot_with_path("seg:0/elem:1", lcc_rs::EventRole::Producer),
        ];
        let missing_key = NodeKey::from_node_id(lcc_rs::NodeID::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));
        let result = slot_for_event_id(&slots, &missing_key, &SLOT_EVENT, &HashMap::new(), lcc_rs::EventRole::Producer);
        assert_eq!(result.unwrap().element_path, vec!["seg:0", "elem:1"]);
    }

    #[test]
    fn slot_for_event_id_cache_present_but_no_event_match_uses_heuristic() {
        let slots = vec![make_slot_with_path("seg:0/elem:0", lcc_rs::EventRole::Consumer)];
        let different_event = [0x00u8; 8];
        let mut node_cache = HashMap::new();
        node_cache.insert("seg:0/elem:0".to_string(), different_event);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_key(0), node_cache);

        let result = slot_for_event_id(&slots, &node_key(0), &SLOT_EVENT, &config_cache, lcc_rs::EventRole::Consumer);
        assert_eq!(result.unwrap().element_path, vec!["seg:0", "elem:0"]);
    }

    #[test]
    fn slot_for_event_id_no_slots_returns_none() {
        let missing_key = NodeKey::from_node_id(lcc_rs::NodeID::new([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]));
        let result = slot_for_event_id(&[], &missing_key, &SLOT_EVENT, &HashMap::new(), lcc_rs::EventRole::Producer);
        assert!(result.is_none());
    }

    // ── walk_cdi_slots ─────────────────────────────────────────────────────────

    fn make_node_with_cdi_xml(index: usize, cdi_xml: &str) -> lcc_rs::DiscoveredNode {
        lcc_rs::DiscoveredNode {
            node_id: lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, index as u8]),
            alias: lcc_rs::NodeAlias::new(0x100 + index as u16).unwrap(),
            snip_data: None,
            snip_status: lcc_rs::types::SNIPStatus::Unknown,
            connection_status: lcc_rs::types::ConnectionStatus::Unknown,
            last_verified: None,
            last_seen: chrono::Utc::now(),
            cdi: Some(lcc_rs::types::CdiData {
                xml_content: cdi_xml.to_string(),
                retrieved_at: chrono::Utc::now(),
            }),
            pip_flags: None,
            pip_status: lcc_rs::types::PIPStatus::Unknown,
        }
    }

    #[test]
    fn walk_cdi_slots_valid_cdi_returns_correct_count() {
        let cdi_xml = r#"<cdi>
            <segment space="253" origin="0">
                <name>Config</name>
                <group><name>Producers</name>
                    <eventid><name>Output A</name></eventid>
                    <eventid><name>Output B</name></eventid>
                </group>
                <group><name>Consumers</name>
                    <eventid><name>Input A</name></eventid>
                </group>
            </segment>
        </cdi>"#;
        let node = make_node_with_cdi_xml(0, cdi_xml);
        let slots = walk_cdi_slots(&node);
        assert_eq!(slots.len(), 3);
    }

    #[test]
    fn walk_cdi_slots_no_cdi_returns_empty() {
        let node = make_nodes(1).into_iter().next().unwrap();
        let slots = walk_cdi_slots(&node);
        assert!(slots.is_empty());
    }

    #[test]
    fn walk_cdi_slots_invalid_xml_returns_empty() {
        let node = make_node_with_cdi_xml(0, "<not valid xml<<<<");
        let slots = walk_cdi_slots(&node);
        assert!(slots.is_empty());
    }

    // ── T007: well-known event ID tests ───────────────────────────────────────

    const WK_EMERGENCY_OFF: [u8; 8] = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF];

    #[test]
    fn t007a_well_known_protocol_only_no_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(WK_EMERGENCY_OFF, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 0);
    }

    #[test]
    fn t007b_well_known_config_one_node_gives_card() {
        let nodes = make_nodes(1);
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        let mut node_cache = HashMap::new();
        node_cache.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_key(0), node_cache);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.event_id_bytes, WK_EMERGENCY_OFF);
        assert_eq!(card.name, Some("Emergency Off".to_string()));
        let total = card.producers.len() + card.consumers.len() + card.ambiguous_entries.len();
        assert_eq!(total, 1);
    }

    #[test]
    fn t007c_well_known_config_two_nodes_gives_card() {
        let nodes = make_nodes(2);
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        let mut cache0 = HashMap::new();
        cache0.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut cache1 = HashMap::new();
        cache1.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_key(0), cache0);
        config_cache.insert(node_key(1), cache1);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.name, Some("Emergency Off".to_string()));
        let total = card.producers.len() + card.consumers.len() + card.ambiguous_entries.len();
        assert_eq!(total, 2);
    }

    #[test]
    fn t007d_well_known_protocol_and_config_uses_config_only() {
        let nodes = make_nodes(3);
        let mut event_roles = HashMap::new();
        event_roles.insert(WK_EMERGENCY_OFF, roles(&[0], &[1]));

        let mut cache2 = HashMap::new();
        cache2.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_key(2), cache2);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        let all_node_keys: Vec<NodeKey> = card.producers.iter()
            .chain(card.consumers.iter())
            .chain(card.ambiguous_entries.iter())
            .map(|e| e.node_key)
            .collect();
        assert_eq!(all_node_keys.len(), 1);
        assert!(all_node_keys.contains(&node_key(2)));
        assert!(!all_node_keys.contains(&node_key(0)));
        assert!(!all_node_keys.contains(&node_key(1)));
    }

    #[test]
    fn zero_event_id_in_config_cache_is_ignored() {
        let nodes = make_nodes(2);
        let zero: [u8; 8] = [0u8; 8];

        let mut config_cache: HashMap<NodeKey, HashMap<String, [u8; 8]>> = HashMap::new();
        let mut node0_cache = HashMap::new();
        node0_cache.insert("seg:0/elem:0".to_string(), zero);
        config_cache.insert(node_key(0), node0_cache);
        let mut node1_cache = HashMap::new();
        node1_cache.insert("seg:0/elem:0".to_string(), zero);
        config_cache.insert(node_key(1), node1_cache);

        let catalog = build_bowtie_catalog(&nodes, &HashMap::new(), &config_cache, None);

        assert_eq!(catalog.bowties.len(), 0);
    }
}
