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
An 8-byte (64-bit) unique identifier in dotted-hex format used by producers and consumers to communicate without direct knowledge of each other.
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

## Flagged Ambiguities

- **"Element"** is used for (a) Connection Element (bowtie producer/consumer), (b) CDI DataElement (leaf config field), and (c) abstract "node/element pair". Qualify on first use: "Connection Element" for (a), "Config Element" or "CDI Element" for (b).
- **"Display Name"** vs "friendly name": Display Name is canonical. Retire "friendly name" in code comments and docs.
- **"Pending" vs "Offline"**: "Offline Change" = stored on disk in offline-changes.yaml. "Pending Change" = subset ready to sync at connect. Offline ⊇ Pending.
- **"Config Read"** is vague — refers to (a) reading CDI from one node, (b) reading PIP flags, (c) entire multi-node session. Use "Config Read Session" for (c), "read CDI" for (a), "read PIP" for (b).
- **"Profile" scope**: "Structure Profile" = `.profile.yaml` shipped with app. "Extraction Profile" = full 7-file set from manual authoring. Ship only structure profiles.
- **"Cascade Rules"** are defined in specs but not yet fully implemented. Reserve term until implementation lands.
- **Node ID forms**: Both `05.02.01.02.FF.01` (dotted) and `050201020000FF` (canonical) exist. Display and store as dotted; normalize to canonical before comparison. See `product/architecture/naming-and-normalization.md`.
