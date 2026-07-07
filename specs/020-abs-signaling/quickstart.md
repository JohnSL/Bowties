# Quickstart: ABS Signaling

**Feature Branch**: `020-abs-signaling` | **Date**: 2026-07-07

## Prerequisites

- Bowties running with a layout open
- At least one Tower LCC node connected (logic target)
- At least one Signal LCC node connected (signal head output)
- At least one BOD-8 block occupancy channel configured

## Create a Standalone ABS 3-Aspect Signal

1. Navigate to the **Railroad** tab
2. Click **Add Facility**
3. Select **ABS 3-Aspect Signal** template
4. Name the facility (e.g., "Signal B1")
5. In the **Block** slot, select an existing block-occupancy channel
6. In the **Signal Head** slot, add a new signal-aspect channel:
   - Select style: **2-LED bicolor**
   - Bind to 2 unclaimed Direct Lamp Control rows on a Signal LCC node (Red + Green)
7. Leave the **Downstream Signal** slot empty (end-of-line signal)
8. Select a **Logic Target** (Tower LCC node) — Bowties shows available capacity
9. **Apply** — Bowties compiles the signal rules and stages CDI changes
10. **Save** — CDI writes are sent to the Tower LCC node via the normal sync flow

**Result**: The signal shows Stop (red) when the block is occupied, Clear (green) otherwise.

## Build a 3-Signal ABS Cascade

After creating Signal B1 (end-of-line):

1. **Add Facility** → ABS 3-Aspect Signal → name "Signal B2"
2. Bind **Block** to a second block-occupancy channel
3. Bind **Signal Head** to a new bicolor channel on the Signal LCC
4. Bind **Downstream Signal** to Signal B1's signal-aspect output channel
5. Apply to the same Tower LCC node → Bowties allocates a Track Circuit for the cascade

Repeat for Signal B3, binding its downstream to Signal B2's output.

**Result**: Occupying a block shows Stop on that signal, Approach on the one behind it, and Clear further back.

## Delete a Signal Facility

1. Click the facility card → **Delete**
2. If other facilities reference this signal as downstream, Bowties warns about broken cascade references
3. Confirm deletion
4. Bowties resets the conditional lines to disabled and frees Track Circuits

## View Logic Target Capacity

1. When applying a facility, the **Logic Target** selector shows per-node capacity:
   - Conditional lines: used / total (e.g., 6 / 32)
   - Track Circuits: used / total (e.g., 2 / 8)
   - Per-facility allocation breakdown
2. Bowties suggests the node hosting the most input channels as the default target

## Key Concepts

| Concept | Description |
|---------|-------------|
| **Behavior Template** | Abstract signal rules — what conditions produce what aspects |
| **Signal-Aspect Channel** | Identity layer for a signal head — carries aspect vocabulary |
| **Signal-Aspect Style** | How aspects map to physical LEDs (e.g., bicolor: red+green) |
| **Logic Adapter** | Compiler from abstract rules to Tower LCC conditional lines |
| **Mast Group** | Contiguous conditional lines evaluated most-restrictive-first |
| **Track Circuit** | Internal cascade channel (1–8 per Tower LCC node) |
| **Logic Allocation** | Record of which lines/circuits Bowties claimed on each node |

## Development: Running Tests

```bash
# Backend (bowties-core)
cd bowties-core
cargo test logic_adapter

# Frontend
cd app
npx vitest run src/lib/stores/facilitiesStore
npx vitest run src/lib/orchestration/facilityOrchestrator
npx vitest run src/lib/utils/channelStyles
```

## Development: Key Files

| Layer | File | Purpose |
|-------|------|---------|
| Domain | `bowties-core/src/logic_adapter/compiler.rs` | Template → conditional line compilation |
| Domain | `bowties-core/src/logic_adapter/allocation.rs` | Resource allocation + capacity tracking |
| Domain | `bowties-core/src/behavior_templates.rs` | Template registry (extend) |
| IPC | `app/src-tauri/src/commands/facilities.rs` | Apply/delete with compilation (extend) |
| Store | `app/src/lib/stores/facilitiesStore.svelte.ts` | Allocation state (extend) |
| Facade | `app/src/lib/stores/effectiveLayoutStore.svelte.ts` | Capacity queries (extend) |
| Orchestrator | `app/src/lib/orchestration/facilityOrchestrator.ts` | Apply workflow (extend) |
| Component | `app/src/lib/components/Facilities/LogicTargetSelector.svelte` | Target selection UI (new) |
| Util | `app/src/lib/utils/channelStyles.ts` | Signal-aspect style registry (extend) |
