# Logic & Automation Builder for Board-Specific Configuration

> **Status: Future design input — not yet implemented.** This describes an aspirational logic/automation layer beyond the current bowtie connection model. Do not treat this as current product architecture. Revisit when the core bowties model is stable and this becomes a planned feature.

**What:** A high-level rule-based interface that lets users configure layout behavior using state-driven logic ("WHEN turnout closed THEN signal red") instead of low-level Event ID mapping. The system understands board-specific semantics through extensible device profiles and compiles rules to appropriate outputs - event mappings for simple nodes, structured logic for sophisticated nodes like Tower LCC +Q.

**Who:** Hobbyists who think in railroad operations terms, not protocol engineers. Solves the frustrated Signal-32 user's problem: "I just want to control signals based on turnout position" without needing to understand Event IDs.

**Why this approach:** Bridges the gap between how users think (state/conditions) and how LCC works (events), while preserving full access to underlying event model for advanced users. Complements Bowties' visualization strength with authoring capability.

## User Scenarios

### Scenario 1: Merge Signals (The Original Problem)

**When** two tracks merge to one and I want signals to show proper aspects based on turnout positions, **I want to** visually configure the signal logic, **so I can** safely operate my layout without understanding Event IDs.

**User flow:**
1. User opens "Logic & Rules" view (new tab alongside Configuration, Bowties, Monitor)
2. Clicks "+ New Rule" or uses template: "Merge Point Signaling"
3. Template prompts: "Select your turnout feedback:" → dropdown shows BOD4-CP "Turnout 1" (auto-discovered, board profile knows it provides turnout state)
4. "Select signals to control:" → dropdown shows Signal-32 "Signal Head 1-4" (board profile knows these are G-type Atlas signals)
5. Visual rule builder appears:
   ```
   WHEN Turnout 1 is Closed (Normal)
   THEN Signal 1 shows Green
   AND  Signal 2 shows Red
   
   WHEN Turnout 1 is Thrown (Diverging)
   THEN Signal 1 shows Red
   AND  Signal 2 shows Green
   ```
6. User clicks "Apply Logic"
7. System compiles:
   - Identifies BOD4-CP Event IDs for "Turnout 1 Closed" and "Turnout 1 Thrown"
   - Maps to Signal-32 consumer slots for red/green aspects
   - Writes Event IDs to Signal-32 configuration
8. User presses physical turnout → signals change appropriately
9. No Event IDs seen, no manual event mapping needed

### Scenario 2: Tower LCC +Q Advanced Logic

**When** I have a Tower LCC +Q node with STL capability and want complex interlocking logic, **I want to** use the same rule interface but compile to onboard logic, **so I can** have sophisticated behavior without requiring the app to run.

**User flow:**
1. Creates rule: "IF (Block 1 Occupied AND Turnout 3 Closed) OR Emergency Button THEN Lock Turnout 5"
2. System detects target is Tower LCC +Q (board profile indicates STL support)
3. Instead of simple event mapping, compiles to STL program
4. Uploads STL bytecode to Tower node's logic engine  
5. Logic runs on hardware, survives app shutdown

### Scenario 3: Mixed Sophistication Network

**When** I have both simple DIY I/O nodes and sophisticated Tower nodes, **I want to** create rules that span both, **so I can** leverage each board's strengths without thinking about implementation differences.

**User flow:**
1. Rule: "WHEN DIY-Node-1 Button 3 pressed THEN Tower-1 Timer 1 starts AND Signal-32 Signal 2 shows Red"
2. System compiles:
   - **For DIY-Node-1:** Already configured as producer (just validates Event ID exists)
   - **For Tower-1:** Configures Timer 1 trigger to respond to Button 3's Event ID
   - **For Signal-32:** Configures Signal 2 red aspect to respond to Button 3's Event ID
3. Single rule, multiple compilation targets

## User Experience Design

### Information Architecture

Four-view application (adding to existing three):
1. **Configuration** (existing): Low-level element configuration, Miller Columns
2. **Event Bowties** (existing): Visualization of event relationships
3. **Logic & Rules** (NEW): State-driven rule authoring
4. **Event Monitor** (existing): Real-time diagnostics

### Logic & Rules View Layout

```
+-------------------------------------------------------------------+
| Logic & Rules         [+ Rule] [Templates] [Filter]               |
+-------------------------------------------------------------------+
|                                                                   |
| Active Rules (5)                           [Compiled OK]          |
|                                                                   |
| +------------------------------------------------------------+    |
| | * Merge Point Signals                   [Edit] [...]       |    |
| | +------------------------------------------------------+   |    |
| | | WHEN                                                 |   |    |
| | |   Turnout 1 (BOD4-CP Port 1) is Closed               |   |    |
| | | THEN                                                 |   |    |
| | |   Signal 1 (Signal-32 Head 1) -> Green               |   |    |
| | |   Signal 2 (Signal-32 Head 2) -> Red                 |   |    |
| | +------------------------------------------------------+   |    |
| | Status: [OK] Compiled to event mappings                    |    |
| | Target: Signal-32 (4 event mappings written)               |    |
| | Last triggered: 2 min ago                                  |    |
| +------------------------------------------------------------+    |
|                                                                   |
| +------------------------------------------------------------+    |
| | * Turnout Interlock                     [Edit] [...]       |    |
| | +------------------------------------------------------+   |    |
| | | WHEN                                                 |   |    |
| | |   Block 3 (Occupancy Detector) is Occupied           |   |    |
| | | THEN                                                 |   |    |
| | |   Turnout 5 (TC64-1 Output 3) -> Lock                |   |    |
| | +------------------------------------------------------+   |    |
| | Status: [OK] Compiled to STL on Tower-1                    |    |
| | Logic executes on: Tower LCC +Q (onboard)                  |    |
| | Last triggered: Never                                      |    |
| +------------------------------------------------------------+    |
|                                                                   |
| Disabled Rules (2)  [Show v]                                      |
|                                                                   |
+-------------------------------------------------------------------+
```

### Rule Editor Interface

When clicking "+ Rule" or "Edit":

```
+-------------------------------------------------------------------+
| Edit Rule: Merge Point Signals                   [Save] [Test]    |
+-------------------------------------------------------------------+
|                                                                   |
| Rule Name: [Merge Point Signals                             ]     |
| Description: [Controls signal aspects at main/siding merge  ]     |
|                                                                   |
| +--- Conditions ------------------------------------------------+ |
| | [+ Add Condition]                                             | |
| |                                                               | |
| | WHEN [Turnout 1 v] [is v] [Closed v]                          | |
| |      +- BOD4-CP / Port 1                                      | |
| |         State options: Closed, Thrown                         | |
| |                                                               | |
| | [AND/OR v] [+ Another Condition]                              | |
| +---------------------------------------------------------------+ |
|                                                                   |
| +--- Actions ---------------------------------------------------+ |
| | [+ Add Action]                                                | |
| |                                                               | |
| | THEN [Signal 1 v] [shows v] [Green v]                         | |
| |      +- Signal-32 / Head 1                                    | |
| |         Aspect options: Red, Yellow, Green, Dark              | |
| |                                                               | |
| | AND  [Signal 2 v] [shows v] [Red v]                           | |
| |      +- Signal-32 / Head 2                                    | |
| |                                                               | |
| | [+ Another Action]                                            | |
| +---------------------------------------------------------------+ |
|                                                                   |
| +--- Compilation -----------------------------------------------+ |
| | Target: Signal-32                                             | |
| | Method: Event mappings (node has no logic engine)             | |
| |                                                               | |
| | Preview:                                                      | |
| | * BOD4-CP Port 1 "Closed" (Event 05.02.01...)                 | |
| |   -> Signal-32 Head 1 Green (Consumer Event 3)                | |
| |   -> Signal-32 Head 2 Red (Consumer Event 1)                  | |
| |                                                               | |
| | [View as Bowtie Diagram]                                      | |
| +----------------------------------------------------------=----+ |
|                                                                   |
+-------------------------------------------------------------------+
```

### Board Profile Dropdown Intelligence

When user clicks condition/action dropdown:

```
Select Entity:

[By Board Type]
  +- BOD4-CP "East Panel"
  |   +- Port 1 (Turnout Feedback)     States: Closed, Thrown
  |   +- Port 2 (Occupancy)            States: Occupied, Clear
  |   +- Port 3 (Turnout Feedback)     States: Closed, Thrown
  |
  +- Signal-32 "Main Signals"
  |   +- Head 1 "Merge Approach"       Aspects: Red, Yellow, Green, Dark
  |   +- Head 2 "Siding Approach"      Aspects: Red, Yellow, Green, Dark
  |   +- Head 3 "Exit Signal"          Aspects: Red, Yellow, Green, Dark
  |
  +- Tower LCC +Q "Logic Controller"
      +- Timer 1                        Actions: Start, Stop, Reset
      +- Counter 1                      States: <threshold, >=threshold
      +- Output 1-8                     Actions: On, Off, Toggle

[By Function]
  +- Inputs (10)
  +- Outputs (24)
  +- Signals (8)
  +- Turnouts (12)
  +- Logic Elements (4)

Search: [type to filter...]
```

**Board profiles provide:**
- Semantic entity names (not just "Line 3")
- Valid states for conditions (Closed/Thrown, Occupied/Clear)
- Valid actions for outputs (Red/Yellow/Green for signals)
- Data type awareness (numeric thresholds for timers, boolean for switches)

### Template Library

Clicking [Templates]:

```
+---------------------------------------------------------------+
| Rule Templates                              [Close]           |
+---------------------------------------------------------------+
|                                                               |
| [Signaling]                                                   |
|   * Merge Point (2-track to 1)           <Selected>           |
|   * Interlocking (crossing protection)                        |
|   * Absolute Permissive Block (APB)                           |
|   * Approach lighting (signal on when train near)             |
|                                                               |
| [Turnout Control]                                             |
|   * Occupancy interlock                                       |
|   * Route selection (set multiple turnouts)                   |
|   * Frog polarity control                                     |
|                                                               |
| [Timing & Sequences]                                          |
|   * Delayed action                                            |
|   * Crossing flasher sequence                                 |
|   * Auto-reversing                                            |
|                                                               |
| [Scenes & Automation]                                         |
|   * One-button route                                          |
|   * Lighting scenes                                           |
|   * Startup/shutdown sequence                                 |
|                                                               |
+---------------------------------------------------------------+
```

### Interaction Patterns

1. **Creating Rule from Template:**
   - Select template → Pre-filled condition/action structure appears
   - Dropdowns show "Select..." placeholders
   - User fills in their specific boards/elements
   - System validates compatibility (e.g., can't set signal aspect on a turnout)

2. **Creating Rule from Scratch:**
   - [+ Rule] → Empty rule editor
   - [+ Add Condition] → Dropdown of all available inputs/states
   - [+ Add Action] → Dropdown of all available outputs/commands
   - AND/OR logic for multiple conditions
   - Multiple actions fire simultaneously

3. **Rule-to-Event Bidirectional Sync:**
   - Rule changes → Immediately updates Event ID mappings in node config
   - Event ID manually changed in Configuration view → Rule shows ⚠️ "Out of sync"
   - Click "Resync" → Options: "Update rule from events" or "Recompile rule to events"
   - Bowties view shows both: Event relationships + "Part of Logic Rule: [name]" annotation

4. **Testing:**
   - [Test] button → Opens Event Monitor filtered to rule's entities
   - Manually trigger condition (or press physical button)
   - Monitor shows: "Rule 'Merge Point Signals' fired → Actions executed"
   - Visual feedback in rule list (flashes when triggered)

5. **Navigation Between Views:**
   - Click entity in rule → "Edit in Configuration" link → Jump to Miller Columns
   - Click rule → "View as Bowtie" → Switch to Bowties view, highlight connections
   - In Bowties: Connections show badge "🧠 Logic Rule" if managed by rule
   - In Configuration: Elements show "📌 Used in 2 rules" indicator

### States to Design

**Loading State:**
- "Discovering boards..." spinner
- "Loading board profiles..." (Signal-32, BOD4-CP, Tower detected)
- If board unknown: "This node type doesn't have a profile yet. [Use generic mode] [Request profile]"

**Empty State:**
- "No rules configured yet"
- Prominent [+ Create Your First Rule] button
- "Or start with a template" → Shows 3-4 common templates inline

**Error States:**
- Rule compilation failed: "Can't map condition to event - BOD4-CP not configured"
- Target node offline: "Can't apply rule - Signal-32 not responding"
- Logic too complex: "This rule requires a logic-capable node - Tower LCC +Q recommended"
- Conflicting rules: "Warning: This overlaps with 'Interlocking Rule 2'"

**Success States:**
- "Rule compiled and applied ✅"
- Show affected nodes: "4 event mappings written to Signal-32"
- "Test your rule: [Open Monitor]"

### Accessibility Considerations

- Keyboard navigation through condition/action builder
- Screen reader announces: "Condition 1: When Turnout 1 is Closed"
- Dropdowns with type-ahead search
- High contrast for rule status indicators
- Clear focus indicators for form fields
- Undo/redo for rule editing

## Feature Breakdown

**F-Logic-1: Board Profile System** (Priority: P1 - Foundation)
Extensible plugin architecture for board-specific knowledge. Profiles define entity types, valid states/actions, compilation targets.

**F-Logic-2: Rule Data Model** (Priority: P1 - Foundation)  
Internal representation of conditions (entity, operator, value), actions (entity, command, parameters), AND/OR logic, rule metadata (name, description, enabled state).

**F-Logic-3: Basic Rule Editor UI** (Priority: P1 - MVP)
Visual condition/action builder with dropdowns, AND logic, single-target compilation. No templates yet, simple event-mapping compilation only.

**F-Logic-4: Event-Mapping Compiler** (Priority: P1 - MVP)
Translates rules to Event ID assignments for simple boards. Validates target nodes, writes configuration, handles errors.

**F-Logic-5: RR-Cirkits Board Profiles** (Priority: P1 - MVP)
Profiles for Signal-32 (signal aspects), BOD4-CP (turnout/occupancy feedback), TC64 (turnout control). Solves the original use case.

**F-Logic-6: Bidirectional Sync** (Priority: P2)
Detect manual Event ID changes, offer resync options, maintain rule-event linkage.

**F-Logic-7: Template Library** (Priority: P2)
Pre-built common patterns (merge signals, interlocking, timing). User can save custom templates.

**F-Logic-8: Tower LCC +Q STL Compiler** (Priority: P2)
Compile complex rules to STL bytecode for onboard execution. Support timers, counters, conditionals.

**F-Logic-9: OR Logic & Complex Conditions** (Priority: P2)
Multi-condition rules with AND/OR grouping, nested logic, numeric comparisons (e.g., "Counter > 5").

**F-Logic-10: Rule Testing & Simulation** (Priority: P2)
Integrated with Event Monitor, manual trigger, visual feedback when rules fire.

**F-Logic-11: Cross-View Integration** (Priority: P2)
Deep linking between Logic, Bowties, Configuration views. Show rule annotations in other views.

**F-Logic-12: Profile Editor** (Priority: P3)
Power users can create custom board profiles without code. JSON-based profile format.

**F-Logic-13: Rule Import/Export** (Priority: P3)
Share rules as files, community library, version control.

## Technical Feasibility

**Based on research:** Bowties already has the foundation for this:
- CDI parsing system can be extended with semantic annotations  
- Event ID read/write mechanisms exist
- Tauri IPC can handle rule compilation commands

### Implementation approach:

**1. Board profiles as JSON manifests:**
```json
{
  "board": "RR-Cirkits Signal-32",
  "version": "1.0",
  "entities": [
    {
      "id": "signal_head",
      "cdi_path": "Outputs/Heads/Head",
      "type": "signal",
      "states": ["red", "yellow", "green", "dark"],
      "event_mapping": {
        "red": "Event1",
        "yellow": "Event2",
        "green": "Event3",
        "dark": "Event4"
      }
    }
  ]
}
```

**2. Rule compiler in Rust:**
- Parse rule AST (conditions, actions)
- Look up entities in board profiles
- Resolve Event IDs from CDI/config-values
- Generate event mappings or STL code
- Write back to nodes via datagram protocol

**3. Bidirectional sync strategy:**
- Rules stored in SQLite with hash of generated Event ID mappings
- On config read: Compare hash, detect drift
- Offer reconciliation UI

**4. STL compilation:** (P2 feature)
- Learn Tower LCC +Q STL instruction set
- Generate bytecode from rule AST
- Upload via memory write commands

**Complexity:** Medium. Core event-mapping is straightforward. STL compilation adds complexity. Profile system is key extensibility point.

## Open Questions

1. **Rule Naming:** Auto-generate from template ("Merge Point Signals 1") or force user input?

2. **Multi-State Signals:** How to handle 2-head vs. 3-head signals with different aspect combinations?

3. **Timing Precision:** For delay actions, should we require a Timer node or support app-based delays?

4. **Rule Conflicts:** How aggressive should conflict detection be? Warn or prevent?

5. **Profile Distribution:** Ship profiles with app, or download from repo on demand?

6. **Generic Fallback:** If board has no profile, offer basic "When Event X Then Event Y" mode?

7. **Tower +Q STL Scope:** How much of STL to support initially? Full language or subset?

## Summary

**This design solves the Signal-32 user's problem directly:** They select their turnout feedback, select their signals, specify desired aspects, and click Apply - never seeing an Event ID. The system handles the gnarly details of copying the right Event IDs to the right consumer slots.

**It also sets up** the extensibility framework for Tower LCC +Q and future sophisticated boards, while maintaining compatibility with simple DIY boards.

**Key Innovation:** Bridges state-driven thinking (how hobbyists conceptualize their layout) with event-driven protocol (how LCC actually works) through board-aware semantic profiles and intelligent compilation.
