# Connect, Discover, Read, And Browse Configuration

## Purpose

This document captures the current user stories for Bowties' primary online workflow: connect to a bus, discover nodes, read configuration, and browse configuration values.

It is validated against current implementation in the frontend orchestration and store layers, current tests, and the implemented portions of specs 004, 005, and 006.

## Validation Sources

- `specs/004-read-node-config/spec.md`
- `specs/006-bowties-event-discovery/spec.md`
- `docs/technical/architecture.md`
- `app/src/lib/orchestration/discoveryOrchestrator.ts`
- `app/src/lib/orchestration/configReadOrchestrator.ts`
- `app/src/lib/orchestration/configReadSessionOrchestrator.ts`
- `app/src/lib/components/ConfigSidebar/**`
- `app/src/routes/+page.svelte`
- current route, orchestrator, component, and store tests covering discovery, config-read, sidebar, and route behavior

## Current User Story 1: Connect And Discover Nodes

A user connects Bowties to an LCC bus and sees the discovered nodes become available for inspection.

### Current Behavior

- The user connects through the main connection flow.
- Bowties probes for nodes and builds the discovered-node state from live bus responses.
- The node list shows discovered nodes with the best available display name and falls back when necessary.
- Discovery updates feed the main route and the node/state stores instead of requiring a separate manual reconciliation step.

### What The User Gets

- a visible set of discovered nodes
- progress and status feedback while discovery is active
- the ability to proceed into configuration and bowtie workflows once enough data is available

### Current Limits

- Discovery is only the first phase. Some follow-on actions, especially configuration and bowtie exploration, depend on later reads completing.

## Current User Story 2: Read Configuration With Progress

A user wants Bowties to read node configuration values and show progress while that work is happening.

### Current Behavior

- Bowties reads configuration through the config-read orchestration path.
- The user sees progress while configuration is being read.
- Nodes that are not eligible for configuration reading are handled through gating behavior rather than silently treated as normal readable nodes.
- Failures on one node do not prevent other eligible nodes from being processed.

### What The User Gets

- current configuration values loaded for supported nodes
- feedback about read progress and read eligibility
- a consistent route into browsing configuration once the read is complete

### Current Limits

- Current product behavior is centered on reading and browsing configuration. This document does not claim the entire online edit-and-save workflow as fully current product behavior.

## Current User Story 3: Browse Configuration And Navigate Between Related Views

A user wants to browse a node's configuration, inspect values, and move between configuration and connection-oriented views.

### Current Behavior

- The configuration UI uses a sidebar and card-deck style browsing model rather than the superseded Miller Columns view.
- Sidebar navigation exposes nodes and configuration areas using the current frontend ownership model.
- Configuration details display current values and related metadata.
- When an event slot participates in a discovered or current bowtie, the configuration view can show a related "Used in" connection reference.

### What The User Gets

- a stable configuration browsing workflow after reads complete
- a consistent mapping from discovered nodes to configuration sections
- visible cross-reference between configuration elements and the bowties view where supported

### Current Limits

- The bowties tab is not treated as ready until the relevant read/discovery work has completed.
- Some advanced configuration-editing workflows remain documented elsewhere as future or incomplete work and are not treated as fully current here.

## Supported Outcome

Today, a user can:

1. connect to a live bus
2. discover nodes
3. read supported configuration with progress feedback
4. browse current configuration values in the current configuration UI
5. move between configuration understanding and bowtie understanding where the product exposes those links

That is the durable current user story for Bowties' online read-and-browse flow.