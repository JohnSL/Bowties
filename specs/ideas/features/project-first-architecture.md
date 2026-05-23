# Layout-First Architecture — Exploration Record

- **Areas**: architecture, layout, connection, startup, save-flow
- **Origin**: spec 013 design exploration (save-flow-reorder → layout-first model)
- **Status**: adopted (spec 013 rewritten to implement this)
- **Date**: 2025-05-16

The current Bowties architecture treats connection and layout as independent axes, producing four states (connected/disconnected × layout/no-layout) with complex transitions. This causes multiple interrelated defects: blank bowties during save (stale catalog), cancel-corrupts-state, dynamic `isOfflineMode` flipping, and metadata edits with no durable home when connected without a layout. The layout-first model collapses this to two states (layout-offline, layout-online) by requiring a layout context for all work, storing connections as layout properties, and making online/offline phases of the layout rather than independent axes.

Terminology: "layout" was chosen over "project" because it maps naturally to model railroad users' mental model — they are working on a layout. The layout is persisted as a base file (`.layout`) plus a companion directory (`.layout.d/`) with per-node snapshot files, designed for git-friendly diffs. A layout picker (known-layout registry) abstracts the file+directory structure from the user.

## Prior Work

### Options explored

1. **Option C (scoped fix):** Rebuild catalog after bus writes for config-only saves; three-phase save flow for metadata saves. Fixes symptoms but leaves 4-state complexity and transition bugs.
2. **Option E (project-always, explicit):** iTrain-style — startup requires New/Open Project. Eliminates "connected without project" state entirely.
3. **Option F (auto-project):** Like E but with implicit scratch project creation on connect. Preserves low ceremony for quick browsing.
4. **Option G (incremental):** Add connection settings to layout file but keep 4-state model. Backward compatible but doesn't address root cause.

### Decision: Option E (project-always)

Rationale:
- Bowties' mission is layout-level configuration, not isolated node editing. A project context is natural.
- The "quick peek without a project" scenario is weak — two clicks ("New Project" → connect) vs one.
- Multi-connection per project enables the home/club workflow (spec 010) as a first-class feature.
- Save flow becomes simple and consistent: always save project file, optionally write to bus.
- Eliminates `isOfflineMode` dynamic flipping entirely.
- Few users now, so breaking change is low risk.

### iTrain comparison

iTrain uses a project-first model (must create/open project before any activity). Connection settings ("digital systems") are stored in the project. Multiple command stations per project are supported. The project is the single container for switchboard, locomotives, routes, signals, and connections. Bowties' "project" maps to this concept.

### JMRI comparison

JMRI requires a "profile" at startup (app-level config container), but profiles don't store connection settings — those are separate. JMRI's profile is more like "which set of preferences to use" than "which layout to work on." Bowties' project concept is closer to a JMRI panel file (the user's actual layout project).

### Multi-connection support

A project can define multiple named connections (e.g., "Home Workbench", "Club Layout"). This directly enables spec 010's workflow: configure new nodes at home on a test bus, then install them at the club and connect to the club's bus — all within the same project. The user selects which connection to activate. If only one connection exists, it's used directly without a selection step.

### Migration path

Existing `.bowties.yaml` layout files are migrated to project format on open: an empty `connections` section is added. All existing data (snapshots, bowties, offline changes, role classifications, connector selections) is preserved. One-time prompt on first open of a legacy file.
