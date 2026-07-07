# Research: ABS Signaling

**Feature Branch**: `020-abs-signaling` | **Date**: 2026-07-07

## Research Tasks

All NEEDS CLARIFICATION items were resolved during the `/clarify` session. This research consolidates the technical findings needed for Phase 1 design.

---

## R1: Tower LCC Conditional Line CDI Structure

**Decision**: Compilation targets the Tower LCC Conditionals segment (space 253, origin 2528) with 32 Logic groups.

**Structure per Logic group**:

| Field | Type | Key Values |
|-------|------|------------|
| Description | string (32 chars) | User label for the line |
| Function | int (1 byte) | 0=Blocked, 1=Group, 3=Last(Single) |
| Logic Operation | int (1 byte) | 0=AND, 1=OR, 6=null⇒true, 7=V1-Only, 8=V2-Only |
| Variable #1/Trigger | int | 0=On Variable Change, 1=On Matching Event, 2=None |
| Variable #1/Source | int | 0=Use Variable's Events, 1–8=Track Circuit 1–8 |
| Variable #1/Track Speed | int | 0=Stop … 5=Approach … 7=Clear/Proceed |
| Variable #1/set true | eventid | Consumer event to set variable true |
| Variable #1/set false | eventid | Consumer event to set variable false |
| Variable #2 | (same structure as #1) | |
| Action/when true | int | 0=Send then Exit, 2=Send then Evaluate Next, 3=Exit Group, 4=Evaluate Next |
| Action/when false | int | (same options) |
| Action Events (×4) | | |
| Action Event/Condition | int | 0=none, 1=Immediately, 2=After delay, 3=Immediate if True, 4=Immediate if False |
| Action Event/Destination | int | 0=Use Action Event, 1–8=Track Circuit 1–8 |
| Action Event/Track Speed | int | 0=Stop … 7=Clear/Proceed |
| Action Event/Action Event | eventid | Producer event sent when triggered |

**Mast Group semantics**: Consecutive lines with Function=Group (1), last line with Function=Last (3). Evaluation is most-restrictive-first with fall-through: if a line's condition is true, its actions fire and the group exits; if false, evaluation falls through to the next line.

**Rationale**: This structure directly maps to ABS signal rules. Each conditional line tests one condition (occupied? downstream Stop?) and produces aspect events. The mast group structure guarantees most-restrictive-first evaluation order.

**Alternatives considered**: STL (Signal Template Logic) is a more powerful programmable model available on Tower LCC+Q firmware, but requires firmware v1.09+ and is more complex than needed for basic ABS. Conditional lines are available on all Tower LCC firmware versions and are sufficient for 3-aspect ABS.

---

## R2: ABS 3-Aspect Signal Compilation Model

**Decision**: A 3-aspect ABS signal compiles to a mast group of 3 contiguous conditional lines, ordered most-restrictive-first: Stop → Approach → Clear.

**Compiled structure**:

```
Line N:   Stop rule
  Function: Group (1)
  Logic Operation: V1-Only (7)
  Variable #1/Source: Use Variable's Events (0)
  Variable #1/set true: [block-occupied event from BOD channel]
  Variable #1/set false: [block-clear event from BOD channel]
  Action/when true: Send then Exit (0)
  Action Events[0]: Condition=Immediate if True (3), Event=[red LED on]
  Action Events[1]: Condition=Immediate if True (3), Event=[green LED off]
  Action/when false: Evaluate Next (4)

Line N+1: Approach rule (omitted for end-of-line signals)
  Function: Group (1)
  Logic Operation: V1-Only (7)
  Variable #1/Source: Track Circuit K (1–8)
  Variable #1/Track Speed: Stop (0)  — true when downstream reports Stop
  Action/when true: Send then Exit (0)
  Action Events[0]: Condition=Immediate if True (3), Event=[red LED on]
  Action Events[1]: Condition=Immediate if True (3), Event=[green LED on]  — both on = yellow
  Action/when false: Evaluate Next (4)

Line N+2: Clear rule (default)
  Function: Last (3)
  Logic Operation: null⇒true (6)
  Action/when true: Send then Exit (0)
  Action Events[0]: Condition=Immediate if True (3), Event=[red LED off]
  Action Events[1]: Condition=Immediate if True (3), Event=[green LED on]
```

**For end-of-line signals** (no downstream): the Approach line is omitted, producing a 2-line mast group (Stop + Clear).

**Rationale**: This matches the standard Tower LCC conditional line pattern for ABS signals. The fall-through evaluation ensures Stop takes priority over Approach, which takes priority over Clear.

**Alternatives considered**: Using Logic Operation AND/OR with both variables could combine Stop+Approach into fewer lines, but would lose the clear separation of rules and make the compiled output harder to audit.

---

## R3: Track Circuit Cascade Mechanism

**Decision**: Same-node cascade uses Track Circuit internal channels. The downstream signal's mast group publishes its aspect to a Track Circuit via Action Event Destination. The upstream signal's Approach rule reads that Track Circuit via Variable #1/Source.

**Wiring pattern**:

1. **Downstream signal** (Signal B): Add an Action Event to the Clear rule with `Destination=Track Circuit K`, `Track Speed=Clear/Proceed (7)`. Add an Action Event to the Stop rule with `Destination=Track Circuit K`, `Track Speed=Stop (0)`.
2. **Upstream signal** (Signal A): Set `Variable #1/Source=Track Circuit K` and `Track Speed=Stop (0)` on the Approach line. This makes Variable #1 true when Track Circuit K reports Stop, triggering the Approach aspect.

**Track Circuit allocation**: Each cascade link consumes one Track Circuit (1–8) on the node. Allocation is per-facility-pair, tracked in the Logic Allocation Record.

**Rationale**: Track Circuits are the hardware-native cascade mechanism on Tower LCC. They carry speed/aspect information internally without external event bus traffic. The 8-circuit limit is sufficient for typical layouts (4 signals = 4 circuits).

**Alternatives considered**: External event-based cascade (one signal produces events, another consumes them) would work but adds bus traffic and doesn't use the hardware's native cascade support. Cross-node cascade via Track Transmitter/Receiver is deferred to a future spec.

---

## R4: Signal-Aspect Style: 2-LED Bicolor

**Decision**: The `2-led-bicolor-aspect` style claims 2 Direct Lamp Control rows (red LED + green LED) and maps aspects to LED on/off combinations.

**Aspect-to-event map**:

| Aspect | Red LED | Green LED | Visual |
|--------|---------|-----------|--------|
| Stop | On | Off | Red |
| Approach | On | On | Yellow (bicolor mixing) |
| Clear | Off | On | Green |
| Unknown | Off | Off | Dark |

**Style declaration** (extends `channelStyles.ts` pattern):

```
'2-led-bicolor-aspect': {
  pinCount: 2,
  pinLabels: ['Red', 'Green'],
  aspectMap: {
    stop:     [{ pin: 0, state: 'on' },  { pin: 1, state: 'off' }],
    approach: [{ pin: 0, state: 'on' },  { pin: 1, state: 'on' }],
    clear:    [{ pin: 0, state: 'off' }, { pin: 1, state: 'on' }],
    unknown:  [{ pin: 0, state: 'off' }, { pin: 1, state: 'off' }],
  }
}
```

Each pin maps to one Direct Lamp Control row. The `on`/`off` state maps to the row's Lamp On / Lamp Off consumer event IDs.

**Rationale**: 2-LED bicolor is the most common signal head type for model railroad ABS. Each aspect requires exactly 2 actions (one per LED), well within the 4-action-per-line limit.

**Alternatives considered**: Tricolor (3 LEDs: red, yellow, green) uses 3 actions per aspect but doesn't need bicolor mixing. Firmware mast-driven mode (Signal LCC Rule-to-Aspect) is more powerful but requires Signal LCC firmware configuration. Both are deferred.

---

## R5: Behavior Template Extension for ABS

**Decision**: Extend the existing `BehaviorTemplate` structure with a `compilation_target` field and an `inputs` model that distinguishes channel-bound inputs from cascade inputs.

The existing Block Indicator template uses simple `producer_state → consumer_command` mappings. ABS requires:
- **Condition-action rules** (not just state mappings): if condition X then produce aspect Y
- **Multiple input sources**: block occupancy (event-driven) + downstream signal (Track Circuit-driven)
- **Ordered evaluation**: most-restrictive-first
- **Target-specific compilation**: the same abstract rules compile differently for Tower LCC vs. STL (future)

**New template fields** (extend `BehaviorTemplate`):

```
rules: [
  { condition: { input: "block", state: "occupied" }, aspect: "stop", priority: 1 },
  { condition: { input: "downstream", state: "stop" }, aspect: "approach", priority: 2 },
  { condition: "default", aspect: "clear", priority: 3 },
]
compilation_target: "tower-lcc-conditional"  // Adapter selection
```

**Rationale**: Separating abstract rules from compilation target allows the same ABS template to target different hardware in the future (STL, LogixNG). The priority ordering maps directly to mast group evaluation order.

**Alternatives considered**: Encoding Tower LCC-specific CDI paths directly in the template would be simpler but couples templates to one hardware target. The adapter pattern enables future extensibility without changing template definitions.

---

## R6: Firmware Configuration Modes

**Decision**: Tower LCC has two firmware revisions with different conditional line capabilities, detected via CDI signature (profile configuration mode `firmware-revision`).

| Mode | Output Function Values | Input Function Values |
|------|----------------------|---------------------|
| `tower-lcc-legacy` | 17 enum values | 9 enum values |
| `tower-lcc-c7` | 5 enum values | 3 enum values |

For ABS conditional line compilation, the relevant Output Function values are consistent across both: "Steady Active Hi" (for signal aspects) is available in both firmware revisions.

**Rationale**: The compiler must produce CDI values that are valid for the detected firmware revision. The profile's configuration mode mechanism already handles this — the compiler should query the active firmware mode when selecting enum values.

**Alternatives considered**: Ignoring firmware differences would risk writing invalid CDI values. The profile configuration mode system already solves this problem.

---

## R7: Logic Allocation Persistence

**Decision**: Logic allocation records are persisted as part of the layout data (backend-owned, ADR-0002/0015). Each record tracks which conditional lines and Track Circuits a facility has claimed on a specific node.

**Persistence model** (new section in layout YAML or separate file):

```yaml
logicAllocations:
  - facilityId: "uuid"
    nodeKey: "05.02.01.02.61.00"
    conditionalLines: [0, 1, 2]          # 0-indexed line indices
    trackCircuits: [1]                    # 1-indexed circuit numbers
```

**Rationale**: Allocation records must survive app restart for capacity reporting and cleanup. They follow the same backend-owned pattern as facilities and channels (ADR-0002/0015). Storing as part of layout data keeps all facility-related state together.

**Alternatives considered**: Deriving allocations from CDI reads at startup would avoid persistence but is slow (requires reading all conditional lines from all Tower LCC nodes) and fragile (can't detect Bowties-managed lines vs. manually configured ones).
