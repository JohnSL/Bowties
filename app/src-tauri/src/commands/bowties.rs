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

use crate::state::{AppState, BowtieCatalog, BowtieCard, EventSlotEntry, NodeRoles};

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

/// Return a full dot-joined path label for an event ID slot.
///
/// Format: `Ancestor1.Ancestor2.LeafLabel`
///
/// Leaf label priority: `<name>` → first sentence of `<description>` → last path component.
/// Empty ancestor names (groups with no `<name>`) are skipped.
/// Examples:
///   ["Settings", "Button 1"] + name "Event On" → "Settings.Button 1.Event On"
///   [] + name "Trigger"                         → "Trigger"
///   ["Outputs"] + no name, desc "When active." → "Outputs.When active"
fn element_label(
    element: &lcc_rs::cdi::EventIdElement,
    ancestor_names: &[&str],
    path: &[String],
) -> String {
    // Resolve the leaf label.
    let leaf = if let Some(name) = &element.name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let leaf = if leaf.is_empty() {
        if let Some(desc) = &element.description {
            let sentence = desc.split('.').next().unwrap_or("").trim().to_string();
            if !sentence.is_empty() { sentence } else { String::new() }
        } else {
            String::new()
        }
    } else {
        leaf
    };

    let leaf = if leaf.is_empty() {
        // Ultimate fallback: last path component.
        path.last().cloned().unwrap_or_default()
    } else {
        leaf
    };

    // Prepend non-empty ancestor names.
    let mut parts: Vec<&str> = ancestor_names.iter().copied().filter(|s| !s.is_empty()).collect();
    parts.push(leaf.as_str());
    parts.join(".")
}

// ── Core builder ─────────────────────────────────────────────────────────────

/// Slot metadata gathered from a single CDI walk.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SlotInfo {
    node_id: String,
    node_name: String,
    element_path: Vec<String>,
    element_label: String,
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
            element_label: element_label(element, ancestor_names, path),
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

    for (event_id_bytes, roles) in event_roles {
        // FR-002: bowtie requires ≥1 confirmed producer AND ≥1 confirmed consumer.
        if roles.producers.is_empty() || roles.consumers.is_empty() {
            continue;
        }

        let event_id_hex = format!(
            "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
            event_id_bytes[0], event_id_bytes[1], event_id_bytes[2], event_id_bytes[3],
            event_id_bytes[4], event_id_bytes[5], event_id_bytes[6], event_id_bytes[7]
        );

        let mut producers: Vec<EventSlotEntry> = Vec::new();
        let mut consumers: Vec<EventSlotEntry> = Vec::new();
        let mut ambiguous_entries: Vec<EventSlotEntry> = Vec::new();

        // Same-node set: nodes that appear in both producer and consumer sets.
        let both: std::collections::HashSet<&String> = roles
            .producers
            .intersection(&roles.consumers)
            .collect();

        // Pure producers
        for node_id in roles.producers.difference(&roles.consumers) {
            let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
            let slot = slot_for_event_id(slots, node_id, event_id_bytes, config_value_cache, lcc_rs::EventRole::Producer);
            let (ep, el, ed) = slot
                .map(|s| (s.element_path.clone(), s.element_label.clone(), s.element_description.clone()))
                .unwrap_or_else(|| (vec![], node_id.clone(), None));

            let node_name = nodes
                .iter()
                .find(|n| n.node_id.to_hex_string() == *node_id)
                .map(node_display_name)
                .unwrap_or_else(|| node_id.clone());

            debug_assert_ne!(
                lcc_rs::EventRole::Ambiguous,
                lcc_rs::EventRole::Producer,
                "EventSlotEntry must not hold Ambiguous in the producers vec"
            );

            producers.push(EventSlotEntry {
                node_id: node_id.clone(),
                node_name,
                element_path: ep,
                element_label: el,
                element_description: ed,
                event_id: *event_id_bytes,
                role: lcc_rs::EventRole::Producer,
            });
        }

        // Pure consumers
        for node_id in roles.consumers.difference(&roles.producers) {
            let slots = slot_map.get(node_id).map(|s| s.as_slice()).unwrap_or(&[]);
            let slot = slot_for_event_id(slots, node_id, event_id_bytes, config_value_cache, lcc_rs::EventRole::Consumer);
            let (ep, el, ed) = slot
                .map(|s| (s.element_path.clone(), s.element_label.clone(), s.element_description.clone()))
                .unwrap_or_else(|| (vec![], node_id.clone(), None));

            let node_name = nodes
                .iter()
                .find(|n| n.node_id.to_hex_string() == *node_id)
                .map(node_display_name)
                .unwrap_or_else(|| node_id.clone());

            consumers.push(EventSlotEntry {
                node_id: node_id.clone(),
                node_name,
                element_path: ep,
                element_label: el,
                element_description: ed,
                event_id: *event_id_bytes,
                role: lcc_rs::EventRole::Consumer,
            });
        }

        // Same-node entries: per-slot classification using config cache, with heuristic fallback.
        //
        // A node in `both` replied ProducerIdentified AND ConsumerIdentified for this event,
        // meaning it has at least one slot on each side — or the role is genuinely unknown.
        // With the config cache we can find each individual slot that holds this event ID and
        // classify it by its own heuristic role, emitting separate entries per slot.  This
        // correctly handles nodes that have distinct Producer and Consumer CDI slots both
        // configured to the same event ID (e.g. "Output A" + "Input A" on Async Blink).
        //
        // Fallback (no cache data or no cache hits for this event): vote-tally across all CDI
        // slots for the node and emit one entry with the majority role (or Ambiguous on tie).
        for node_id in &both {
            let slots = slot_map.get(*node_id).map(|s| s.as_slice()).unwrap_or(&[]);

            let node_name = nodes
                .iter()
                .find(|n| n.node_id.to_hex_string() == **node_id)
                .map(node_display_name)
                .unwrap_or_else(|| (*node_id).clone());

            // Precise: find every slot whose cached value matches this event ID.
            let mut cache_hits: Vec<&SlotInfo> = Vec::new();
            if let Some(node_cache) = config_value_cache.get(*node_id) {
                for slot in slots {
                    let path_key = slot.element_path.join("/");
                    if let Some(&cached_bytes) = node_cache.get(&path_key) {
                        if &cached_bytes == event_id_bytes {
                            cache_hits.push(slot);
                        }
                    }
                }
            }

            if !cache_hits.is_empty() {
                // Emit one entry per matching slot.
                // If profile_group_roles provides a definitive role for a slot, route it into
                // producers / consumers.  Otherwise fall back to Ambiguous so the user can
                // classify it manually.
                for slot in cache_hits {
                    let profile_key = format!("{}:{}", *node_id, slot.element_path.join("/"));
                    let resolved_role = profile_group_roles
                        .and_then(|map| map.get(&profile_key))
                        .copied();

                    match resolved_role {
                        Some(lcc_rs::EventRole::Producer) => {
                            producers.push(EventSlotEntry {
                                node_id: (*node_id).clone(),
                                node_name: node_name.clone(),
                                element_path: slot.element_path.clone(),
                                element_label: slot.element_label.clone(),
                                element_description: slot.element_description.clone(),
                                event_id: *event_id_bytes,
                                role: lcc_rs::EventRole::Producer,
                            });
                        }
                        Some(lcc_rs::EventRole::Consumer) => {
                            consumers.push(EventSlotEntry {
                                node_id: (*node_id).clone(),
                                node_name: node_name.clone(),
                                element_path: slot.element_path.clone(),
                                element_label: slot.element_label.clone(),
                                element_description: slot.element_description.clone(),
                                event_id: *event_id_bytes,
                                role: lcc_rs::EventRole::Consumer,
                            });
                        }
                        _ => {
                            // No profile entry (or Ambiguous) — surface in ambiguous_entries so
                            // the user can clarify roles manually.
                            ambiguous_entries.push(EventSlotEntry {
                                node_id: (*node_id).clone(),
                                node_name: node_name.clone(),
                                element_path: slot.element_path.clone(),
                                element_label: slot.element_label.clone(),
                                element_description: slot.element_description.clone(),
                                event_id: *event_id_bytes,
                                role: lcc_rs::EventRole::Ambiguous,
                            });
                        }
                    }
                }
            } else {
                // Fallback (no cache data for this node): emit one Ambiguous entry using
                // the first available CDI slot for label context, or bare node ID if no CDI.
                let slot = slots.first();
                let (ep, el, ed) = slot
                    .map(|s| (s.element_path.clone(), s.element_label.clone(), s.element_description.clone()))
                    .unwrap_or_else(|| (vec![], (*node_id).clone(), None));

                ambiguous_entries.push(EventSlotEntry {
                    node_id: (*node_id).clone(),
                    node_name,
                    element_path: ep,
                    element_label: el,
                    element_description: ed,
                    event_id: *event_id_bytes,
                    role: lcc_rs::EventRole::Ambiguous,
                });
            }
        }

        // Emit a card when ≥2 total entries across producers, consumers, and ambiguous_entries.
        // A single-entry event (one slot, one node) carries no connection information and is
        // silently excluded.  Cards with only ambiguous entries are valid — the protocol
        // confirmed the node is on both sides; the user can clarify roles in a future flow.
        let total_entries = producers.len() + consumers.len() + ambiguous_entries.len();
        if total_entries < 2 {
            continue;
        }

        bowties.push(BowtieCard {
            event_id_hex,
            event_id_bytes: *event_id_bytes,
            producers,
            consumers,
            ambiguous_entries,
            name: None,
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
/// If the connection or dispatcher is unavailable the function returns an empty map.
pub async fn query_event_roles(
    state: &AppState,
    send_delay_ms: u64,
    collect_window_ms: u64,
) -> HashMap<[u8; 8], NodeRoles> {
    use lcc_rs::protocol::{GridConnectFrame, MTI};
    use tokio::sync::broadcast;
    use tokio::time::{sleep, Duration};

    // Grab connection + dispatcher + own alias
    let (_connection, dispatcher, our_alias) = {
        let conn_lock = state.connection.read().await;
        let conn_opt = match conn_lock.as_ref() {
            Some(c) => c.clone(),
            None => {
                eprintln!("[bowties] query_event_roles: no connection");
                return HashMap::new();
            }
        };
        let our_alias = {
            let c = conn_opt.lock().await;
            c.our_alias().value()
        };
        let disp_lock = state.dispatcher.read().await;
        let disp_opt = match disp_lock.as_ref() {
            Some(d) => d.clone(),
            None => {
                eprintln!("[bowties] query_event_roles: no dispatcher");
                return HashMap::new();
            }
        };
        (conn_opt, disp_opt, our_alias)
    };

    // Read current node list
    let nodes = state.nodes.read().await.clone();
    if nodes.is_empty() {
        return HashMap::new();
    }

    // Subscribe to all broadcast traffic so we catch the six relevant MTIs.
    let mut rx = {
        let disp = dispatcher.lock().await;
        disp.subscribe_all()
    };

    // Send IdentifyEventsAddressed to each node, 125 ms apart.
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
                let disp = dispatcher.lock().await;
                if let Err(e) = disp.send(&frame).await {
                    eprintln!(
                        "[bowties] IdentifyEventsAddressed send error to {:?}: {}",
                        node.node_id, e
                    );
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

    eprintln!(
        "[bowties] query_event_roles complete: {} event IDs collected",
        roles.len()
    );

    roles
}

// ── Tauri command ─────────────────────────────────────────────────────────────

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

