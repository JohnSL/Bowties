# Research: Bowties Tab — Discover Existing Connections

**Feature**: `006-bowties-event-discovery`  
**Date**: 2026-02-22  
**Status**: Updated 2026-02-22 (v3 — OpenLCB_Java research complete; changed Identify Events from per-event-ID broadcast to per-node addressed; added RQ-11 EventState; clarified AKA/COL_CONTEXT_INFO source)

---

## RQ-1: Does the CDI XML schema have explicit `<producer>` / `<consumer>` elements to classify event slots?

**Decision**: No. There is no explicit XML-level producer/consumer distinction. All event slots use the identical `<eventid>` element.

**Rationale**: Per S-9.7.4.1 Appendix A, the `eventidType` complex type has only `offset` attribute and `name`/`description`/`map` child elements. There is no `role`, `type`, or `direction` attribute, and no `<producer>` or `<consumer>` wrapper element. lcc-rs `EventIdElement` reflects this: `{ name, description, offset }` — no role field.

**Alternatives considered**:
- Role attribute on `<eventid>` — doesn't exist in the standard.
- Parent-wrapper elements `<producer>` / `<consumer>` — don't exist in the standard.

**Implication for feature design**: Role cannot be determined from CDI XML alone. The primary mechanism is the Identify Events protocol exchange (RQ-2). The CDI heuristic (RQ-3) is a fallback for the same-node case only. Where neither approach resolves role, slots are shown as Ambiguous in the bowtie card for future user clarification (RQ-9).

---

## RQ-2: Primary classification — does the LCC protocol provide node-level producer/consumer ground truth?

**Decision**: Yes. The `Identify Events` message (TN-9.7.3.1) elicits `Producer Identified` and `Consumer Identified` replies from all nodes that produce or consume a given event. This is **Tier 0** — the primary classification mechanism. The CDI name heuristic (previously RQ-2, now Tier 1/2) is only used as a fallback for the same-node case described below.

**Mechanism**: After all CDI reads complete, the app sends one `IdentifyEventsAddressed` message (MTI 0x0488) *to each known node* — not a per-event-ID broadcast. The targeted node replies with a `ProducerIdentified` (MTI 0x0544/0x0545/0x0547) for every event it produces, and a `ConsumerIdentified` (MTI 0x04C4/0x04C5/0x04C7) for every event it consumes. The app collects all replies from all nodes before building the catalog.

**Reference implementation (JMRI `EventTablePane.sendRequestEvents`)**: JMRI sends addressed `IdentifyEventsAddressedMessage` to each node in `MimicNodeStore`, with a 125 ms delay between sends, then schedules UI refreshes at 1 s, 2 s, and 4 s after the last send. The Bowties app will use a configurable reply-collection timeout (default 500 ms after last send) rather than scheduled refreshes, but the per-node addressed approach is identical.

**Why addressed-per-node over per-event-ID broadcast**: One `IdentifyEventsAddressed` per node retrieves *all* of that node's events in one round trip. Sending `IdentifyProducers`+`IdentifyConsumers` per unique event ID would require 2 × |event IDs| messages vs |nodes| messages — almost always more traffic, and no benefit.

**Cross-node case — fully resolved by Tier 0**:  
If Node A → `Producer Identified` and Node B → `Consumer Identified` for event X, then every `EventId` CDI field on Node A containing value X is a **producer element**, and every such field on Node B is a **consumer element**. No heuristic needed.

**Same-node case — residual ambiguity**:  
If Node A sends both `Producer Identified` AND `Consumer Identified` for event X (the node both produces and consumes the same event), then among Node A's CDI fields containing value X, we cannot determine element-level roles from the protocol reply alone. The CDI heuristic (Tier 1/2 below) is applied as a fallback. If the heuristic also fails, the slot is marked **Ambiguous** and surfaced for user clarification in a future phase (not silently excluded — see RQ-9).

**Timing**: The Identify Events query fires once, immediately after all `read_all_config_values` calls complete. It collects replies with a short timeout (default 500 ms). This is a new active network exchange — Spec Assumption #1 is revised to accommodate it (see plan.md).

**Alternatives considered**:
- Passive capture only (record `Producer/Consumer Identified` as they arrive during normal operation) — rejected; coverage is incomplete unless all nodes have announced recently.
- CDI heuristic as primary — rejected in favour of protocol ground truth; heuristic is now fallback only.

---

## RQ-3: What heuristic signals identify producer vs consumer event slots (same-node fallback)?

**Decision** (revised from original RQ-2): Two-tier heuristic applied **only when Tier 0 (Identify Events) leaves ambiguity** (same-node case). Two-tier heuristic: (1) parent group name keywords; (2) element description phrase patterns. Both evaluated in order; slot classified as Ambiguous if neither signal fires.

**Rationale**: Analysis of real LCC nodes (standard examples, JMRI-configured hardware) reveals two consistent conventions:

### Tier 1 — Parent group name (case-insensitive, substring match)
| Keyword match | Inferred role |
|---|---|
| "producer", "producers", "input", "inputs", "generated", "output activat" | **Producer** |
| "consumer", "consumers", "output", "outputs", "responded", "activates turnout" | **Consumer** |

Many node developers name their groups explicitly (e.g., `<group><name>Producers</name>`, `<group><name>Consumers</name>`). This is the most reliable signal.

### Tier 2 — Element `<description>` phrase patterns (case-insensitive)
| Phrase pattern | Inferred role |
|---|---|
| Starts with / contains "generated when", "sent when", "produced when", "trigger when" | **Producer** |
| Starts with / contains "when this event", "activates", "causes", "responds to" | **Consumer** |

Standard example (TN-9.7.4.1 §3.2): `<description>Generated when input line goes active</description>` → Producer; `<description>When this event arrives, turnout moves to closed position</description>` → Consumer.

### Ambiguous result (same-node, heuristic fails)
If neither tier fires for a slot on a same-node case, the slot is classified **Ambiguous**. It is surfaced in the bowtie card in a dedicated ambiguous section (not silently excluded), pending user clarification in a future phase. See RQ-9 for the UI treatment.

**Alternatives considered**:
- Match-only approach (form bowties from event IDs found on ≥2 different nodes, skip P/C classification at element level) — rejected; we need Tier 0 data anyway, and the heuristic is useful for same-node disambiguation.
- Machine-learning text classifier — far too heavy for an embedded desktop tool; heuristics are sufficient given the conventions used in real nodes.

---

## RQ-4: Where does the role classification logic live in the codebase?

**Decision**: New module `lcc-rs/src/cdi/role.rs` — a pure Rust function with no I/O dependencies.

**Rationale**: Putting the classifier in lcc-rs:
- Keeps it testable without Tauri (unit tests in the library crate).
- Keeps CDI-domain knowledge in the library layer (per constitution §Architecture Constraints).
- Makes it available for future use in other Rust consumers of lcc-rs.

The new public API surface:

```rust
// lcc-rs/src/cdi/role.rs
pub enum EventRole { Producer, Consumer, Ambiguous }

/// Classify a single event ID element given its CDI context.
/// Only called when Tier 0 (Identify Events protocol) is inconclusive
/// (i.e., the same node replied both Producer Identified and Consumer Identified).
/// `parent_group_names` is a slice of all ancestor group names (outermost-first).
pub fn classify_event_slot(
    element: &EventIdElement,
    parent_group_names: &[&str],
) -> EventRole
```

**Alternatives considered**:
- Logic in the Tauri backend command — rejected; would duplicate CDI-domain logic outside the library.
- Logic in the frontend — rejected; string matching on raw XML in TypeScript would bypass the parsed structure already available in Rust.

---

## RQ-5: Does the feature require a new network protocol exchange?

**Decision**: Yes — one new exchange. After all `read_all_config_values` calls complete, the app sends `IdentifyEventsAddressed` to each known node (125 ms between sends) and collects `Producer Identified` / `Consumer Identified` replies before building the bowtie catalog.

**Rationale**:
- `read_all_config_values` reads every CDI field's live byte value, populating `AppState.nodes`.
- One `IdentifyEventsAddressed` is sent per node (not per event ID) to retrieve each node's complete event role list in one round trip.
- The bowtie builder runs after the reply-collection window closes — combining protocol role data with the CDI slot addresses already in `AppState`.
- FR-007 prohibits a "second full network scan" — the Identify Events exchange is targeted (one message per *known* node, not a topology rediscovery), so it does not violate FR-007's intent.

**Timing model**:
```
  For each node in AppState (spaced 125 ms apart):
    send IdentifyEventsAddressed(nodeID)
  Wait (collection_timeout) ms after last send (default 500 ms)
  Build BowtieCatalog from collected NodeRoles
  Emit cdi-read-complete
```

---

## RQ-6: How does the app know when all CDI reads and role queries are complete?

**Decision**: Two-phase completion. (1) CDI reads complete when the last node's `read_all_config_values` returns. (2) Identify Events queries fire immediately after, and completion is detected after the collect window expires. The `cdi-read-complete` Tauri event is emitted only after both phases are done, carrying the finished `BowtieCatalog`.

**Rationale**: The frontend only needs one signal to enable the tab and display the catalog. Merging both phases into a single "ready" event keeps the frontend simple.

**Alternatives considered**:
- Two separate events (cdi-reads-done + bowties-ready) — more granular but adds frontend complexity for no current benefit.
- Polling from frontend — rejected; wasteful and racy.

---

## RQ-7: What tab/navigation structure does the app currently use?

**Decision**: The app already has a multi-route SvelteKit structure (`/config`, `/traffic`). A new `/bowties` route will be added. The main layout or navigation component must be updated to include a Bowties tab.

**Rationale**: The root `+page.svelte` currently has a flat layout with no tab bar. The existing routes (`/config`, `/traffic`) imply a tab or nav-link bar somewhere. The Bowties tab (FR-013) must be visually disabled (greyed out) until both CDI reads and Identify Events collection are complete.

**Current `routes/` layout:**
```
routes/
  +layout.svelte    (thin shell — just renders children)
  +page.svelte      (discovery/connection UI + config view — the main page)
  config/
    +page.svelte    (ConfigSidebar + SegmentView)
  traffic/
    +page.svelte    (traffic monitor)
```

---

## RQ-8: What is the bowtie build algorithm?

**Decision**: Two-stage — first apply Tier 0 (protocol replies) to assign node-level roles, then walk CDI slots to produce element-level entries, then group by event ID.

**Algorithm**:
```
Stage A — Collect protocol roles (after Identify Events exchange):
  NodeRoles[event_id] = { producers: Set<node_id>, consumers: Set<node_id> }

Stage B — Build EventSlotEntry list:
  For each discovered node N:
    For each event ID slot S in N's read config values with value V:
      node_is_producer = NodeRoles[V].producers.contains(N.node_id)
      node_is_consumer = NodeRoles[V].consumers.contains(N.node_id)

      if node_is_producer AND NOT node_is_consumer:
        role = Producer
      elif node_is_consumer AND NOT node_is_producer:
        role = Consumer
      elif node_is_producer AND node_is_consumer:
        // Same-node case: apply CDI heuristic
        role = classify_event_slot(S.cdi_element, S.ancestor_group_names)
        // role may be Producer, Consumer, or Ambiguous
      else:
        // Node didn't reply for this event: skip (no protocol confirmation)
        continue

      emit SlotEntry { node_id: N, element_path, label, event_id_bytes: V, role }

Stage C — Group and filter:
  Group SlotEntry list by event_id_bytes:
    for each group G:
      producers  = G.filter(role == Producer)
      consumers  = G.filter(role == Consumer)
      ambiguous  = G.filter(role == Ambiguous)
      if producers.empty AND consumers.empty: skip entirely (no confirmed sides)
      emit BowtieCard { event_id, producers, consumers, ambiguous_entries, name: None }

Stage D — Sort BowtieCard list by event_id_bytes (lexicographic)
```

A `BowtieCard` is emitted as long as it has ≥1 confirmed producer AND ≥1 confirmed consumer. Ambiguous entries from a same-node are added alongside and shown in the card's UI for later clarification.

---

## RQ-9: How are ambiguous same-node slots surfaced to the user?

**Decision**: Ambiguous `EventSlotEntry` objects are added to a separate `ambiguous_entries` list on `BowtieCard`. The card renders a third section ("Unknown role — needs clarification") listing these entries. No user action is possible in this phase (read-only); the entries are shown with their CDI-path label and an explanatory tooltip. A future phase will add a "clarify role" action that lets the user assign Producer or Consumer and stores the decision.

**Rationale**:
- Silently excluding these entries would hide real configuration that the user needs to understand.
- Showing them clearly with a "needs clarification" label matches the UX-First principle and sets up the future edit flow.
- The future phase needs to persist role decisions per `(node_id, element_path)` — this is why the spec notes "name storage is out of scope for this phase" alongside role storage.

---

## RQ-10: How should the "Used in" cross-reference (FR-008/FR-009) work technically?

**Decision**: The `EventSlotRow` component receives an optional `used_in` prop populated from the bowtie store; clicking it calls a navigation helper that routes to `/bowties` with a `highlight` query parameter.

**Rationale**:
- The bowties store holds the mapping of event_id → bowtie card.
- A derived store produces a `Map<eventIdHex, BowtieCard>` for O(1) lookup per slot.
- Navigation: SvelteKit's `goto('/bowties?highlight=<eventIdHex>')` + the Bowties page reads the param and scrolls to and highlights the matching card.
- No backend change needed for navigation.

---

## RQ-11: Do `ProducerIdentified` / `ConsumerIdentified` replies carry `EventState`, and do we use it?

**Decision**: The replies do carry an `EventState` field (`Unknown` / `Valid` / `Invalid`), but we **do not use it** in Phase 1 of this feature.

**Rationale**: The LCC spec defines `EventState` to report whether the event's "state" is currently active (`Valid`), inactive (`Invalid`), or unknown (`Unknown`). This is orthogonal to role classification — a node can produce an event in any state. JMRI's event table (reference implementation) ignores `EventState` in its `handleProducerIdentified` and `handleConsumerIdentified` handlers; it only records `source NodeID → event ID` pairs.

**Future consideration**: `EventState` could be surfaced on a BowtieCard (e.g., a dot indicator showing whether the event was last observed active or inactive). This is explicitly out of scope for Phase 1. If added later, it should be stored as an optional field on `EventSlotEntry`.

---

## RQ-12: What is the source of JMRI's "Also Known As" / `COL_CONTEXT_INFO` column, and how does it inform our `element_label` design?

**Decision**: JMRI's `COL_CONTEXT_INFO` (column 6, "Also Known As") is populated from `EventTable.getEventInfo(eventID).getAllEntries()` where each `EventTableEntry.getDescription()` is printed one line per entry. These descriptions are **not** read from CDI XML — they come from JMRI's own configured objects (sensors, turnouts, reporters) that register themselves with the global `EventTable` via `iface.getEventTable().addEvent(eventID, "Sensor:LS42 active")`. CDI-derived names are NOT part of JMRI's AKA column.

**What this means for our `element_label`**: The Bowties app draws its label from CDI directly (we own the CDI parse), not from a JMRI-style global EventTable. The correct label hierarchy for an `EventSlotEntry` is:

1. CDI element `<name>` (if non-empty) → primary label
2. CDI element `<description>` first sentence (if non-empty) → secondary label
3. CDI element path (group names + element index) → fallback

The user's reference to "JMRI's AKA column coming from CDI name/description" was directionally correct but mechanically different: JMRI's AKA comes from JMRI-managed turnout/sensor names (which often *mirror* CDI names because the user typed the same string in both places). In our app, the CDI name IS the authoritative source and is read directly.

**Node display name**: For the producer/consumer node names shown in a bowtie card, use `SimpleNodeIdent.user_name` if non-empty, otherwise `"{mfg_name} — {model_name}"` — same convention as JMRI `EventTablePane.recordProducer/recordConsumer`.

---

## Summary of Resolved Clarifications

| # | NEEDS CLARIFICATION | Resolution |
|---|---|---|
| 1 | CDI producer/consumer XML elements | None exist in the standard (S-9.7.4.1) |
| 2 | Primary classification mechanism | Identify Events protocol (Tier 0) — addressed per-node, 125 ms spacing |
| 3 | Same-node fallback heuristic | CDI group name + description keywords (Tier 1/2) |
| 4 | Classifier code location | New `lcc-rs/src/cdi/role.rs` module |
| 5 | New network exchange needed? | Yes — `IdentifyEventsAddressed` per node after CDI reads (not per event ID) |
| 6 | CDI+role-query completion signal | Single `cdi-read-complete` event after both phases done |
| 7 | Tab/navigation structure | New `/bowties` route in existing SvelteKit structure |
| 8 | Build algorithm | Two-stage: protocol roles → CDI slot walk → group by event ID |
| 9 | Same-node ambiguous UI | Show in `ambiguous_entries` section of card; future phase adds clarify action |
| 10 | "Used in" cross-reference | Derived frontend store + SvelteKit `goto` with highlight param |
| 11 | EventState in protocol replies | Present in replies, ignored in Phase 1 (same as JMRI reference impl) |
| 12 | AKA / element_label design | CDI `<name>` → `<description>` first sentence → CDI path; node name = user name \|\| mfg+model |
