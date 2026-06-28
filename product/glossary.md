# Bowties Glossary

Domain vocabulary for humans and AI agents working in the Bowties codebase. Canonical terms are **bolded**; alternatives to avoid are listed under _Avoid_.

## Protocol

**LCC**:
Layout Command Control — the NMRA standard (S-9.7) for decentralized node-to-node communication over model railroad layouts.
_Avoid_: OpenLCB protocol (LCC is the standard; OpenLCB is the reference implementation)

**OpenLCB**:
The reference implementation specification for the LCC standard, defining message formats, timing, and state machines.
_Avoid_: "the LCC standard" (OpenLCB implements the standard), "a protocol" (it is a specification family)

**CAN**:
Controller Area Network — the physical and link-layer transport used by LCC, delivering 8-byte frames over a bus topology.
_Avoid_: "the protocol" (CAN is transport, not the protocol), "LCC" (conflating layers)

**Node**:
A discrete LCC device on the network with a unique 6-byte Node ID that participates in message exchange.
_Avoid_: device, module, endpoint

**Node ID**:
A globally unique 6-byte (48-bit) identifier assigned to a node at manufacture, displayed in dotted-hex format `05.02.01.02.00.FF` and normalized to canonical form for comparison.
_Avoid_: address (that is the alias), serial number

**Node Alias**:
A dynamically allocated 12-bit temporary address assigned during CAN startup, used in frame headers to reduce bandwidth.
_Avoid_: address (alone), Node ID (permanent vs dynamic), short ID

**Event ID**:
An 8-byte (64-bit) unique identifier used by producers and consumers to communicate without direct knowledge of each other. Stored and compared in canonical contiguous hex form (`0201570002D90100`); displayed to users in dotted-hex form (`02.01.57.00.02.D9.01.00`). See ADR-0010.
_Avoid_: message ID, signal ID, event code

**Producer**:
A node or element that creates and transmits an Event ID onto the network.
_Avoid_: sender, publisher, transmitter

**Consumer**:
A node or element that listens for and responds to an Event ID transmitted by producers.
_Avoid_: receiver, subscriber, responder

**CDI**:
Configuration Description Information — XML metadata from a node describing its memory layout, configuration fields, and semantic meaning of each byte range.
_Avoid_: CDI data, config schema, memory map

**SNIP**:
Simple Node Information Protocol — a datagram-based query/response that retrieves human-readable metadata from a node (manufacturer, model, user name, versions).
_Avoid_: node info, discovery protocol (discovery queries presence; SNIP queries metadata)

**PIP**:
Protocol Identification Protocol — a query/response that returns a node's capability flags indicating which LCC features it implements.
_Avoid_: protocol version, feature flags

**MTI**:
Message Type Indicator — a field in the CAN frame header that identifies the type and direction of an LCC message.
_Avoid_: message type (alone), message code, frame type

**Datagram**:
A multi-frame LCC protocol unit (1–72 bytes across 1–10 CAN frames) with integrity checking, used for config reads, SNIP, and other transfers exceeding 8 bytes.
_Avoid_: message (single-frame), packet, stream (streams are a separate LCC protocol)

## App Model

**Bowtie**:
A card representing one logical connection (one shared Event ID) linking one or more producers to one or more consumers, shown with producers on the left and consumers on the right.
_Avoid_: connection (vague), event (UI representation, not the protocol event), link

**Connector**:
A modular physical slot or daughterboard interface on a carrier board, managed via `connectorSlots` and `daughterboardReferences` in the node profile.
_Avoid_: slot (connectors are physical; slots are CDI config areas), port, interface

**Pill**:
A searchable dropdown component (`PillSelector`) rendered as a compact pill button that opens a selectable list and closes when a selection is made or another pill opens.
_Avoid_: dropdown, selector, autocomplete (PillSelector has dedicated mutual-exclusion behavior)

**Connection Element**:
A node/element pair (node display name + CDI path) that participates in a bowtie as either a producer or consumer.
_Avoid_: event slot (the field holding the event ID), element (ambiguous alone), role (the element has a role)

**Display Name**:
The human-readable label shown for a node in the UI, resolved via the Display Name Fallback chain.
_Avoid_: node name, label, friendly name

**Display Name Fallback**:
The priority-ordered resolution rule: user_name → manufacturer+model → model → Node ID hex.
_Avoid_: name resolution, fallback chain (imprecise)

**Layout**:
A persistently saved YAML file containing the node tree, offline changes, sync session state, bowtie metadata, and connector selections for a particular LCC layout.
_Avoid_: config file, session, project, template

**NodeKey**:
A unified identifier for nodes in the Bowties backend and frontend. On the frontend it is a branded discriminated union (`LiveNodeKey | PlaceholderNodeKey`) defined in `app/src/lib/utils/nodeKey.ts`; on the backend and on the wire it is the canonical `String` form. Live nodes use their canonical 12-hex `NodeID` (e.g. `"050101011402"`). Placeholder nodes use `"placeholder:<uuidv4>"`. The Proxy Registry, layout layer, and offline-change layer are keyed by `NodeKey`. See ADR-0008 and ADR-0010.
_Avoid_: node_id (ambiguous — could mean the 6-byte LCC Node ID or the string key), node identifier, BrandedNodeKey (renamed to NodeKey 2024-12)

**Placeholder (Board)**:
A node that exists only in the Bowties layout, not on the physical LCC bus. Represented as a `NodeSnapshot` with `node_id: None` and `profile_stem: Some(...)`. In memory, a `SynthesizedNodeProxy` in the Proxy Registry. The placeholder factory (`placeholder.rs`) synthesizes what bus discovery would have produced. All event ID fields are pre-filled with `[0u8; 8]` (all-zero, excluded from bowtie binding by the zero-prefix rule). See ADR-0009.
_Avoid_: virtual node (implies protocol presence), stub, mock

**Information Channel** (a.k.a. **Channel**):
A typed, named representation of a single piece of layout-meaningful information (e.g., "Block 7 Occupancy", "Block 5 Indicator LED") independent of protocol details. Channels are the foundational binding entity for railroad-level abstractions — facilities bind channels by **Role**, not pins. Persisted in `channels.yaml`. Key attributes: stable ID (UUID v4), user-assigned name, **Role**, **Style**, **Ownership**, **Binding**. See Spec 015 (original shape) and Spec 018 (role/style/ownership extension). Every channel is bound to specific hardware; channels without a binding are not a persistable state.
_Avoid_: sensor (protocol-specific), input (hardware-level), event (protocol-level), "logical channel" (channels always have a binding), "resource" (Style is the implementation-shape concept)

**Channel Type** _(retired by Spec 018; superseded by **Role**)_:
Pre-018 classification on a channel. Replaced by the **Role** + **Style** pair: Role replaces the state-vocabulary part; Style replaces the hardware-shape part. Removed from the persistent schema in the final implementation slice of Spec 018.
_Avoid_: using `channelType` in new code or new specs — say **Role** for state vocabulary and **Style** for hardware shape.

**Hardware Reference** _(extended by Spec 018; renamed to **Binding**)_:
The backing physical source for a channel. Pre-018: `{ nodeKey, connector, input }`. Post-018: a discriminated `Binding` shape — `{ kind: 'connectorInput', nodeKey, connector, input }` for BOD-style inputs, `{ kind: 'lampRow', nodeKey, rowOrdinal }` for Direct Lamp Control rows, with the discriminator chosen by the channel's **Style**. Displayed via `resolveNodeName(nodeKey)` — never the raw key.
_Avoid_: source, origin, pin reference, `hardwareRef` (in new code, use `binding`)

**PCER (Producer/Consumer Event Report)**:
An LCC message (MTI 0x195B4) carrying an 8-byte Event ID, broadcast on the bus when a producer fires. Every node on the bus receives every PCER. This is the primary mechanism for real-time state communication (e.g., a BOD board sending "block occupied").
_Avoid_: event message (too vague), notification

**Event State Store**:
A session-scoped, transient frontend store that records every PCER event received from the LCC bus. Maintains a map from Event ID (hex) to last-seen timestamp. Channel-unaware — records all events regardless of whether they match a known channel. Channel state is derived at display time by joining the event ledger with resolved event IDs. See Spec 016.
_Avoid_: event log (implies persistence), event monitor (that's a future UI), subscription store

**Event Mapping**:
Profile-declared mapping from a channel type's abstract states (e.g., occupied/clear) to specific CDI producer event leaf indices within the channel's hardware scope. Part of the `channelInputs` section in daughter board metadata. This is how the system knows which configured event ID means "occupied" vs "clear" for a given board model.
_Avoid_: event binding, event wiring (that's bowtie creation)

**Railroad Tab**:
The third (rightmost) tab in the main application view. Hosts the **Channels Panel** (hardware-organised list of every channel in the layout) and the **Facilities Section** (named instances of behavior templates). The home for layout-level railroad abstractions.
_Avoid_: channel tab, inventory tab, layout tab (overloaded with "layout file")

**Channels Panel**:
The hardware-organised list of every channel in the layout, rendered in the **Railroad Tab**. Grouped by node + subsystem (not by **Channel Type**). Each row shows ownership, role, style, identity (the pin(s) claimed), name, live state, and the slot/facility currently bound to it (or "unbound"). Functions standalone — hardware verification needs no facility. See Spec 018 (FR-031).
_Avoid_: channel inventory (pre-018 surface, retired), channel list, hardware panel

## Facilities System

**Facility**:
A named, persisted instance of a **Behavior Template**, with one **Facility Slot** per template-declared slot. Each slot is optionally bound to one **Channel** by role. A facility has exactly one **Facility Status** (`Incomplete` if any slot empty; `Wired` if all slots filled, with the underlying bowtie(s) created via the existing bowtie creation mechanism). Persisted in `facilities.yaml`. A facility is a **UI veneer** over bowties — Spec 018 introduces no new sync, persistence, or deployment machinery for facilities themselves.
_Avoid_: arrangement, signal, device, behavior, automation (these describe future template families; "facility" is the generic noun)

**Behavior Template**:
A declared template for a **Facility**. Carries a stable template ID, display name, ordered list of slot declarations (each with a slot label, producer/consumer designation, and required **Role**), and a mapping from producer-side semantic states to consumer-side commands. The first template is **Block Indicator** (`occupied → lit`, `clear → unlit`). Hardcoded in `bowties-core/src/behavior_templates/` in Spec 018; future declarative YAML loader is Future Considerations.
_Avoid_: template (alone — ambiguous with structure profile), scenario, recipe (recipes are profile-level), preset

**Facility Slot**:
A named position within a **Facility**, carrying a producer/consumer designation, a required **Role**, and an optional bound **Channel** reference. Empty slots are first-class — they are how an incomplete plan is represented. A channel is bound to at most one facility slot in this slice. Distinct from **CDI Slot** / CDI group (which is a config structure, not a binding).
_Avoid_: "slot" alone (qualify as "facility slot" or "CDI slot"), placeholder, hole

**Role** (Channel Role):
What a **Channel** does in the layout — its state vocabulary plus the slot-binding contract. A **Facility Slot** binds by role. Examples: `block-occupancy` (states `unknown` / `occupied` / `clear`); `lamp-indicator` (states `unknown` / `lit` / `unlit`); future `signal-aspect-3-color`. Every role includes `unknown` as a first-class state for "no observation yet". State values name real-world intent, never electrical or boolean abstractions (`true`/`false`, `on`/`off`). Roles are declared in Rust code (typed enums for exhaustive match safety); state-vocabulary changes are a code change, not a YAML change. Internally a role corresponds to an interface in the OO sense; user-facing language is always "role".
_Avoid_: channel type (pre-018 term, retired), kind, classification, role (in another sense — e.g., "event role" is a separate Spec 014 concept; qualify when both are in scope)

**Style** (Channel Style):
The specific hardware-shape realisation of a **Role** — the pins claimed, the producer/consumer event-leaf mapping, the constraint contract over the claimed pins' CDI fields, and whether instances are user-creatable. Multiple styles may realise the same role (e.g., a future `2-led-bicolor-aspect` and `3-led-direct-aspect` both realising a `signal-aspect-3-color` role). Examples: `bod-block-detector-input` (1 input pin → `block-occupancy`, auto-created by BOD daughter-board selection); `single-led-direct-lamp` (1 Direct Lamp Control row → `lamp-indicator`, user-creatable via Add channel). Declared in profile YAML; the in-code registry maps `styleId → realisation`. Internally a style corresponds to an implementation class realising the role's interface; user-facing language is always "style".
_Avoid_: implementation, variant, hardware kind, role-impl, "resource" (Style replaces the older "resource type" framing)

**Channel Ownership**:
The lifecycle classification of a **Channel**, deciding who may destroy it. Two values: `hardware-owned` (auto-created when a hardware-config choice fixes the role of pins — e.g., BOD daughter-board selection — and auto-deleted when that selection is cleared or changed; user rename does not change ownership) and `user-owned` (created via a **Facility Slot**'s *Add channel* action and deleted when removed from its only slot in this slice — no ref-counting yet). When a channel is destroyed, any facility slot bound to it becomes empty; if the facility was **Wired** it returns to **Incomplete**.
_Avoid_: provenance, source, origin, "system channel" / "manual channel" (vague)

**Binding** (Channel Binding):
The discriminated reference from a **Channel** to the specific pin(s) it claims on a specific subsystem on a specific node. Shape: `{ kind: 'connectorInput' | 'lampRow' | …, …style-specific fields }`. The `kind` MUST match the **Style**'s declared binding shape. Replaces the pre-018 `hardwareRef` field; see also that entry.
_Avoid_: hardware reference (pre-018 term), wiring (wiring is the bowtie/bus side), target, anchor

**Facility Status**:
The derived lifecycle status of a **Facility**, computed by a pure function over slot fullness — never stored on persistence (ADR-0004). Two values: `Incomplete` (at least one slot empty; no underlying bowtie(s) exist for the template's mapping) and `Wired` (all slots filled; underlying bowtie(s) exist, created via the existing bowtie creation mechanism). Transitions are automatic; there is no separate "deploy" action in Spec 018.
_Avoid_: "Live" (the original Spec 018 draft used this; renamed to **Wired** to distinguish structural completeness from bus-sync state, which the existing layered storage system owns), "state" (Channel has state; Facility has status), "ready" / "complete" (vague)

**Style Constraint Contract**:
A declaration on a **Style** describing how its **Channel**s manage the CDI fields of the pins they claim — fix a field to a specific value, restrict it to a subset of allowed values, or hide it entirely. Unmanaged fields stay freely user-editable. Spec 018 repositions the existing BOD-family `validityRules` (today on daughter-board entries) onto the `bod-block-detector-input` style; the renderer (the existing profile-driven relevance/validity surface) is unchanged. The user cannot put a managed field into a state that would invalidate the channel's semantics.
_Avoid_: validity rules (the renderer mechanism; the contract is the source declaration), restrictions, locks, schema

**Hardware-owned Channel**:
See **Channel Ownership**.

**User-owned Channel**:
See **Channel Ownership**.

## Architecture Roles

**Route**:
A Svelte page component (`+page.svelte`) that composes screens, owns visible page-level state, and wires user actions to orchestrators or stores.
_Avoid_: page, screen, controller, view

**Component**:
A Svelte component that renders state, handles local UI interactions, and emits intent events, delegating multi-step workflows elsewhere.
_Avoid_: view, widget, presenter

**Orchestrator**:
A TypeScript module that owns multi-step async workflows, lifecycle transitions, backend call sequencing, and cross-store coordination.
_Avoid_: service, manager, controller, handler

**Store**:
A Svelte store module that owns durable frontend state, deterministic transitions, and derived values used by routes and components.
_Avoid_: state manager, service, repository

**Util**:
A pure-function module that owns normalization, formatting, comparison, and translation logic reused across multiple layers, with no hidden side effects.
_Avoid_: helper (util is a helper), service (services have state/side effects)

**Transport Actor**:
An internal runtime abstraction that manages the physical CAN/serial connection and dispatches frames to protocol handlers.
_Avoid_: transport layer, serial handler, connection (Connection owns the Transport Actor)

**Node Proxy**:
A per-node actor (thread) that owns all mutable state for a single discovered LCC node (SNIP, PIP, CDI cache, config values, tree snapshots), communicating via message passing through `NodeProxyHandle`.
_Avoid_: node handler, node manager, node service

## Data & Workflow

**Sync Session**:
A temporary session object that classifies offline changes into conflict/clean/already-applied/node-missing rows, presenting unresolved conflicts to the user for apply/skip choices.
_Avoid_: sync state, sync job, sync operation

**Modified Value**:
A node configuration field value that has been changed from its baseline (bus state when layout was opened) and persisted in the node tree's `modified_value` field.
_Avoid_: edited value, override, pending value

**Pending Change**:
An offline-authored modification to a node configuration that has not yet been synced to the physical bus, ready to sync at next connection.
_Avoid_: unsaved change, draft

**Offline Change**:
A configuration modification authored or recorded while the layout is disconnected from the bus, stored in `offline-changes.yaml` alongside the layout file and replayed during reconciliation.
_Avoid_: queued change, saved change

**Config Read Session**:
A multi-step orchestrated process that sequentially reads CDI and PIP from each discovered node, updates the node tree, and signals completion.
_Avoid_: config read (alone), CDI read (CDI read is one step), configuration sync

## Profile System

**Profile**:
A set of structured YAML/JSON files extracted from a node's CDI XML and PDF manual, enabling guided configuration workflows.
_Avoid_: node profile (redundant), configuration template, metadata file

**Structure Profile**:
The `.profile.yaml` file shipped with the app containing event roles, relevance rules, and connector definitions for a specific node model.
_Avoid_: extraction profile (that is the full 7-file authoring set)

**Relevance Rules**:
Configuration conditions identifying when certain CDI sections become irrelevant based on other field values (e.g., "consumer events irrelevant when Output Function = No Function").
_Avoid_: visibility rules, conditional rules, dependency rules

**Cascade Rules**:
Configuration repairs staged by the profile system when connector selections invalidate current field values, automatically written as part of sync apply.
_Avoid_: repair rules, auto-fix, side-effect rules

**Guided Configuration**:
A UI/workflow mode that uses profile relevance rules and field descriptions to present only applicable configuration sections and step-by-step recipes to users.
_Avoid_: assisted configuration, wizard (implies linear flow; guided is contextual), smart forms

**Fully Captured**:
A node whose CDI tree has been read into `nodeTreeStore` *and* is not currently in `partialCaptureNodesStore`. The threshold from ADR-0007 — the *tree-completeness* half of persistability. A fully-captured node is not yet persistable on its own; live nodes additionally require **Config Read**.
_Avoid_: "complete" (ambiguous), "captured" alone (drops the partial-capture exclusion)

**Config Read**:
Membership in `configReadNodesStore` — the user has run "Read all configuration" against the node and real values are in hand. Distinct from "the CDI tree is loaded" (that is **Fully Captured**). For placeholders this concept does not apply: a placeholder is persistable as soon as it exists.
_Avoid_: "values loaded", "fetched" (both ambiguous with CDI fetch)

**Persistable in Layout (`isPersistableInLayout`)**:
The single predicate governing whether a node can be promoted into the saved layout file: `isFullyCaptured(key) ∧ (isConfigRead(key) ∨ key.kind === 'placeholder')`. Owned by `effectiveNodeStore` (ADR-0011). Save, the orange in-memory-changes dot, the unsaved-changes count, and the unsaved-new sidebar badge all derive from this one predicate so they cannot drift.
_Avoid_: "saveable", "dirty" (a layout can be dirty without any persistable in-memory node — e.g. a metadata edit), "promotable" (used historically; superseded)

**effectiveNodeStore**:
The per-node layout facade (ADR-0011, sibling to `effectiveLayoutStore`). Projects `nodeTreeStore`, `nodeInfoStore`, `configReadNodesStore`, `partialCaptureNodesStore`, `layoutStore.activeContext`, and the edit-layer stores into `nodeOrigin`, `isFullyCaptured`, `isConfigRead`, `isPersistableInLayout`, `unsavedInMemoryNodeIds`, `unsavedRemovedNodeIds`, `isDirty`. Reads only — never writes through. Lives at `app/src/lib/layout/effectiveNodeStore.svelte.ts`, exposed via `$lib/layout`.
_Avoid_: "node store" (overloaded), "dirty store"

## Relationships

- A **Node** has exactly one **Node ID** (permanent) and one **Node Alias** (per session, dynamic)
- A **Node** may have zero or one **SNIP** data set (some nodes don't support SNIP)
- One **Event ID** maps to one **Bowtie**; a **Bowtie** defines producers + consumers for that Event ID
- A **Bowtie** has one or more **Connection Elements** as producers and one or more as consumers
- A **Layout** contains one **Node Tree** with one **Node Proxy** per discovered node
- A **Profile** is specific to one node model; it may contain **Relevance Rules** and **Cascade Rules**
- **Relevance Rules** govern CDI sections (many-to-many: one rule may affect multiple sections; one section may be governed by multiple rules)
- **Offline Changes** are a superset of **Pending Changes** (all pending changes are offline; not all offline changes are pending)
- A **Sync Session** classifies **Offline Changes** into actionable rows
- An **Information Channel** has exactly one **Binding** linking it to pin(s) on a subsystem on a node (replaces the pre-018 **Hardware Reference**)
- An **Information Channel** has exactly one **Role** (state vocabulary + slot-binding contract) and exactly one **Style** (hardware-shape realisation); every channel's Style realises its Role
- An **Information Channel** has exactly one **Channel Ownership** (`hardware-owned` or `user-owned`) deciding its lifecycle
- A **Layout** contains zero or more **Information Channels** (persisted in `channels.yaml`)
- A **Layout** contains zero or more **Facilities** (persisted in `facilities.yaml`)
- A **Facility** has exactly one **Behavior Template** and one **Facility Slot** per template-declared slot
- A **Facility Slot** is either empty or bound to exactly one **Channel** whose **Role** matches the slot's required role
- A **Channel** is bound to at most one **Facility Slot** at a time (Spec 018; ref-counting + fan-out are Future Considerations)
- A **Facility** has exactly one derived **Facility Status** (`Incomplete` or `Wired`) computed from its slot fullness; status is never persisted (ADR-0004)
- A **Style** has exactly one **Style Constraint Contract** governing its claimed pins' CDI fields

## Flagged Ambiguities

- **"Element"** is used for (a) Connection Element (bowtie producer/consumer), (b) CDI DataElement (leaf config field), and (c) abstract "node/element pair". Qualify on first use: "Connection Element" for (a), "Config Element" or "CDI Element" for (b).
- **"Display Name"** vs "friendly name": Display Name is canonical. Retire "friendly name" in code comments and docs.
- **"Pending" vs "Offline"**: "Offline Change" = stored on disk in offline-changes.yaml. "Pending Change" = subset ready to sync at connect. Offline ⊇ Pending.
- **"Config Read"** is vague — refers to (a) reading CDI from one node, (b) reading PIP flags, (c) entire multi-node session. Use "Config Read Session" for (c), "read CDI" for (a), "read PIP" for (b).
- **"Profile" scope**: "Structure Profile" = `.profile.yaml` shipped with app. "Extraction Profile" = full 7-file set from manual authoring. Ship only structure profiles.
- **"Cascade Rules"** are defined in specs but not yet fully implemented. Reserve term until implementation lands.
- **Node ID forms**: Both `05.02.01.02.FF.01` (dotted) and `050201020000FF` (canonical) exist. Display and store as dotted; normalize to canonical before comparison. See `product/architecture/naming-and-normalization.md`.
