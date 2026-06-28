# Quickstart — Block Indicator Facility (Spec 018)

Once spec 018 ships, here is the **end-to-end user journey** that exercises every architectural seam (channel, role, style, facility, slot, constraint contract, Wired transition, slot-detach). This is the demo script for SC-004 and the integration-test scenario the `facilityOrchestrator` is built against.

## Prerequisites

- A Bowties build that includes spec 018.
- A working layout open in offline mode (no hardware needed for steps 1-3).
- For step 4 (the physical demo), connect to a bus that includes:
  - One RR-CirKits TowerLCC node with a BOD-8 daughter board on one connector (any of the 8 inputs wired to a real block detector).
  - One RR-CirKits Signal LCC node with at least one Direct Lamp Control lamp row wired to a real LED.

## Step 1 — Scaffold a facility before any hardware is connected (~30 seconds)

> **Tab**: Railroad. **No hardware needed.** **Validates**: US1, FR-016.

1. Open the Railroad tab. Notice the new **Facilities** section. (No in-product "New" tag or banner — expectation-setting is in the release notes and user docs, per FR-034 / FR-035.)
2. Click **Add facility**. A dialog asks for a template (only **Block Indicator** is offered in this slice) and a name. Enter `Block 5` and confirm.
3. The facility appears in the Facilities section with status **Incomplete** and two empty slots labelled **Input** (requires `block-occupancy`) and **Output** (requires `lamp-indicator`). Each slot displays *empty* and shows a tooltip explaining its required role.
4. Save the layout (`File → Save` or Ctrl+S). Close and reopen the layout. **Expected**: the facility, its name, both empty slots, and Incomplete status round-trip exactly.

## Step 2 — Verify BOD hardware via the Channels panel (~30 seconds)

> **Tab**: Railroad. **Hardware needed**: TowerLCC + BOD-8 (live). **Validates**: US2, FR-007, FR-008, FR-031, SC-001, SC-011.

1. Connect to the bus. The TowerLCC discovers; its CDI tree loads.
2. In the Config tab, select **BOD-8** on a connector (e.g., Port A). Switch back to the Railroad tab.
3. The **Channels** panel now shows the TowerLCC with 8 new entries under *Port A — Inputs 1–8*. Each entry shows:
   - Ownership badge: **HW** (hardware-owned).
   - Role: `block-occupancy`.
   - Style: `bod-block-detector-input`.
   - Name: auto-generated default (e.g., `Tower-1 — Port A — Input 1`).
   - Live state: starts ○ (unknown), then resolves to ● (clear) or ● (occupied) as bus events arrive.
   - Binding column: **unbound**.
4. Physically trigger one BOD input (place a wheelset on the block). The corresponding row updates to **● occupied** in the live-state column within the existing event-store window. Remove the wheelset; it returns to **● clear**. Verification done — **no facility involved**.

## Step 3 — Bind the facility's input slot (~15 seconds)

> **Tab**: Railroad. **Validates**: US3 (producer half), FR-018 (Select channel), FR-019, SC-003.

1. Return to the **Block 5** facility from Step 1. Its Input slot is still empty.
2. Click **Select channel** on the Input slot. A picker opens listing the 8 unbound BOD channels from Step 2. Pick the one wired to your real block (e.g., `Tower-1 — Port A — Input 3`).
3. The Input slot fills with the channel's name and shows its live state (● clear).
4. In the Channels panel, that channel's binding column now reads **Block 5 → Input** instead of *unbound*.
5. The facility is still **Incomplete** (Output slot empty).

## Step 4 — Add the lamp channel and watch the facility go Wired (~30 seconds + end-to-end test)

> **Tab**: Railroad. **Hardware needed**: Signal LCC (live) with at least one Direct Lamp Control row not claimed by a mast. **Validates**: US3 (consumer + Wired transition), FR-010, FR-011, FR-018 (Add channel), FR-021, FR-023, SC-004.

1. Click **Add channel** on the **Output** slot. A sub-picker opens listing every unclaimed, constraint-compatible Direct Lamp Control row across connected Signal LCC nodes (rows whose `Lamp Selection` is **Used by Mast** are hidden, per FR-030).
2. Pick the row wired to your real LED (e.g., `Signal-1 — Direct Lamp Control — Row 7`).
3. In a single atomic action, Bowties:
   - Creates a user-owned channel with role `lamp-indicator`, style `single-led-direct-lamp`, bound to the chosen row.
   - Assigns it a default name (e.g., `Signal-1 — Direct Lamp Control — Row 7`). You can rename it inline now or later.
   - Binds it to the Output slot.
   - Marks the facility **Wired**.
   - Creates the underlying bowtie(s) for the Block Indicator pass-through mapping (`occupied → lit`, `clear → unlit`) via the **existing bowtie creation mechanism**.
4. Once the new bowtie edits propagate to the bus through the existing layered storage system, **occupying the physical block lights the LED; clearing the block turns it off**. End-to-end test passes.

## Step 5 — Verify the Wired persistence round-trips (~15 seconds)

> **Validates**: SC-010.

1. Save, close, and reopen the layout (still connected to the bus).
2. **Expected**: the facility reopens as **Wired**, both slots stay bound to the correct channels, the underlying bowtie(s) reload, the LED follows the block immediately on reconnect.

## Step 6 — Verify slot-detach and constraint release (~30 seconds)

> **Validates**: US3 cleanup paths, FR-022, FR-029, SC-005, SC-008.

1. Open the Output slot's menu and click **Remove from slot**. In a single atomic step:
   - The slot empties.
   - The user-owned lamp channel is deleted entirely.
   - The lamp row's `Lamp Selection` field becomes editable again in the Config tab (the style's constraint contract releases its lock).
   - The facility returns to **Incomplete**.
   - The facility's bowtie(s) are removed via the **existing slot-detach pipeline**.
   - The LED stops following the block (once the bowtie-delete change reaches the bus).
2. Re-add a lamp channel (Step 4 again) to confirm the facility returns to **Wired** without any explicit deploy action.

## Step 7 — Verify the hardware-clear cascade (~30 seconds)

> **Validates**: edge case "BOD daughter board cleared", FR-006, FR-022, SC-007.

1. With the facility **Wired**, go to the Config tab and clear the BOD-8 selection on the TowerLCC connector that owns your Input-slot channel.
2. **Expected**, in a single atomic step:
   - All 8 hardware-owned BOD channels for that connector disappear from the Channels panel.
   - The facility's Input slot becomes empty.
   - The facility returns to **Incomplete**.
   - The facility's bowtie(s) are removed (LED stops following the block once the bowtie-delete change reaches the bus).
3. Re-select BOD-8 on the same connector. 8 new hardware-owned channels appear with default names (the previously-renamed names are *not* restored — names are lost on hardware-config changes, per FR-006). The Input slot stays empty; bind it again to restore the facility to **Wired**.

## What this exercises

- Channel data model with `role` + `style` + `ownership` + discriminated `binding` (FR-001, FR-002, FR-006).
- Facility data model with optional slot bindings, status derived from slots (FR-003, FR-020).
- Block Indicator behavior template (FR-014, FR-015).
- Hardware-owned channel auto-create + cascade-delete (FR-007).
- User-owned channel atomic Add-channel flow (FR-018).
- Style constraint contract applied to the claimed row (FR-027, FR-029, FR-030).
- Wired transition creates bowtie(s) via the **existing** mechanism (FR-021, FR-023).
- Incomplete transition frees bowtie(s) via the **existing** slot-detach pipeline (FR-022).
- Save/reopen round-trip across mixed lifecycle states (FR-005, SC-010).
- Channels panel as a hardware-organised verification surface, functional with no facilities (FR-031, US2).
- Expectation-setting via release notes + user docs only — no in-product chrome (FR-034, FR-035).

## What this deliberately does **not** exercise

- Multi-style picker UX (only one style per role in this slice).
- Channel fan-out across multiple slots (FR-004 enforces one-slot-per-channel).
- Test-event injection on a consumer channel (deferred — Future Considerations).
- Placeholder-node-backed facilities (deferred — Future Considerations).
- Top-level tab chrome refresh (Slice 8, FR-036, fully isolated and optional).
