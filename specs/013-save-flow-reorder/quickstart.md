# Quickstart: Layout-First Model

## What Changes for Users

### Startup
Before: App opens to an empty state or auto-reopens the last layout. Connection dialog is independent.

After: App opens to a **layout picker** showing your known layouts. You must open or create a layout before doing anything else. The layout picker shows layout names, locations, and when each was last used.

### Connections
Before: Connection settings are global. Connecting is independent of having a layout open.

After: Connection definitions are **stored in the layout**. Each layout can have multiple named connections (e.g., "Home Workbench," "Club Layout"). You connect to a bus from within an open layout.

### Saving
Before: Save writes config to bus nodes first, then saves the layout file. This sometimes causes blank bowties.

After: Save **always writes the layout first**, then writes config to bus (if online), then updates the layout with results. Bowties never go blank during save. A progress dialog shows each phase.

### Event Roles
Before: Only user-override role classifications are saved. Protocol-resolved roles are lost when reopening offline.

After: All resolved event roles are **persisted in the layout**. Bowties display correct Producer/Consumer roles when reopening offline.

## New Workflows

### First Launch
1. App shows layout picker with empty list
2. Click "New Layout" → enter name → pick save location
3. Empty layout opens — ready to add connections and browse nodes

### Creating a Connection
1. Open a layout
2. Open connection settings (from the layout)
3. Add a named connection (e.g., "Home Workbench") with host/port
4. Connect — the app discovers nodes on the bus
5. Connection settings saved automatically in the layout

### Switching Connections
1. Disconnect from current connection
2. Pick a different connection from the layout's connection list
3. Connect — same layout, different bus
4. All node snapshots and bowties preserved across the switch

### Saving (Online)
1. Make configuration changes
2. Click Save
3. Progress: "Saving layout…" → "Writing configuration… 3 of 7" → "Updating layout…" → Done
4. If some writes fail: "2 changes could not be written — they'll be retried next time"
5. Bowties stay correct throughout

### Opening an Existing Layout
1. App shows layout picker
2. Click a layout from the known list — opens immediately
3. Or click "Browse…" to find a layout file not in the list

## Developer Guide

### Key Architecture Changes

| Before | After |
|--------|-------|
| 4 states (connected×layout) | 2 states (layout-offline, layout-online) |
| Connections global in `connections.json` | Connections in layout manifest |
| Save: bus first → layout second | Save: layout first → bus second → reconcile |
| Recent layout: single `recent-layout.json` | Known layouts: `known-layouts.json` array |
| Roles: only user overrides persisted | Roles: all resolved roles persisted |

### Testing the Save Flow

```bash
# Run all tests
cd app && npx vitest run

# Run Rust backend tests  
cd app/src-tauri && cargo test

# Run save-specific tests
cd app && npx vitest run -t "save"
cd app/src-tauri && cargo test save
```

### Migration
- Existing layouts (schema v3) are automatically migrated to v4 on open
- Migration adds an empty `connections` section
- No data loss — all existing fields preserved
- Users add connections manually after migration
