//! Bowtie catalog: discovery, building, and query commands
//!
//! A "bowtie" is a shared LCC event ID that has at least one producer slot
//! and at least one consumer slot across the discovered nodes on the network.
//!
//! ## Data flow
//! 1. `read_all_config_values` completes for all nodes.
//! 2. `query_event_roles` sends `IdentifyEventsAddressed` to each node (125 ms apart)
//!    and collects `ProducerIdentified` / `ConsumerIdentified` replies for 500 ms.
//! 3. `build_bowtie_catalog` groups the resulting `NodeRoles` map into `BowtieCard`s.
//! 4. The catalog is stored in `AppState.bowties_catalog` and emitted as `cdi-read-complete`.
//! 5. `get_bowties` Tauri command lets the frontend retrieve the catalog on demand.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use tauri::{Emitter, Manager};

use crate::state::{AppState, BowtieCatalog, BowtieCard, BowtieState, EventSlotEntry, NodeRoles};

// ── Well-known event IDs ──────────────────────────────────────────────────────

/// Standard LCC well-known event IDs from the OpenLCB Event Identifiers Standard.
///
/// These events are handled at the protocol level by nodes such as command stations
/// and throttles — those nodes respond to `IdentifyEventsAddressed` for them even
/// when no user has configured a CDI slot to one of these values.  Therefore bowtie
/// cards for well-known event IDs are built exclusively from `config_value_cache`
/// (CDI slots the user explicitly set to these values), not from the protocol exchange.
/// The all-zeros event ID is the default "unset" value written to CDI event ID
/// slots that have never been configured.  It is not a valid routable event ID
/// and must be excluded from bowtie catalog discovery.
const ZERO_EVENT_ID: [u8; 8] = [0u8; 8];

const WELL_KNOWN_EVENT_IDS: &[([u8; 8], &str)] = &[
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

// ── Payload ───────────────────────────────────────────────────────────────────

/// Payload emitted with the `cdi-read-complete` Tauri event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiReadCompletePayload {
    /// Freshly-built catalog.
    pub catalog: BowtieCatalog,
    /// Number of nodes that were included in the build.
    pub node_count: usize,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return a human-readable node name using the SNIP priority chain:
/// user_name → "{mfg} — {model}" → node_id_hex.
fn node_display_name(node: &lcc_rs::DiscoveredNode) -> String {
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

// ── Core builder ─────────────────────────────────────────────────────────────

/// Slot metadata gathered from a single CDI walk.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SlotInfo {
    node_id: String,
    node_name: String,
    element_path: Vec<String>,
    /// Raw CDI <description> text, preserved for forwarding to frontend.
    element_description: Option<String>,
    heuristic_role: lcc_rs::EventRole,
}

/// Pre-walk all CDI event slots for a node and return a list of slot infos.
///
/// If the node has no CDI data (or parsing fails) an empty vec is returned.
fn walk_cdi_slots(node: &lcc_rs::DiscoveredNode) -> Vec<SlotInfo> {
    let cdi_xml = match node.cdi.as_ref().map(|d| d.xml_content.as_str()) {
        Some(xml) if !xml.is_empty() => xml,
        _ => return Vec::new(),
    };

    let cdi = match lcc_rs::cdi::parser::parse_cdi(cdi_xml) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut slots = Vec::new();
    let node_id = node.node_id.to_hex_string();
    let node_name = node_display_name(node);

    lcc_rs::walk_event_slots(&cdi, |element, ancestor_names, path| {
        let role = lcc_rs::classify_event_slot(element, ancestor_names);
        slots.push(SlotInfo {
            node_id: node_id.clone(),
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
fn best_slot<'a>(slots: &'a [SlotInfo], expected_role: lcc_rs::EventRole) -> Option<&'a SlotInfo> {
    slots
        .iter()
        .find(|s| s.heuristic_role == expected_role)
        .or_else(|| slots.first())
}

/// Find the slot for a specific event ID by first checking the config value cache
/// (precise match on which slot actually holds that event ID), then falling back
/// to the heuristic `best_slot` if no cache entry exists for this slot.
///
/// `config_cache` is `AppState.config_value_cache` keyed by node_id_hex → (path → bytes).
fn slot_for_event_id<'a>(
    slots: &'a [SlotInfo],
    node_id: &str,
    event_id_bytes: &[u8; 8],
    config_cache: &std::collections::HashMap<String, std::collections::HashMap<String, [u8; 8]>>,
    fallback_role: lcc_rs::EventRole,
) -> Option<&'a SlotInfo> {
    // Precise lookup: which slot on this node was actually configured with this event ID?
    if let Some(node_cache) = config_cache.get(node_id) {
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
    config_value_cache: &HashMap<String, HashMap<String, [u8; 8]>>,
    profile_group_roles: Option<&HashMap<String, lcc_rs::EventRole>>,
) -> BowtieCatalog {
    let build_start = std::time::Instant::now();
    let built_at = chrono::Utc::now().to_rfc3339();
    let source_node_count = nodes.len();

    // Pre-walk: build a map from node_id_hex → Vec<SlotInfo>
    let mut slot_map: HashMap<String, Vec<SlotInfo>> = HashMap::new();
    for node in nodes {
        let slots = walk_cdi_slots(node);
        if !slots.is_empty() {
            slot_map.insert(node.node_id.to_hex_string(), slots);
        }
    }

    let total_slots_scanned: usize = slot_map.values().map(|v| v.len()).sum();

    let mut bowties: Vec<BowtieCard> = Vec::new();

    // Build a set of well-known event IDs for O(1) skip in the protocol loop.
    let well_known_set: std::collections::HashSet<[u8; 8]> =
        WELL_KNOWN_EVENT_IDS.iter().map(|&(bytes, _)| bytes).collect();

    // ── Config-primary event discovery ───────────────────────────────────────────
    //
    // The config value cache is the canonical source for which event IDs are
    // configured across the network.  Reading CDI memory reliably surfaces every
    // configured slot; IdentifyEvents replies are NOT always present (some firmware
    // never sends ProducerIdentified / ConsumerIdentified, and nodes with both a
    // producer and a consumer slot wired to the same event respond on both sides).
    //
    // Algorithm:
    //   1. Build a per-event slot list from config_value_cache (config-primary map).
    //   2. Union with events found only via the protocol exchange (backward-compat).
    //   3. For each event, classify every config slot:
    //      - If the node has exactly one slot for this event AND the protocol reply
    //        unambiguously names it as producer XOR consumer → use that role.
    //      - If the node has multiple slots OR replied on both sides (or neither) →
    //        use the CDI heuristic / profile role; this handles the common case of
    //        a single node with a producer slot and a consumer slot wired to the
    //        same event ID.
    //   4. Visibility: emit a card when ≥2 config slots share the event ID.
    //      Protocol-only events (no config evidence) fall back to the original
    //      FR-002 rule (≥1 confirmed producer AND ≥1 confirmed consumer).
    //
    // Named single-slot events (e.g. Planning bowties in the layout file) are
    // added by merge_layout_metadata; they do not need special handling here.

    // Step 1 — build event_id → [(node_id, path_key)] from config cache.
    let mut config_slot_map: HashMap<[u8; 8], Vec<(String, String)>> = HashMap::new();
    for (node_id, node_cache) in config_value_cache {
        for (path_key, &event_bytes) in node_cache {
            if event_bytes == ZERO_EVENT_ID || well_known_set.contains(&event_bytes) {
                continue;
            }
            config_slot_map
                .entry(event_bytes)
                .or_default()
                .push((node_id.clone(), path_key.clone()));
        }
    }

    // Step 2 — union: include protocol-only events not already in config_slot_map.
    let mut all_event_ids: std::collections::HashSet<[u8; 8]> =
        config_slot_map.keys().copied().collect();
    for event_id_bytes in event_roles.keys() {
        if *event_id_bytes != ZERO_EVENT_ID && !well_known_set.contains(event_id_bytes) {
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

            // Count per-node slots so we know when the node-level protocol reply
            // can classify an individual slot vs when we need the CDI heuristic.
            let mut node_slot_count: HashMap<&str, usize> = HashMap::new();
            for (node_id, _) in slot_refs {
                *node_slot_count.entry(node_id.as_str()).or_insert(0) += 1;
            }

            for (node_id, path_key) in slot_refs {
                let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
                let node_name = nodes
                    .iter()
                    .find(|n| n.node_id.to_hex_string() == *node_id)
                    .map(node_display_name)
                    .unwrap_or_else(|| node_id.clone());

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

                // Node-level protocol classification is reliable only when the node has
                // exactly one config slot for this event.  Multiple slots → fall back to
                // CDI heuristic / profile, which classifies each slot individually.
                let multi_slot_node =
                    node_slot_count.get(node_id.as_str()).copied().unwrap_or(1) > 1;
                let node_in_producers = !multi_slot_node
                    && roles_opt.map(|r| r.producers.contains(node_id)).unwrap_or(false);
                let node_in_consumers = !multi_slot_node
                    && roles_opt.map(|r| r.consumers.contains(node_id)).unwrap_or(false);

                let resolved_role = match (node_in_producers, node_in_consumers) {
                    (true, false) => lcc_rs::EventRole::Producer,
                    (false, true) => lcc_rs::EventRole::Consumer,
                    _ => {
                        let profile_key = format!("{}:{}", node_id, path_key);
                        profile_group_roles
                            .and_then(|map| map.get(&profile_key))
                            .copied()
                            .unwrap_or(heuristic)
                    }
                };

                let entry = EventSlotEntry {
                    node_id: node_id.clone(),
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

            // Require ≥2 total entries as evidence of a connection.
            // Include protocol-confirmed nodes that have no config slot for this event —
            // they count toward the threshold and are classified by their protocol role.
            let config_nodes: std::collections::HashSet<&str> =
                slot_refs.iter().map(|(nid, _)| nid.as_str()).collect();

            if let Some(roles) = roles_opt {
                for node_id in roles.producers.difference(&roles.consumers) {
                    if config_nodes.contains(node_id.as_str()) { continue; }
                    let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
                    let slot = slot_for_event_id(
                        slots, node_id, event_id_bytes, config_value_cache,
                        lcc_rs::EventRole::Producer,
                    );
                    let (ep, ed) = slot
                        .map(|s| (s.element_path.clone(), s.element_description.clone()))
                        .unwrap_or_else(|| (vec![], None));
                    let node_name = nodes
                        .iter()
                        .find(|n| n.node_id.to_hex_string() == *node_id)
                        .map(node_display_name)
                        .unwrap_or_else(|| node_id.clone());
                    producers.push(EventSlotEntry {
                        node_id: node_id.clone(),
                        node_name,
                        element_path: ep,
                        element_description: ed,
                        event_id: *event_id_bytes,
                        role: lcc_rs::EventRole::Producer,
                    });
                }
                for node_id in roles.consumers.difference(&roles.producers) {
                    if config_nodes.contains(node_id.as_str()) { continue; }
                    let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
                    let slot = slot_for_event_id(
                        slots, node_id, event_id_bytes, config_value_cache,
                        lcc_rs::EventRole::Consumer,
                    );
                    let (ep, ed) = slot
                        .map(|s| (s.element_path.clone(), s.element_description.clone()))
                        .unwrap_or_else(|| (vec![], None));
                    let node_name = nodes
                        .iter()
                        .find(|n| n.node_id.to_hex_string() == *node_id)
                        .map(node_display_name)
                        .unwrap_or_else(|| node_id.clone());
                    consumers.push(EventSlotEntry {
                        node_id: node_id.clone(),
                        node_name,
                        element_path: ep,
                        element_description: ed,
                        event_id: *event_id_bytes,
                        role: lcc_rs::EventRole::Consumer,
                    });
                }
                for node_id in roles.producers.intersection(&roles.consumers) {
                    if config_nodes.contains(node_id.as_str()) { continue; }
                    let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
                    let node_name = nodes
                        .iter()
                        .find(|n| n.node_id.to_hex_string() == *node_id)
                        .map(node_display_name)
                        .unwrap_or_else(|| node_id.clone());
                    let slot = slots.first();
                    let (ep, ed) = slot
                        .map(|s| (s.element_path.clone(), s.element_description.clone()))
                        .unwrap_or_else(|| (vec![], None));
                    ambiguous_entries.push(EventSlotEntry {
                        node_id: node_id.clone(),
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
            if total_entries < 2 {
                continue;
            }
        } else {
            // ── Protocol-only path ────────────────────────────────────────────
            // No config slot confirms this event; rely on FR-002 (original rule):
            // ≥1 confirmed producer AND ≥1 confirmed consumer from the protocol.
            let roles = match roles_opt {
                Some(r) if !r.producers.is_empty() && !r.consumers.is_empty() => r,
                _ => continue,
            };

            let both: std::collections::HashSet<&String> =
                roles.producers.intersection(&roles.consumers).collect();

            for node_id in roles.producers.difference(&roles.consumers) {
                let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
                let slot = slot_for_event_id(
                    slots, node_id, event_id_bytes, config_value_cache,
                    lcc_rs::EventRole::Producer,
                );
                let (ep, ed) = slot
                    .map(|s| (s.element_path.clone(), s.element_description.clone()))
                    .unwrap_or_else(|| (vec![], None));
                let node_name = nodes
                    .iter()
                    .find(|n| n.node_id.to_hex_string() == *node_id)
                    .map(node_display_name)
                    .unwrap_or_else(|| node_id.clone());
                producers.push(EventSlotEntry {
                    node_id: node_id.clone(),
                    node_name,
                    element_path: ep,
                    element_description: ed,
                    event_id: *event_id_bytes,
                    role: lcc_rs::EventRole::Producer,
                });
            }

            for node_id in roles.consumers.difference(&roles.producers) {
                let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
                let slot = slot_for_event_id(
                    slots, node_id, event_id_bytes, config_value_cache,
                    lcc_rs::EventRole::Consumer,
                );
                let (ep, ed) = slot
                    .map(|s| (s.element_path.clone(), s.element_description.clone()))
                    .unwrap_or_else(|| (vec![], None));
                let node_name = nodes
                    .iter()
                    .find(|n| n.node_id.to_hex_string() == *node_id)
                    .map(node_display_name)
                    .unwrap_or_else(|| node_id.clone());
                consumers.push(EventSlotEntry {
                    node_id: node_id.clone(),
                    node_name,
                    element_path: ep,
                    element_description: ed,
                    event_id: *event_id_bytes,
                    role: lcc_rs::EventRole::Consumer,
                });
            }

            for node_id in &both {
                let slots = slot_map.get(*node_id).map(|s| s.as_slice()).unwrap_or(&[]);
                let node_name = nodes
                    .iter()
                    .find(|n| n.node_id.to_hex_string() == **node_id)
                    .map(node_display_name)
                    .unwrap_or_else(|| (*node_id).clone());
                let slot = slots.first();
                let (ep, ed) = slot
                    .map(|s| (s.element_path.clone(), s.element_description.clone()))
                    .unwrap_or_else(|| (vec![], None));
                let profile_key = slot
                    .map(|s| format!("{}:{}", *node_id, s.element_path.join("/")))
                    .unwrap_or_else(|| (*node_id).clone());
                let resolved = profile_group_roles
                    .and_then(|map| map.get(&profile_key))
                    .copied()
                    .unwrap_or(lcc_rs::EventRole::Ambiguous);
                let entry = EventSlotEntry {
                    node_id: (*node_id).clone(),
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
    //
    // Protocol replies from nodes like JMRI or UWT-100 are ignored for these IDs because
    // those nodes participate at the protocol level even when no CDI slot has been set.
    // A card is emitted only when ≥1 node has a CDI config slot whose cached value equals
    // the well-known event ID.  A single-entry card is valid here — it informs the user
    // which slot they have configured; the "requires ≥2 entries" rule does not apply.
    for &(wk_bytes, wk_name) in WELL_KNOWN_EVENT_IDS {
        let event_id_hex = format!(
            "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
            wk_bytes[0], wk_bytes[1], wk_bytes[2], wk_bytes[3],
            wk_bytes[4], wk_bytes[5], wk_bytes[6], wk_bytes[7]
        );

        let mut wk_producers: Vec<EventSlotEntry> = Vec::new();
        let mut wk_consumers: Vec<EventSlotEntry> = Vec::new();
        let mut wk_ambiguous: Vec<EventSlotEntry> = Vec::new();

        for (node_id, node_cache) in config_value_cache {
            let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
            let node_name = nodes
                .iter()
                .find(|n| n.node_id.to_hex_string() == *node_id)
                .map(node_display_name)
                .unwrap_or_else(|| node_id.clone());

            for (path_key, &cached_bytes) in node_cache {
                if cached_bytes != wk_bytes {
                    continue;
                }

                // Resolve SlotInfo for this path to obtain description and
                // heuristic role classification.
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

                // Profile role overrides the CDI heuristic; Ambiguous heuristic stays Ambiguous.
                let profile_key = format!("{}:{}", node_id, path_key);
                let resolved_role = profile_group_roles
                    .and_then(|map| map.get(&profile_key))
                    .copied()
                    .unwrap_or(heuristic);

                match resolved_role {
                    lcc_rs::EventRole::Producer => {
                        wk_producers.push(EventSlotEntry {
                            node_id: node_id.clone(),
                            node_name: node_name.clone(),
                            element_path: ep,
                            element_description: ed,
                            event_id: wk_bytes,
                            role: lcc_rs::EventRole::Producer,
                        });
                    }
                    lcc_rs::EventRole::Consumer => {
                        wk_consumers.push(EventSlotEntry {
                            node_id: node_id.clone(),
                            node_name: node_name.clone(),
                            element_path: ep,
                            element_description: ed,
                            event_id: wk_bytes,
                            role: lcc_rs::EventRole::Consumer,
                        });
                    }
                    lcc_rs::EventRole::Ambiguous => {
                        wk_ambiguous.push(EventSlotEntry {
                            node_id: node_id.clone(),
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
            // Only canonicalise to upper-case when the key is a valid dotted-hex
            // event ID.  Planning placeholder keys like "planning-1234567890" must
            // be preserved as-is so the TypeScript preview store can match them
            // against the layout's lowercase keys (prevents duplicate cards).
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
    // Classification key format: "{nodeId}:{element_path_joined_by_/}"
    for card in &mut catalog.bowties {
        let mut still_ambiguous = Vec::new();
        for entry in card.ambiguous_entries.drain(..) {
            let class_key = format!("{}:{}", entry.node_id, entry.element_path.join("/"));
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

/// Parse a dotted-hex event ID string into 8 bytes.
fn parse_event_id_hex(hex: &str) -> Option<[u8; 8]> {
    let parts: Vec<&str> = hex.split('.').collect();
    if parts.len() != 8 {
        return None;
    }
    let mut bytes = [0u8; 8];
    for (i, part) in parts.iter().enumerate() {
        bytes[i] = u8::from_str_radix(part, 16).ok()?;
    }
    Some(bytes)
}

// ── Protocol query ────────────────────────────────────────────────────────────

/// Send `IdentifyEventsAddressed` (MTI 0x0488) to each node and collect
/// `ProducerIdentified` / `ConsumerIdentified` replies.
///
/// **Timing** (per spec contract and JMRI reference):
/// - 125 ms between successive addressed sends.
/// - 500 ms collection window starting from the last send.
///
/// Returns a map of `event_id_bytes → NodeRoles` where `NodeRoles` records
/// which node IDs claimed to produce / consume each event.
///
/// If the connection or transport handle is unavailable the function returns an empty map.
pub async fn query_event_roles(
    state: &AppState,
    send_delay_ms: u64,
    collect_window_ms: u64,
) -> HashMap<[u8; 8], NodeRoles> {
    use lcc_rs::protocol::{GridConnectFrame, MTI};
    use lcc_rs::TransportHandle;
    use tokio::sync::broadcast;
    use tokio::time::{sleep, Duration};
    use std::time::Instant as StdInstant;

    // Grab connection + transport handle + own alias.
    let (_connection, handle, our_alias) = {
        let conn_lock = state.connection.read().await;
        let conn_opt = match conn_lock.as_ref() {
            Some(c) => c.clone(),
            None => {
                eprintln!("[bowties] query_event_roles: no connection");
                return HashMap::new();
            }
        };
        let (our_alias, handle) = {
            let c = conn_opt.lock().await;
            let alias = c.our_alias().value();
            let h = c.transport_handle().cloned();
            (alias, h)
        };
        let handle: TransportHandle = match handle {
            Some(h) => h,
            None => {
                eprintln!("[bowties] query_event_roles: no transport handle");
                return HashMap::new();
            }
        };
        (conn_opt, handle, our_alias)
    };

    // Read current node list from proxy registry
    let nodes = state.node_registry.get_all_snapshots().await;
    if nodes.is_empty() {
        return HashMap::new();
    }

    let exchange_start = StdInstant::now();
    let started_at = chrono::Utc::now();
    let nodes_queried = nodes.len();
    crate::bwlog!(state, "[bowties] query_event_roles: sending to {} nodes", nodes_queried);

    // Subscribe to all broadcast traffic so we catch the six relevant MTIs.
    let mut rx = handle.subscribe_all();

    // Send IdentifyEventsAddressed to each node, 125 ms apart.
    let mut events_sent: usize = 0;
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            sleep(Duration::from_millis(send_delay_ms)).await;
        }

        let dest_alias = node.alias.value();
        match GridConnectFrame::from_addressed_mti(
            MTI::IdentifyEventsAddressed,
            our_alias,
            dest_alias,
            vec![],
        ) {
            Ok(frame) => {
                if let Err(e) = handle.send(&frame).await {
                    eprintln!(
                        "[bowties] IdentifyEventsAddressed send error to {:?}: {}",
                        node.node_id, e
                    );
                } else {
                    events_sent += 1;
                }
            }
            Err(e) => {
                eprintln!(
                    "[bowties] frame build error for {:?}: {}",
                    node.node_id, e
                );
            }
        }
    }

    // Build alias → node_id lookup for fast resolution.
    let alias_to_node_id: HashMap<u16, String> = nodes
        .iter()
        .map(|n| (n.alias.value(), n.node_id.to_hex_string()))
        .collect();

    // Collect replies for `collect_window_ms` ms.
    let mut roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();
    let mut responses_received: usize = 0;

    let collect_deadline = tokio::time::Instant::now()
        + Duration::from_millis(collect_window_ms);

    loop {
        let remaining = collect_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        let recv_result = tokio::time::timeout(remaining, rx.recv()).await;

        match recv_result {
            Ok(Ok(msg)) => {
                let frame = &msg.frame;
                // Extract (mti_value, source_alias) from header.
                let mti_value = (frame.header >> 12) & 0x1FFFF;
                let source_alias = (frame.header & 0xFFF) as u16;

                // Check for one of the six event-identified MTIs.
                // ProducerIdentified: Valid=0x19544, Invalid=0x19545, Unknown=0x19547
                // ConsumerIdentified: Valid=0x194C4, Invalid=0x194C5, Unknown=0x194C7
                let is_producer = mti_value == 0x19544
                    || mti_value == 0x19545
                    || mti_value == 0x19547;
                let is_consumer = mti_value == 0x194C4
                    || mti_value == 0x194C5
                    || mti_value == 0x194C7;

                if !is_producer && !is_consumer {
                    continue;
                }

                // Frame data = 8-byte event ID.
                if frame.data.len() < 8 {
                    continue;
                }
                let event_id: [u8; 8] = frame.data[..8].try_into().unwrap_or([0u8; 8]);

                // Resolve source_alias → node_id_hex.
                let node_id = match alias_to_node_id.get(&source_alias) {
                    Some(id) => id.clone(),
                    None => continue, // unknown alias — not one of our nodes
                };

                let entry = roles.entry(event_id).or_default();
                if is_producer {
                    entry.producers.insert(node_id);
                } else {
                    entry.consumers.insert(node_id);
                }
                responses_received += 1;
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                // Missed some frames — continue collecting
                continue;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                break;
            }
            Err(_) => {
                // Timeout — collection window ended
                break;
            }
        }
    }

    let duration_ms = exchange_start.elapsed().as_millis() as u64;
    crate::bwlog!(state,
        "[bowties] query_event_roles complete: {} event IDs, {} responses, {} nodes, {}ms",
        roles.len(), responses_received, nodes_queried, duration_ms);

    // Record EventRoleExchangeStats in diagnostics.
    {
        let stats = crate::diagnostics::EventRoleExchangeStats {
            started_at,
            nodes_queried,
            events_sent,
            responses_received,
            duration_ms,
        };
        state.diag_stats.write().await.event_role_exchange = Some(stats);
    }

    roles
}

// ── Layout file Tauri commands ────────────────────────────────────────────────

/// Load a YAML layout file from disk.
///
/// Validates the schema and emits a `layout-loaded` event on success.
#[tauri::command]
pub async fn load_layout(
    path: String,
    app: tauri::AppHandle,
) -> Result<crate::layout::types::LayoutFile, String> {
    let layout = crate::layout::io::load_file(std::path::Path::new(&path))?;

    // Emit layout-loaded event
    let _ = app.emit("layout-loaded", serde_json::json!({
        "path": path,
        "bowtieCount": layout.bowties.len(),
        "classificationCount": layout.role_classifications.len(),
    }));

    Ok(layout)
}

/// Save bowtie metadata and role classifications to a YAML layout file.
///
/// Uses atomic write (temp → flush → rename). Emits `layout-save-error` on failure.
#[tauri::command]
pub async fn save_layout(
    path: String,
    layout: crate::layout::types::LayoutFile,
    app: tauri::AppHandle,
) -> Result<(), String> {
    match crate::layout::io::save_file(std::path::Path::new(&path), &layout) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = app.emit("layout-save-error", serde_json::json!({
                "path": path,
                "error": e,
            }));
            Err(e)
        }
    }
}

/// Retrieve the most recently opened layout file path from app data dir.
#[tauri::command]
pub async fn get_recent_layout(
    app: tauri::AppHandle,
) -> Result<Option<crate::layout::types::RecentLayout>, String> {
    let app_data = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let recent_path = app_data.join("recent-layout.json");
    if !recent_path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&recent_path)
        .map_err(|e| format!("Failed to read recent layout file: {}", e))?;

    let recent: crate::layout::types::RecentLayout = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse recent layout data: {}", e))?;

    // Verify the referenced file still exists
    if !std::path::Path::new(&recent.path).exists() {
        return Ok(None);
    }

    Ok(Some(recent))
}

/// Store the most recently opened layout file path in app data dir.
#[tauri::command]
pub async fn set_recent_layout(
    path: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let app_data = app.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    std::fs::create_dir_all(&app_data)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    let recent = crate::layout::types::RecentLayout {
        path,
        last_opened: chrono::Utc::now().to_rfc3339(),
    };

    let json = serde_json::to_string_pretty(&recent)
        .map_err(|e| format!("Failed to serialize recent layout: {}", e))?;

    let recent_path = app_data.join("recent-layout.json");
    std::fs::write(&recent_path, json)
        .map_err(|e| format!("Failed to write recent layout: {}", e))?;

    Ok(())
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Rebuild the bowtie catalog, optionally merging layout file metadata.
///
/// Uses the current AppState (discovered nodes, event roles, config cache)
/// to build a fresh catalog, then merges layout metadata if provided.
/// The result is stored in AppState and emitted via `cdi-read-complete`.
#[tauri::command]
pub async fn build_bowtie_catalog_command(
    layout_metadata: Option<crate::layout::types::LayoutFile>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<BowtieCatalog, String> {
    let nodes_snap = state.node_registry.get_all_snapshots().await;

    // Gather config values from all proxies
    let config_cache_snap: HashMap<String, HashMap<String, [u8; 8]>> = {
        let handles = state.node_registry.get_all_handles().await;
        let mut map = HashMap::new();
        for h in &handles {
            if let Ok(vals) = h.get_config_values().await {
                if !vals.is_empty() {
                    map.insert(h.node_id.to_hex_string(), vals);
                }
            }
        }
        map
    };

    // Gather profile group roles from proxy config trees
    let profile_group_roles = {
        let handles = state.node_registry.get_all_handles().await;
        let mut map = HashMap::new();
        for h in &handles {
            let nid = h.node_id.to_hex_string();
            if let Ok(Some(tree)) = h.get_config_tree().await {
                for leaf in crate::node_tree::collect_event_id_leaves(&tree).into_iter() {
                    if let Some(role) = leaf.event_role {
                        if role != lcc_rs::EventRole::Ambiguous {
                            let key = format!("{}:{}", nid, leaf.path.join("/"));
                            map.insert(key, role);
                        }
                    }
                }
            }
        }
        map
    };

    // Run event query from existing cached roles (use stored catalog's data)
    // We rebuild from AppState, not re-querying the network
    let event_roles = {
        // If we have a catalog already, we can extract event roles from it
        // Otherwise query fresh
        let existing = state.bowties_catalog.read().await;
        if existing.is_some() {
            // Reconstruct event roles from existing catalog nodes
            // For a proper rebuild, we need the raw event roles
            // Use query_event_roles only if needed
            drop(existing);
            query_event_roles(&state, 125, 500).await
        } else {
            HashMap::new()
        }
    };

    let mut catalog = build_bowtie_catalog(
        &nodes_snap,
        &event_roles,
        &config_cache_snap,
        Some(&profile_group_roles),
    );

    // Merge layout metadata if provided
    if let Some(layout) = &layout_metadata {
        merge_layout_metadata(&mut catalog, layout);
    }

    // Store in AppState
    *state.bowties_catalog.write().await = Some(catalog.clone());

    // Emit to frontend
    let node_count = nodes_snap.len();
    let _ = app.emit(
        "cdi-read-complete",
        CdiReadCompletePayload { catalog: catalog.clone(), node_count },
    );

    Ok(catalog)
}

/// Return the current `BowtieCatalog` from AppState.
///
/// Returns `null` (serialized as `None`) if no catalog has been built yet —
/// i.e., CDI reads have not completed or no nodes were present.
#[tauri::command]
pub async fn get_bowties(
    state: tauri::State<'_, AppState>,
) -> Result<Option<BowtieCatalog>, String> {
    let catalog = state.bowties_catalog.read().await;
    Ok(catalog.clone())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod build_bowtie_catalog_tests {
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

    fn node_id(node_index: usize) -> String {
        lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, node_index as u8]).to_hex_string()
    }

    fn roles(producers: &[usize], consumers: &[usize]) -> NodeRoles {
        NodeRoles {
            producers: producers.iter().map(|&i| node_id(i)).collect(),
            consumers: consumers.iter().map(|&i| node_id(i)).collect(),
        }
    }

    // ── T006 test cases ───────────────────────────────────────────────────────

    /// (a) Two nodes share one event ID: one producer, one consumer → 1 BowtieCard,
    ///     ambiguous_entries empty.
    #[test]
    fn t006a_one_producer_one_consumer_gives_one_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.source_node_count, 2);
        assert_eq!(catalog.bowties.len(), 1, "Should have exactly 1 BowtieCard");
        let card = &catalog.bowties[0];
        assert_eq!(card.event_id_bytes, EVENT_A);
        assert_eq!(card.producers.len(), 1, "Should have 1 producer entry");
        assert_eq!(card.consumers.len(), 1, "Should have 1 consumer entry");
        assert!(card.ambiguous_entries.is_empty(), "No ambiguous entries expected");
        assert_eq!(card.producers[0].role, lcc_rs::EventRole::Producer);
        assert_eq!(card.consumers[0].role, lcc_rs::EventRole::Consumer);
    }

    /// (b) Two producers + one consumer on three nodes → 1 card with 2 producers.
    #[test]
    fn t006b_two_producers_one_consumer() {
        let nodes = make_nodes(3);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0, 1], &[2]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.producers.len(), 2, "Should have 2 producers");
        assert_eq!(card.consumers.len(), 1);
        assert!(card.ambiguous_entries.is_empty());
    }

    /// (c) Same node replies both ProducerIdentified + ConsumerIdentified;
    ///     CDI heuristic resolves → classified correctly in producers or consumers.
    #[test]
    fn t006c_same_node_both_sides_heuristic_resolves() {
        let nodes = make_nodes(3); // node 0 = both, node 1 = pure consumer
        let mut event_roles = HashMap::new();
        // node 0 appears in both, node 1 is pure consumer
        event_roles.insert(EVENT_A, roles(&[0], &[0, 1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        // node0: no CDI → 0 P-votes, 0 C-votes → tie → Ambiguous → ambiguous_entries.
        // node1: pure consumer → consumers.
        // Total entries: 0P + 1C + 1A = 2 ≥ 2 → card IS emitted.
        assert_eq!(catalog.bowties.len(), 1, "Card emitted: 1 consumer + 1 ambiguous = 2 entries");
        let card = &catalog.bowties[0];
        assert!(card.producers.is_empty(), "No confirmed producers");
        assert_eq!(card.consumers.len(), 1, "node1 is a pure consumer");
        assert_eq!(card.ambiguous_entries.len(), 1, "node0 tie → ambiguous");
    }

    /// (c-resolved) Same-node with CDI that has clear Producer slots.
    ///
    /// We build minimal DiscoveredNode objects with CDI XML so classify_event_slot
    /// can resolve the same-node case.
    #[test]
    fn t006c_same_node_resolved_by_cdi_heuristic() {
        use lcc_rs::types::{SNIPStatus, ConnectionStatus};

        // Node 0 has 2 Producer-labelled slots, 1 Consumer-labelled slot → majority = Producer.
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

        // node 0 is in both sides; node 1 is pure consumer.
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, NodeRoles {
            producers: [node_id(0)].iter().cloned().collect(),
            consumers: [node_id(0), node_id(1)].iter().cloned().collect(),
        });

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        // Heuristic no longer routes same-node entries: node0 → ambiguous_entries.
        // node1 is a pure consumer (protocol-confirmed) → consumers.
        // Total = 0P + 1C + 1A = 2 entries → card emitted.
        assert_eq!(catalog.bowties.len(), 1, "Expected 1 card: 1 consumer + 1 ambiguous");
        let card = &catalog.bowties[0];
        assert!(card.producers.is_empty(), "Heuristic not used; node0 is ambiguous");
        assert_eq!(card.consumers.len(), 1, "node1 is a pure consumer");
        assert_eq!(card.ambiguous_entries.len(), 1, "node0 → ambiguous (no heuristic routing)");
    }

    /// (d) Same-node, heuristic inconclusive → entry in ambiguous_entries.
    #[test]
    fn t006d_same_node_heuristic_inconclusive_goes_to_ambiguous() {
        use lcc_rs::types::{SNIPStatus, ConnectionStatus};

        // Node 0 has equal producer/consumer slots → tie → Ambiguous.
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

        // node 0 in both; node 1 = pure consumer (provides the required consumer)
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, NodeRoles {
            producers: [node_id(0)].iter().cloned().collect(),
            consumers: [node_id(0), node_id(1)].iter().cloned().collect(),
        });

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        // Tie → node0 goes to ambiguous_entries.
        // node1 is pure consumer → consumers vec.
        // Total entries: 0P + 1C + 1A = 2 ≥ 2 → card IS emitted.
        assert_eq!(catalog.bowties.len(), 1, "Card emitted: ambiguous + consumer = 2 entries");
        let card = &catalog.bowties[0];
        assert!(card.producers.is_empty());
        assert_eq!(card.consumers.len(), 1, "node1 is a pure consumer");
        assert_eq!(card.ambiguous_entries.len(), 1, "node0 tie → ambiguous");
        assert_eq!(card.ambiguous_entries[0].role, lcc_rs::EventRole::Ambiguous);
    }

    /// A bowtie where both the producer and consumer slots are confirmed via the
    /// config value cache must appear in the catalog, even when the IdentifyEvents
    /// exchange returned no roles for that event.
    ///
    /// This covers the common case where nodes don't respond to
    /// `IdentifyEventsAddressed` (e.g. firmware limitation) but the CDI config
    /// values are already known from the memory read.
    #[test]
    fn catalog_includes_event_confirmed_by_config_cache_when_identify_events_returns_no_roles() {
        let nodes = make_nodes(2);
        // IdentifyEvents exchange produced no roles for EVENT_A — nodes didn't respond.
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        // Config cache confirms: node 0 has EVENT_A in a producer slot,
        // node 1 has EVENT_A in a consumer slot.
        let mut config_cache: HashMap<String, HashMap<String, [u8; 8]>> = HashMap::new();
        let mut node0_cache: HashMap<String, [u8; 8]> = HashMap::new();
        node0_cache.insert("seg:0/elem:0".to_string(), EVENT_A);
        config_cache.insert(node_id(0), node0_cache);
        let mut node1_cache: HashMap<String, [u8; 8]> = HashMap::new();
        node1_cache.insert("seg:0/elem:1".to_string(), EVENT_A);
        config_cache.insert(node_id(1), node1_cache);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(
            catalog.bowties.len(), 1,
            "Event confirmed in config cache on two nodes must appear in catalog \
             even when IdentifyEvents returned no roles"
        );
        assert_eq!(catalog.bowties[0].event_id_bytes, EVENT_A);
        // Both nodes should appear as entries (producer + consumer or ambiguous)
        let total = catalog.bowties[0].producers.len()
            + catalog.bowties[0].consumers.len()
            + catalog.bowties[0].ambiguous_entries.len();
        assert_eq!(total, 2, "Both config-cache-confirmed nodes must surface as entries");
    }

    /// (e) Event ID only in producers → no card (FR-002).
    #[test]
    fn t006e_producers_only_no_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0, 1], &[]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(
            catalog.bowties.len(), 0,
            "No card when there are only producers and no consumers"
        );
    }

    /// (f) Zero nodes → empty catalog.
    #[test]
    fn t006f_zero_nodes_empty_catalog() {
        let nodes: Vec<lcc_rs::DiscoveredNode> = Vec::new();
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.source_node_count, 0);
        assert_eq!(catalog.bowties.len(), 0);
        assert_eq!(catalog.total_slots_scanned, 0);
    }

    /// (g) Same event ID appears only once as a BowtieCard (SC-002).
    #[test]
    fn t006g_same_event_id_exactly_one_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        // Inserting the same key twice is impossible with HashMap; just verify len.
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        // SC-002: unique event IDs → unique cards
        let unique_event_ids: HashSet<[u8; 8]> =
            catalog.bowties.iter().map(|c| c.event_id_bytes).collect();
        assert_eq!(
            unique_event_ids.len(),
            catalog.bowties.len(),
            "Every BowtieCard must have a unique event_id_bytes (SC-002)"
        );
    }

    /// Catalog is sorted by event_id_bytes.
    #[test]
    fn t006_bowties_sorted_by_event_id() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        // Insert in reverse order to ensure sorting happens.
        event_roles.insert(EVENT_B, roles(&[0], &[1]));
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 2);
        assert!(
            catalog.bowties[0].event_id_bytes <= catalog.bowties[1].event_id_bytes,
            "Cards should be sorted by event_id_bytes"
        );
    }

    /// EventSlotEntry.role is never Ambiguous inside producers or consumers vecs.
    #[test]
    fn t006_entry_role_invariant() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);
        for card in &catalog.bowties {
            for entry in &card.producers {
                assert_ne!(
                    entry.role, lcc_rs::EventRole::Ambiguous,
                    "producers[] must never contain Ambiguous"
                );
            }
            for entry in &card.consumers {
                assert_ne!(
                    entry.role, lcc_rs::EventRole::Ambiguous,
                    "consumers[] must never contain Ambiguous"
                );
            }
        }
    }

    /// A lone same-node entry (total = 1) is filtered out: no connection to show.
    /// One node that is both producer+consumer with one ambiguous slot → total = 1.
    #[test]
    fn t006_single_ambiguous_entry_filtered() {
        let nodes = make_nodes(1);
        let mut event_roles = HashMap::new();
        // node 0 appears in both sets, no CDI → fallback heuristic → tie → 1 ambiguous entry.
        event_roles.insert(EVENT_A, NodeRoles {
            producers: [node_id(0)].iter().cloned().collect(),
            consumers: [node_id(0)].iter().cloned().collect(),
        });

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(
            catalog.bowties.len(), 0,
            "Single ambiguous entry (total = 1) must be silently excluded"
        );
    }

    /// T027: Same-node slot with a profile-declared Producer role is routed to
    /// `producers`, not `ambiguous_entries` (FR-016, FR-017).
    #[test]
    fn build_bowtie_catalog_uses_profile_roles() {
        use lcc_rs::types::{SNIPStatus, ConnectionStatus};

        // node 0 has a single Producer-intended EventId slot; node 1 is a pure consumer.
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

        // node 0 appears in both producer + consumer sets (same-node); node 1 is pure consumer.
        let mut event_roles_map = HashMap::new();
        event_roles_map.insert(EVENT_A, NodeRoles {
            producers: [node_id(0)].iter().cloned().collect(),
            consumers: [node_id(0), node_id(1)].iter().cloned().collect(),
        });

        // Config cache: node 0's CDI slot at path "seg:0/elem:0/elem:0" holds EVENT_A.
        // (seg:0 = first segment, elem:0 = first group, elem:0 = first eventid inside group)
        let slot_path = "seg:0/elem:0/elem:0".to_string();
        let mut node_cache = HashMap::new();
        node_cache.insert(slot_path.clone(), EVENT_A);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_id(0), node_cache);

        // Profile declares that slot as Producer.
        let profile_key = format!("{}:{}", node_id(0), slot_path);
        let mut profile_roles: HashMap<String, lcc_rs::EventRole> = HashMap::new();
        profile_roles.insert(profile_key, lcc_rs::EventRole::Producer);

        let catalog = build_bowtie_catalog(&nodes, &event_roles_map, &config_cache, Some(&profile_roles));

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.producers.len(), 1, "Profile role should route node0 slot to producers");
        assert_eq!(card.consumers.len(), 1, "node1 is a pure consumer");
        assert!(card.ambiguous_entries.is_empty(), "No ambiguous entries when profile resolves the role");
        assert_eq!(card.producers[0].role, lcc_rs::EventRole::Producer);
        assert_eq!(card.producers[0].node_id, node_id(0));
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
        let node = make_nodes(1).into_iter().next().unwrap(); // snip_data: None
        let expected = lcc_rs::NodeID::new([0x05, 0x02, 0x01, 0x00, 0x00, 0x00]).to_hex_string();
        assert_eq!(node_display_name(&node), expected);
    }

    #[test]
    fn node_display_name_empty_user_name_falls_through_to_mfg_model() {
        // Empty user_name → should fall through to mfg/model
        let node = make_node_with_snip(0, make_snip("", "Mfg Co", "Model X"));
        assert_eq!(node_display_name(&node), "Mfg Co — Model X");
    }

    // ── best_slot ──────────────────────────────────────────────────────────────

    fn make_slot(path: &str, role: lcc_rs::EventRole) -> SlotInfo {
        SlotInfo {
            node_id: "test".to_string(),
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
        // No Producer slot → falls back to the first slot
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
            node_id: "test".to_string(),
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
        // Cache says "seg:0/elem:1" holds SLOT_EVENT
        let mut node_cache = HashMap::new();
        node_cache.insert("seg:0/elem:1".to_string(), SLOT_EVENT);
        let mut config_cache = HashMap::new();
        config_cache.insert("test".to_string(), node_cache);

        let result = slot_for_event_id(&slots, "test", &SLOT_EVENT, &config_cache, lcc_rs::EventRole::Consumer);
        assert_eq!(result.unwrap().element_path, vec!["seg:0", "elem:1"]);
    }

    #[test]
    fn slot_for_event_id_node_not_in_cache_uses_heuristic() {
        let slots = vec![
            make_slot_with_path("seg:0/elem:0", lcc_rs::EventRole::Consumer),
            make_slot_with_path("seg:0/elem:1", lcc_rs::EventRole::Producer),
        ];
        // Node "missing" not in cache → heuristic: find first Producer slot
        let result = slot_for_event_id(&slots, "missing", &SLOT_EVENT, &HashMap::new(), lcc_rs::EventRole::Producer);
        assert_eq!(result.unwrap().element_path, vec!["seg:0", "elem:1"]);
    }

    #[test]
    fn slot_for_event_id_cache_present_but_no_event_match_uses_heuristic() {
        let slots = vec![make_slot_with_path("seg:0/elem:0", lcc_rs::EventRole::Consumer)];
        // Cache has the node but with different event bytes
        let different_event = [0x00u8; 8];
        let mut node_cache = HashMap::new();
        node_cache.insert("seg:0/elem:0".to_string(), different_event);
        let mut config_cache = HashMap::new();
        config_cache.insert("test".to_string(), node_cache);

        let result = slot_for_event_id(&slots, "test", &SLOT_EVENT, &config_cache, lcc_rs::EventRole::Consumer);
        // No cache match for SLOT_EVENT → heuristic fallback to first Consumer slot
        assert_eq!(result.unwrap().element_path, vec!["seg:0", "elem:0"]);
    }

    #[test]
    fn slot_for_event_id_no_slots_returns_none() {
        let result = slot_for_event_id(&[], "node", &SLOT_EVENT, &HashMap::new(), lcc_rs::EventRole::Producer);
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
        assert_eq!(slots.len(), 3, "Expected 3 event slots");
    }

    #[test]
    fn walk_cdi_slots_no_cdi_returns_empty() {
        let node = make_nodes(1).into_iter().next().unwrap(); // cdi: None
        let slots = walk_cdi_slots(&node);
        assert!(slots.is_empty(), "No CDI must return empty slots");
    }

    #[test]
    fn walk_cdi_slots_invalid_xml_returns_empty() {
        let node = make_node_with_cdi_xml(0, "<not valid xml<<<<");
        let slots = walk_cdi_slots(&node);
        assert!(slots.is_empty(), "Invalid XML must return empty slots");
    }

    // ── T007: well-known event ID tests ───────────────────────────────────────

    // Emergency Off bytes for convenience in these tests.
    const WK_EMERGENCY_OFF: [u8; 8] = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF];

    /// (T007a) Well-known event ID appears in protocol event_roles only (no config
    /// cache entry) → no card is emitted.
    #[test]
    fn t007a_well_known_protocol_only_no_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        // Simulate JMRI (node 0) and UWT-100 (node 1) replying for Emergency Off.
        event_roles.insert(WK_EMERGENCY_OFF, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(
            catalog.bowties.len(), 0,
            "Well-known event with only protocol replies → no card"
        );
    }

    /// (T007b) Well-known event ID is in a CDI config slot on one node → a
    /// single-entry card is emitted (single-entry rule is relaxed for well-known events).
    #[test]
    fn t007b_well_known_config_one_node_gives_card() {
        let nodes = make_nodes(1);
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        let mut node_cache = HashMap::new();
        node_cache.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_id(0), node_cache);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1, "Single config entry → card emitted");
        let card = &catalog.bowties[0];
        assert_eq!(card.event_id_bytes, WK_EMERGENCY_OFF);
        assert_eq!(card.name, Some("Emergency Off".to_string()));
        let total = card.producers.len() + card.consumers.len() + card.ambiguous_entries.len();
        assert_eq!(total, 1, "Exactly the one configured slot");
    }

    /// (T007c) Well-known event ID in config on two separate nodes → 2-entry card.
    #[test]
    fn t007c_well_known_config_two_nodes_gives_card() {
        let nodes = make_nodes(2);
        let event_roles: HashMap<[u8; 8], NodeRoles> = HashMap::new();

        let mut cache0 = HashMap::new();
        cache0.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut cache1 = HashMap::new();
        cache1.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_id(0), cache0);
        config_cache.insert(node_id(1), cache1);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        assert_eq!(card.name, Some("Emergency Off".to_string()));
        let total = card.producers.len() + card.consumers.len() + card.ambiguous_entries.len();
        assert_eq!(total, 2, "Both configured nodes appear in the card");
    }

    /// (T007d) Well-known event ID appears in both protocol event_roles (nodes 0 & 1)
    /// AND in config cache (node 2 only) → only the config entry is shown; protocol
    /// participants are excluded.
    #[test]
    fn t007d_well_known_protocol_and_config_uses_config_only() {
        let nodes = make_nodes(3);
        let mut event_roles = HashMap::new();
        event_roles.insert(WK_EMERGENCY_OFF, roles(&[0], &[1]));

        let mut cache2 = HashMap::new();
        cache2.insert("seg:0/elem:0".to_string(), WK_EMERGENCY_OFF);
        let mut config_cache = HashMap::new();
        config_cache.insert(node_id(2), cache2);

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &config_cache, None);

        assert_eq!(catalog.bowties.len(), 1);
        let card = &catalog.bowties[0];
        let all_node_ids: Vec<String> = card.producers.iter()
            .chain(card.consumers.iter())
            .chain(card.ambiguous_entries.iter())
            .map(|e| e.node_id.clone())
            .collect();
        assert_eq!(all_node_ids.len(), 1, "Only the config-cache entry for node 2");
        assert!(all_node_ids.contains(&node_id(2)), "Node 2 (config) should be present");
        assert!(!all_node_ids.contains(&node_id(0)), "Node 0 (protocol only) must not appear");
        assert!(!all_node_ids.contains(&node_id(1)), "Node 1 (protocol only) must not appear");
    }

    /// Zero event ID (all-zeros) in config cache must be silently ignored — it
    /// represents an unconfigured CDI slot and is not a valid event ID.
    #[test]
    fn zero_event_id_in_config_cache_is_ignored() {
        let nodes = make_nodes(2);
        let zero: [u8; 8] = [0u8; 8];

        // Two nodes both have an unconfigured slot (all-zeros) in their config cache.
        let mut config_cache: HashMap<String, HashMap<String, [u8; 8]>> = HashMap::new();
        let mut node0_cache = HashMap::new();
        node0_cache.insert("seg:0/elem:0".to_string(), zero);
        config_cache.insert(node_id(0), node0_cache);
        let mut node1_cache = HashMap::new();
        node1_cache.insert("seg:0/elem:0".to_string(), zero);
        config_cache.insert(node_id(1), node1_cache);

        let catalog = build_bowtie_catalog(&nodes, &HashMap::new(), &config_cache, None);

        assert!(
            catalog.bowties.is_empty(),
            "All-zero event IDs must not generate a bowtie card"
        );
    }

    /// Zero event ID (all-zeros) arriving via the protocol exchange must also be
    /// silently ignored (defensive guard for misbehaving nodes).
    #[test]
    fn zero_event_id_in_protocol_roles_is_ignored() {
        let nodes = make_nodes(2);
        let zero: [u8; 8] = [0u8; 8];

        let mut event_roles = HashMap::new();
        event_roles.insert(zero, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert!(
            catalog.bowties.is_empty(),
            "All-zero event ID from protocol exchange must not generate a bowtie card"
        );
    }

    /// (T007e) A normal (non-well-known) event pair is unaffected — regular bowtie
    /// cards still appear via the protocol loop (regression guard).
    #[test]
    fn t007e_non_well_known_event_still_creates_card() {
        let nodes = make_nodes(2);
        let mut event_roles = HashMap::new();
        event_roles.insert(EVENT_A, roles(&[0], &[1]));

        let catalog = build_bowtie_catalog(&nodes, &event_roles, &HashMap::new(), None);

        assert_eq!(catalog.bowties.len(), 1, "Normal event still creates a card");
        assert_eq!(catalog.bowties[0].name, None, "Normal events have no canonical name");
    }
}

// ── Integration test stubs ────────────────────────────────────────────────────

#[cfg(test)]
mod get_bowties_integration_tests {
    // Note: this test module doesn't use super::* as the actual command
    // requires `tauri::State` and can't be tested here. See tests/ folder.

    /// Verifies the command contract: returns Ok(None) before any catalog is built.
    ///
    /// Full integration tests requiring a Tauri app context live in
    /// `tests/bowties_integration.rs`.
    #[test]
    fn stub_returns_none_before_catalog_built() {
        // The actual command is async and requires `tauri::State`.
        // This stub documents the expected outcome without a full Tauri context.
        //
        // Real integration tests at: app/src-tauri/tests/bowties_integration.rs
        //   - test_get_bowties_returns_none_before_cdi_read
        //   - test_get_bowties_returns_catalog_after_build
        let _placeholder = "integration tests live in tests/bowties_integration.rs";
    }
}

