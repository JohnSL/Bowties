# Signaling UX — Proposal

Status: **draft**. Companion to [signaling-ux-mockups.html](signaling-ux-mockups.html) and the
[README](README.md).

---

## 1. Signal type — the core data object

A **signal type** is the reusable definition of a physical signal: what it looks like, and what it can
express. Twenty identical signals on a layout share one signal type. Shipped catalog entries are
ordinary signal types with no special status.

A signal type contains:

1. **Heads** — how many, and how many LED drivers each head uses.
2. **Indications** — every indication the signal can physically display, with:
   - The LED state per head (`show`)
   - The speed this signal authorises (`speed` — "permits past")
   - The speed expected at the next signal (`speedAhead` — "expects ahead")
   - Whether it applies to the normal or diverging route (`route`)
   - A tier tag (`normal` or `diverging`) indicating when the indication is relevant
3. **Roles** — which indications serve special functions (danger, permissive, held, dark).
4. **Cascade** (optional) — for full-prototype types, the table of valid aspects given what the
   next signal ahead is showing. Omit for simple types; Bowties derives from speed.

The signal type is **system-agnostic**. The same 2-head NORAC color light signal type is used
whether the signal lives in ABS territory, at an interlocking, or on APB single-track. What changes
between those situations is the logic Bowties generates, not the signal type itself.

### Proposed YAML format

```yaml
name: "NORAC 2-head color light"
source: "NS-2008"                    # provenance for traceability

heads:
  - leds: 3
    labels: [G, Y, R]               # display hint — not a wiring constraint
  - leds: 3
    labels: [G, Y, R]

indications:
  - name: Clear
    rule: "N281"
    meaning: "Proceed at authorized speed"
    speed: Normal
    speedAhead: Normal
    route: normal
    tier: normal
    show: [green, red]

  - name: Approach Limited
    rule: "N281-B"
    meaning: "Proceed approaching next signal at Limited Speed"
    speed: Normal
    speedAhead: Limited
    route: normal
    tier: normal
    show: [yellow, flashgreen]

  - name: Approach Medium
    rule: "N282"
    meaning: "Proceed, approaching next signal at Medium Speed"
    speed: Normal
    speedAhead: Medium
    route: normal
    tier: normal
    show: [yellow, green]

  - name: Advance Approach
    rule: "N282-A"
    meaning: "Proceed prepared to stop at second signal"
    speed: Normal
    speedAhead: Medium
    route: normal
    tier: normal
    show: [flashyellow, red]

  - name: Approach
    rule: "N285"
    meaning: "Proceed prepared to stop at next signal"
    speed: Medium
    speedAhead: Stop
    route: normal
    tier: normal
    show: [yellow, red]

  - name: Stop Signal
    rule: "N292"
    meaning: "Stop."
    speed: Stop
    speedAhead: Stop
    route: either
    tier: normal
    show: [red, red]

  - name: Limited Clear
    rule: "N281-C"
    meaning: "Proceed at Limited Speed; then authorized speed"
    speed: Limited
    speedAhead: Normal
    route: diverging
    tier: diverging
    show: [red, flashgreen]

  - name: Medium Clear
    rule: "N283"
    meaning: "Proceed at Medium Speed; then authorized speed"
    speed: Medium
    speedAhead: Normal
    route: diverging
    tier: diverging
    show: [red, green]

  - name: Medium Approach
    rule: "N286"
    meaning: "Proceed prepared to stop at next signal; Medium Speed"
    speed: Medium
    speedAhead: Stop
    route: diverging
    tier: diverging
    show: [red, flashyellow]

  - name: Slow Clear
    rule: "N287"
    meaning: "Proceed at Slow Speed; then authorized speed"
    speed: Slow
    speedAhead: Normal
    route: diverging
    tier: diverging
    show: [red, red]              # on 3-head mast; not displayable on 2-head

  - name: Restricting
    rule: "N290"
    meaning: "Proceed at Restricted Speed"
    speed: Restricted
    speedAhead: Restricted
    route: either
    tier: normal
    show: [red, yellow]

  - name: Dark
    meaning: "Signal unlit (approach lighting inactive)"
    speed: Normal
    speedAhead: Normal
    route: either
    tier: normal
    show: [dark, dark]

roles:
  danger: "Stop Signal"
  permissive: "Restricting"
  held: "Stop Signal"
  dark: "Dark"

# Optional — omit for simple types (≤4 indications), include for full prototype.
# Converted from JMRI aspectMappings. Entries list all valid aspects for this
# signal when the next signal ahead shows [advance]. Route and topology select
# among multiple valid entries at installation time.
cascade:
  - advance: Clear
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Approach Limited
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Limited Clear
    valid: [Approach Limited, Restricting]
  - advance: Approach Medium
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Advance Approach
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Medium Clear
    valid: [Approach Medium, Restricting]
  - advance: Approach
    valid: [Advance Approach, Restricting]
  - advance: Medium Approach
    valid: [Approach Medium, Restricting]
  - advance: Restricting
    valid: [Approach, Medium Approach, Restricting]
  - advance: Stop Signal
    valid: [Approach, Medium Approach, Restricting]
```

### Design choices

| Choice | Rationale |
|---|---|
| Signal type = appearance + indications joined | The `show` field is inherently a property of appearance × system — you can't separate them without a cross-reference that adds complexity without enabling reuse. |
| Full indication table, always | The signal type describes capability; which indications are active is determined at installation by system + topology. No information loss, smooth upgrade path. |
| Tier tags on indications | `normal` indications are used everywhere. `diverging` indications activate only at junctions where routes diverge. This falls out of the user's answers — it is not a separate "fidelity" choice. |
| LED labels are hints | The Signal-LCC's R/G/Y/L markings are convenience labels. Any color LED can be wired to any driver. The capacity constraint is 16 LED drivers, full stop. |
| `show` uses abstract state names | Same vocabulary as JMRI: `green`, `yellow`, `red`, `lunar`, `dark`, `flashgreen`, `flashyellow`, `flashred`, `flashlunar`. Mapped to physical LED drivers at wiring time (mockup step 4). |
| `speed` / `speedAhead` on every indication | These are the inputs to cascade derivation. They make the condition table mechanically computable. |

---

## 2. How active indications are determined

The user never picks a "tier" directly. Bowties determines which indications are active based on two
things the user has already told it:

1. **System choice** (mockup frame 2): ABS / APB / Interlocking / CTC.
2. **Topology** (mockup frame 5): does a route diverge at this signal?

| System | Diverging route? | Active indications |
|---|---|---|
| ABS | No | `tier: normal` only |
| ABS | Yes | `tier: normal` + `tier: diverging` |
| APB | No | `tier: normal` only |
| APB | Yes | `tier: normal` + `tier: diverging` |
| Interlocking | Always yes | `tier: normal` + `tier: diverging` |
| CTC | Depends on location | both where routes diverge |

On straight track with no junctions, ABS and "full prototype ABS" produce identical behavior. There
is nothing to simplify — the signal type's `normal` tier indications ARE the full prototype for that
case.

The "simple vs. prototype-faithful" difference only surfaces **at junctions**, where the user can
choose between:

- **Simplified** (system = ABS): signal goes to Approach/Stop based on block occupancy and cascade;
  turnout position selects which next-signal to read but doesn't produce diverging-speed indications.
- **Full** (system = Interlocking): proper diverging speeds (Medium Clear, Slow Clear, etc.) with
  route locking, approach locking, flank protection.

This is the system choice the mockup already asks.

---

## 3. Cascade — the condition table is computed, not authored

The user never writes conditions. Bowties computes the per-installation condition table from the
signal type's cascade data + the user's installation bindings ("next signal ahead is X", "route
diverges toward Y").

### Two tiers of cascade complexity

**Simple ABS (≤4 active indications):** The cascade is trivially derivable from `speed`/`speedAhead`.
With only Clear, Approach, and Stop active, there is exactly one valid response per advance state:

| Block ahead | Next signal shows | This signal shows |
|---|---|---|
| Occupied | — | Stop |
| Clear | Stop | Approach |
| Clear | Approach or Clear | Clear |

No stored cascade table is needed. Bowties derives this from the speed hierarchy:

```
Normal > Limited > Medium > Slow > Restricted > Stop
```

**Full prototype (5+ active indications):** The cascade encodes railroad-specific signal engineering
policy that **cannot be derived from speed values alone**. Empirical testing against JMRI's NS-2008
data proves this: `Approach Slow` and `Approach` have identical speed/speed2 values (Medium/Slow)
but produce different valid-aspect sets — because one means "approaching slow" and the other means
"prepared to stop," and the appropriate preceding indications differ.

For full-prototype signal types, the cascade table is stored in the signal type YAML, converted
directly from JMRI's `aspectMappings`.

### The `cascade` section in the YAML

Optional. Omit for simple signal types (Bowties derives from speed). Include for full-prototype
types (converted from JMRI data):

```yaml
cascade:
  # "when the next signal ahead shows [advance], this signal may show any of [valid]"
  - advance: Clear
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Approach Limited
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Limited Clear
    valid: [Approach Limited, Restricting]
  - advance: Approach Medium
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Advance Approach
    valid: [Clear, Limited Clear, Medium Clear, Restricting]
  - advance: Medium Clear
    valid: [Approach Medium, Restricting]
  - advance: Approach
    valid: [Advance Approach, Restricting]
  - advance: Medium Approach
    valid: [Approach Medium, Restricting]
  - advance: Restricting
    valid: [Approach, Medium Approach, Restricting]
  - advance: Stop Signal
    valid: [Approach, Medium Approach, Restricting]
```

Multiple entries in `valid` mean the selection depends on route and other conditions — the
installation's topology resolves which one applies.

### How the per-installation condition table is produced

Given:
- The signal type (with cascade table)
- The user's bindings: next signal ahead (normal route), next signal ahead (diverging route)
- Block occupancy input

Bowties produces: for each active indication I, the condition under which I activates. The user
sees this as the "Review" screen (mockup frame 6) and confirms or adjusts.

### The "Adjust…" escape hatch

For genuine exceptions (a signal held for a drawbridge, an indication used only during a specific
move), the user can override one row of the derived table without turning the entire table into
hand-written logic. Most users never open it.

### Speed hierarchy is universal

All 52 JMRI signal systems use the same 10-value speed vocabulary. There are no railroad-specific
orderings. The hierarchy is a fixed global constant.

---

## 4. What changes per system — logic, not display

The signal type is the same across all four systems. What differs:

### ABS (Automatic Block Signaling)

- **Inputs**: block occupancy + speed published by next signal ahead
- **Logic runs on**: Signal-LCC (conditionals + track circuits)
- **Bowties generates**: conditional configuration fields
- **No route, no direction, no dispatcher control**

### ABS + CTC

- **Adds**: dispatcher hold (signal forced to Stop), turnout state selects which next-signal applies
- **At junctions**: diverging-route indications activate (Medium Clear, etc.)
- **Logic runs on**: Signal-LCC; possibly Tower LCC+Q for route lockout
- **Bowties generates**: conditional configuration + hold event bindings

### APB (Absolute Permissive Block)

- **Adds**: direction detection (temporal AND on adjacent blocks), direction latch, tumbledown
  (opposing signals forced to Stop)
- **Same signal indications as ABS** — the complexity is in the logic, not the display
- **Direction is a section property** — all signals in a section share it
- **Logic runs on**: Tower LCC+Q (needs latched state)
- **Bowties generates**: STL source code; Tower LCC+Q compiles on board

### Interlocking

- **Most demanding of the display**: diverging indications activate
- **Adds**: route set + locked before signal clears, opposing routes proved clear, approach locking,
  flank protection
- **Logic runs on**: Tower LCC+Q
- **Bowties generates**: STL source code

### Invariant across all systems

The mast always lives on a Signal-LCC — no other board in the range drives LEDs. For APB and CTC,
only the logic relocates to the Tower LCC+Q; it reaches the masts by sending the aspect events
they already listen for. Nothing about the signal type, the wiring step, or the LED driver model
changes.

---

## 5. LED driver model

### The constraint

A Signal-LCC has **16 LED drivers**. That is the capacity ceiling that matters. Not heads, not lamps,
not masts — LED drivers.

### Labels are not constraints

The drivers are labeled H1-G, H1-Y, H1-R, H1-L through H4-G, H4-Y, H4-R, H4-L. These labels are a
wiring convenience. Any color LED can be connected to any driver. Consequences:

- A 3-LED head (searchlight or color-light) uses 3 drivers, regardless of the head's lamp colors.
- Skipping the L (lunar) driver frees it for other use — e.g. five 3-LED heads fit on one board
  (15 drivers) instead of four 4-LED heads (16 drivers).
- A PRR position light head with 8 amber LEDs uses 8 drivers per head — only two heads fit per board.
- A searchlight signal with R/G/Y capability is one physical lamp housing but still 3 LED drivers
  (one per color element inside).

### Capacity tracking

The mockup's §5 shows five capacity ceilings. The "lamp drivers" ceiling is the one that binds most
often — heads and mast slots rarely exhaust before drivers do.

Bowties must:
- Track driver ownership centrally across all masts on a board
- Refuse assignments that would exceed 16
- Show which ceiling binds and why, so the user knows what to change

---

## 6. Relationship to JMRI signal data

JMRI's `xml/signals/` directory contains **52 signal systems** and **565 appearance files**, authored
primarily by Dick Bronson (Signal-LCC's designer). This data is directly convertible to Bowties
signal type YAML.

### What maps directly

| JMRI element | Bowties equivalent |
|---|---|
| Signal system directory (e.g. `NS-2008/`) | Source provenance (`source:` field) |
| `aspects.xml` → `<aspect>` | Indication entry |
| `<speed>` | `speed` ("permits past") |
| `<speed2>` | `speedAhead` ("expects ahead") |
| `<route>` | `route` |
| `appearance-*.xml` → `<show>` sequence | `show` field |
| `<specificappearances>` | `roles` |
| `<aspectMappings>` | `cascade` section (for full-prototype types) |
| `signalSpeeds.xml` | Speed hierarchy (global constant, not per-type) |

### Conversion rules

1. One JMRI appearance file → one Bowties signal type YAML (the natural unit is the join).
2. Head count = number of `<show>` elements per appearance entry.
3. LEDs per head = number of distinct non-`dark` states that head can show (3 for a standard color
   light: green/yellow/red; 4 if lunar is used).
4. Tier assignment: indications with `route: diverging` get `tier: diverging`; others get
   `tier: normal`.
5. The `dccAspect` field (DCC signal decoder address) is irrelevant and dropped.
6. The `aspectMappings` become the `cascade` section for full-prototype signal types.
   For simple types (≤4 indications), omit `cascade` — Bowties derives trivially from speed.

### What needs human judgment

- **Position light signals**: JMRI uses abstract colors (`green`/`yellow`/`red`) even for PRR position
  lights, which have no colored lamps. The Bowties signal type for a position light must define the
  actual LED arrangement (which amber LEDs in which positions) — this requires the §4 editor.
- **Tier boundaries for extended systems**: Some systems have indications that fall between basic ABS
  and interlocking (e.g. Advance Approach, Approach Slow). Editorial judgment assigns these to
  `tier: normal` since they appear in straight-track ABS with 4+ aspect signaling.

### Catalog shipping strategy

Convert the JMRI files mechanically for all color-light systems. For each system that has multiple
appearance files (e.g. NS-2008 has 7), produce one signal type per mast configuration:

- `norac-cls-3-hi.yaml` — single head, 3-color, high signal
- `norac-cls-3-3-hi.yaml` — double head, 3-color, high signal
- `norac-cls-3-3-3-hi.yaml` — triple head, 3-color, high signal
- `norac-cls-3-lo.yaml` — single head, 3-color, dwarf

Position-light and semaphore types require manual definition (§4 editor) and are expected to be
community-contributed rather than machine-converted.

---

## 7. Resolved questions

| # | Question | Resolution |
|---|---|---|
| 1 | Is the speed hierarchy sufficient to derive all cascade tables? | **Partially.** The hierarchy is universal (all 52 systems use the same 10 values, no railroad-specific orderings). It's sufficient for simple ABS (≤4 aspects). For full-prototype types (5+ aspects), the cascade encodes railroad-specific policy beyond speed — the signal type must carry the `cascade` section, sourced from JMRI data. |
| 2 | Does dispatcher "hold" interact with the signal type? | **No.** `roles.held` in the signal type says which indication to force. CTC hold is purely a logic-layer input that selects that indication. No per-system variation needed. |
| 3 | Should the signal type carry aspectMappings? | **Yes — as the `cascade` section.** Optional for simple types (Bowties derives trivially). Required for full-prototype types (converted from JMRI's `aspectMappings`). The per-installation "Adjust…" escape hatch handles remaining edge cases. |
| 4 | Is `normal`/`diverging` sufficient for tiers? | **Yes.** Extended-ABS indications (Advance Approach, Approach Slow) have `route: normal` and are already in `tier: normal`. They don't activate on single-head signals because the signal can't physically display them — filtered by displayability, not tier. |
| 5 | Should the YAML carry visual icons? | **No.** Bowties renders signal faces dynamically from `show` values + head layout. Pre-rendered images add maintenance burden for no gain. |
| 6 | Does Dark need to be a named indication? | **Yes.** Dark occupies a rule slot and publishes a speed (often Normal/Clear). Without it, the cascade breaks for approach-lit signals — the signal behind would see "no speed published" and restrict unnecessarily. |

### Dark as an indication

Every signal type that supports approach lighting must include Dark:

```yaml
  - name: Dark
    speed: Normal
    speedAhead: Normal
    route: either
    tier: normal
    show: [dark, dark]
```

The `speed: Normal` value means the signal behind treats this signal as if it were at Clear —
the correct behavior for an approach-lit signal that is dark because no train is approaching.

---

## 8. Remaining open questions

| # | Question | Impact |
|---|---|---|
| 1 | For the cascade `valid` list with multiple entries, is route always the disambiguator, or are there cases where block occupancy or other state selects among them? | Condition derivation completeness |
| 2 | Should the signal type carry a `darkSpeed` field separate from a Dark indication, or is Dark-as-indication the right model for all cases? | File format for approach-lit signals |
| 3 | How should the YAML handle flash rate or fade (incandescent simulation)? As indication-level data, or as a per-installation rendering choice? | File format scope |
