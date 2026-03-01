# Quickstart: 007-edit-node-config

**Date**: 2026-02-28

## Overview

This feature adds inline editing of LCC node configuration values with visual dirty-tracking, validation, Save/Discard controls, and progress indication. It extends the existing read-only configuration view with write support through the Memory Configuration Protocol.

## Prerequisites

- Feature `004-read-node-config` must be complete (config values loaded)
- Feature `006-unified-node-tree` must be complete (tree UI with segments/leaves)
- Active TCP connection to an LCC network with at least one configurable node
- Rust stable toolchain (1.70+), Node.js, pnpm

## Development Setup

```bash
# From repo root
cd app
pnpm install              # Frontend dependencies
cd src-tauri
cargo build               # Backend + lcc-rs build

# Run in dev mode
cd ../..
cd app
pnpm tauri dev            # Launches app with hot-reload
```

## Testing

```bash
# Rust unit tests (lcc-rs write commands)
cd lcc-rs
cargo test

# Frontend tests (editable components, stores)
cd app
pnpm test
```

## Implementation Order

### Layer 1: Protocol (lcc-rs)
1. Add `build_write()` to `MemoryConfigCmd` — mirrors `build_read()` with write command bytes
2. Add `build_update_complete()` — 2-byte datagram `[0x20, 0xA8]`
3. Add `write_memory()` to `LccConnection` — send write + await ack + retry
4. Add `send_update_complete()` to `LccConnection`
5. Unit tests with `MockTransport`

### Layer 2: Backend (Tauri commands)
6. Add `write_config_value` command — bridges frontend to `write_memory()`
7. Add `send_update_complete` command
8. Add value serialization helper (TypeScript → bytes format)
9. Register new commands in `lib.rs`

### Layer 3: Frontend (Svelte)
10. Add `PendingEdit`, `WriteResult`, `SaveProgress` TypeScript types
11. Add `serializeConfigValue()` utility function
12. Create `PendingEditsStore` (Svelte 5 runes, class singleton)
13. Modify `TreeLeafRow.svelte` — conditional editable inputs by type
14. Create `SaveControls.svelte` — Save/Discard buttons + progress bar
15. Add Save/Discard toolbar to `SegmentView.svelte`
16. Add unsaved-change badges to `NodeEntry.svelte` and `SegmentEntry.svelte`
17. Add navigation guards to `+page.svelte`

### Layer 4: Integration
18. End-to-end test: edit field → save → verify write → update complete
19. Error handling test: simulate write failure → verify error indicators

## Key Files to Modify

| File | Change |
|------|--------|
| `lcc-rs/src/protocol/memory_config.rs` | Add `build_write()`, `build_update_complete()` |
| `lcc-rs/src/discovery.rs` | Add `write_memory()`, `send_update_complete()` |
| `app/src-tauri/src/commands/cdi.rs` | Add `write_config_value`, `send_update_complete` commands |
| `app/src-tauri/src/lib.rs` | Register new commands |
| `app/src/lib/types/nodeTree.ts` | Add `PendingEdit`, `WriteResult`, `SaveProgress` types |
| `app/src/lib/api/config.ts` | New file: `writeConfigValue()`, `sendUpdateComplete()` wrappers |
| `app/src/lib/stores/pendingEdits.svelte.ts` | New file: `PendingEditsStore` class |
| `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte` | Add editable inputs |
| `app/src/lib/components/ElementCardDeck/SaveControls.svelte` | New file: Save/Discard UI |
| `app/src/lib/components/ElementCardDeck/SegmentView.svelte` | Add SaveControls toolbar |
| `app/src/lib/components/ConfigSidebar/NodeEntry.svelte` | Add unsaved badge |
| `app/src/lib/components/ConfigSidebar/SegmentEntry.svelte` | Add unsaved badge |
| `app/src/routes/config/+page.svelte` | Add navigation guards |

## Verification

After implementation, verify these user scenarios:

1. **Edit & Save**: Change a string field value → see dirty indicator → click Save → see progress → field clears to clean
2. **Dropdown**: Change an integer field with map entries → dropdown shows labels → save writes numeric value
3. **Event ID**: Edit event ID in dotted-hex → validate format → save writes 8 bytes
4. **Validation**: Enter invalid value → field shows invalid state → Save button disabled
5. **Discard**: Edit multiple fields → click Discard → all revert to original values
6. **Sidebar badges**: Edit fields in a segment → node and segment entries show unsaved indicators
7. **Navigation guard**: Edit fields → try to switch nodes → confirmation dialog appears
8. **Write failure**: Disconnect mid-save → failed fields show error state → retry works
