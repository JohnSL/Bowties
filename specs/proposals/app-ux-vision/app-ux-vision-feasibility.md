# Bowties UX Vision — Feasibility & Architecture Companion

**Status:** Draft — captures architectural thinking behind the UX vision.  
**Companion to:** [App UX Vision](./app-ux-vision.md)  
**Origin:** Feasibility analysis session, June 2026. Responds to external review feedback and documents the technical approach that makes the vision achievable.

---

## Template System Architecture

### Overview

Behavior templates are the mechanism that transforms channels (raw hardware I/O) into working railroad behavior (signal logic, interlocking, automation). The vision describes the user experience; this section describes how it works underneath.

The architecture has three layers:

1. **Templates (YAML)** — Declarative behavior descriptions: inputs, outputs, and condition → action rules. Human-readable, AI-authorable, target-agnostic.
2. **Target adapters (Rust)** — Per-provider modules that expose primitives for a specific logic execution environment. Each adapter knows how to express boolean logic in its native mechanism.
3. **Apply engine (Rust)** — Orchestrates template application: validates channel mappings, invokes the selected target adapter, and produces a write plan (CDI field writes for on-node targets, or API calls for LogixNG).

### Template Format

A template describes railroad behavior as condition → action rules over named channel states. It does not reference CDI paths, logic line numbers, or target-specific concepts.

```yaml
id: abs-3-aspect
name: "ABS 3-Aspect Signaling"
description: >
  Automatic Block Signaling with three aspects. Protects a single block
  by displaying Stop when occupied, Approach when the next block is
  occupied, and Clear when both blocks are clear.

inputs:
  - id: this_block
    type: block-occupancy
    label: "Protected block"
  - id: next_block
    type: block-occupancy
    label: "Next block ahead"

outputs:
  - id: signal
    type: signal-aspect
    aspects: [stop, approach, clear]
    label: "Protecting signal"

rules:
  - when: { this_block: occupied }
    then: { signal: stop }
  - when: { this_block: clear, next_block: occupied }
    then: { signal: approach }
  - when: { this_block: clear, next_block: clear }
    then: { signal: clear }

targets: [tower-lcc-logic, stl, logixng]
```

Templates are human-readable and human-verifiable: "when this block is occupied, signal shows stop" is auditable by anyone who understands ABS. They are also straightforward for AI to generate or update when adding new signaling patterns.

The DSL design will require further iteration to handle the full range of common signaling patterns (APB with directional authority, CTC interlocking, timed sequences). The goal is not to handle every possible scenario — it is to make the majority of cases expressible in the DSL, with uncommon cases handled through direct configuration.

### Target Adapters

Each target adapter is a Rust module that knows how to express boolean condition → action logic in a specific execution environment. The target set is small and stable:

| Target | Environment | Characteristics |
|---|---|---|
| **Tower-LCC Logic** | RR-CirKits Tower-LCC logic conditionals | 32 lines, 2 variables per line, 4 actions per line, boolean operations (AND, OR, XOR, etc.) |
| **STL** | RR-CirKits Tower-LCC+Q STL programs | Procedural, more expressive per line, larger capacity |
| **LogixNG** | JMRI LogixNG (via bridge) | Expression trees, unlimited capacity, requires computer running, works across all protocols |

An adapter provides:
- **Capacity query** — How many logic slots are free? What are the per-slot constraints?
- **Feasibility check** — Can this template's rule set fit on this target?
- **Instantiation** — Rules + mapped channels → write plan (CDI field writes or API calls)

The adapter handles target-specific concerns internally. For example, the Tower-LCC Logic adapter manages:
- **Allocation** — finding free logic lines (lines with no wired producer events on their variables)
- **Chaining** — rules with more than 2 conditions are split across multiple logic lines, with intermediate action events feeding the next line's variables
- **Defragmentation** — optionally shifting existing grouped logic lines to consolidate free space when allocation would otherwise fail

### Template Application Creates a Facility

Applying a template produces a **facility** — a named, live functional unit in the Railroad workspace. The facility is the container that ties together:
- The input channels the user mapped during application
- The logic rules instantiated on the target
- The output channels driven by the logic
- The target and resource allocation (e.g., "Tower-3, Logic Lines 5–7")

A facility created by template application knows its template origin, which enables:
- Re-application if the template is updated
- Comprehension view (the bowtie diagram showing inputs → logic → outputs)
- Debugging ("which rule is producing this aspect?")

Facilities are template-first: every facility originates from a template application. For advanced scenarios where a user has already configured logic directly (e.g., an experienced user with a pre-existing layout), a future workflow will allow attaching a template to existing configured elements — mapping what's already on the node into a facility structure. This path does not need to be as streamlined as starting fresh with a template; it serves technically proficient users who are willing to do more manual mapping.

### Capacity and Overflow

When a template's rules exceed a target's capacity, the adapter reports this at feasibility-check time — before any writes occur. The user sees:
- Which target(s) can accommodate the template
- Why a target can't (e.g., "requires 5 logic lines, only 3 available on Tower-3")
- Options: choose a different node with more capacity, choose a different target (LogixNG has no capacity limit), or reduce the template's scope

For the Tower-LCC Logic adapter specifically, capacity management includes:
- Detecting free lines by checking whether any other node produces events that a logic line's variables consume — if no producer exists for a line's consumer events, that line is unused
- Optionally defragmenting — shifting existing grouped logic lines to consolidate free space
- Chaining rules that exceed the 2-variable-per-line limit across multiple lines

### Scoping: Common Cases Easy, Uncommon Possible

The template DSL is designed to cover the **majority** of railroad signaling and automation patterns — ABS, APB, simple interlocking, detection-based automation. These are well-established, standardized patterns with bounded complexity.

For **uncommon cases** that exceed the DSL's expressiveness (complex custom interlocking, non-standard logic, experimental configurations):
- Users configure logic directly using the guided CDI editor or raw CDI view
- A future workflow will allow attaching a template to existing configured elements, bringing directly-configured logic into the facility model for comprehension and debugging
- The template DSL is designed to grow over time — patterns that start as uncommon can be formalized into templates as demand emerges

This means the system does not need to solve every possible logic scenario in the DSL. It needs to make the common path effortless while keeping the advanced path accessible.

---

## JMRI Bridge Sync Philosophy

### Strategy: Single Master First

The v1 target user is a non-technical model railroader who uses Bowties as their sole configuration tool. For this user, there is no second master — Bowties creates channels and facilities, the JMRI bridge projects them into JMRI objects, and JMRI serves as the downstream display and operation platform.

The two-master synchronization problem (what happens when both tools can write?) is a real concern for the secondary user — technically proficient people with existing JMRI layouts. That path is a deliberate later evolution, not a v1 constraint. The existing three-way sync model (shipping today for offline-edit → bus-connect reconciliation) provides the mechanism when that time comes.

### The Bridge Creates Atoms; JMRI Connects Them

The JMRI bridge's primary value is eliminating the tedious manual creation of JMRI objects. Today, a user must know to create a sensor with the right system name format, get the event IDs right, set up signal masts with the correct signal system and aspect mappings, then create Blocks and assign sensors to them. The bridge automates all of this.

JMRI objects fall into two tiers:

**Tier 1 — Bowties creates these** (event-backed, channel-mapped):

| JMRI Bean | Created From | System Name Identity | Event IDs Mutable? |
|---|---|---|---|
| **Sensor** | Occupancy, button, or current channel | Event IDs embedded in system name | No — delete + recreate |
| **Turnout** | Turnout position/command channel(s) | Event IDs embedded in system name | No — delete + recreate |
| **Signal Mast** | Signal aspect channel | Type + ordinal (no event IDs) | Yes — aspect events updated via setters |
| **Light** | LED state channel | Event IDs embedded in system name | No — delete + recreate |
| **Block** | Auto-created alongside occupancy Sensor | Separate system name | N/A — references Sensor by name |

**Tier 2 — JMRI connects these** (topology, logic — JMRI-owned):

| JMRI Structure | What It Does | Who Owns It |
|---|---|---|
| **Path + BeanSetting** | Block adjacency + required turnout positions | JMRI (Layout Editor drawing) |
| **LayoutBlock** | 1:1 wrapper around Block; runtime adjacency | JMRI (Layout Editor) |
| **Signal Mast Logic** | Auto-discovered signal rules (source → destination) | JMRI (auto-discover from topology) |
| **Section** | Ordered block sequence for dispatch/CTC | JMRI (manual or auto) |
| **LogixNG** | Conditional logic expressions | JMRI or Bowties (when template targets LogixNG) |

Bowties creates tier 1 objects. JMRI connects them via tier 2. The bridge reads tier 2 (via `GET /topology`) for comprehension and gap analysis but does not write it — with the exception of LogixNG when a template targets it as the logic execution platform.

### Channel → JMRI Bean Mapping

Each Bowties channel maps to a JMRI bean based on channel type:

| Bowties Channel Type | Direction | JMRI Bean | Notes |
|---|---|---|---|
| Block Occupancy | Producer | Sensor + Block | Bridge auto-creates Block and assigns Sensor |
| Turnout Position Feedback | Producer | Turnout (feedback side) | Combined into one Turnout bean with command |
| Turnout Command | Consumer | Turnout (command side) | Same Turnout bean — JMRI merges both directions |
| Signal Aspect | Consumer | Signal Mast | One event per aspect; events are mutable properties |
| LED State | Consumer | Light | On/off event pair |
| Button Press | Producer | Sensor | Pressed/released event pair |
| Current Sensor | Producer | Sensor | Active/inactive event pair |

The turnout case is notable: Bowties may model command and feedback as separate channels, but the bridge merges them into a single JMRI Turnout bean. This is a bridge-layer concern, not a channel-model concern — the internal representation can be resolved independently of the sync architecture.

### Facility → JMRI Structure

Facilities do not map to a single JMRI object. A facility spans multiple beans plus logic:

| Bowties Facility | JMRI Beans Created | Logic Target | JMRI Topology Equivalent |
|---|---|---|---|
| ABS Signal Block | Sensors (occupancy) + Block + Signal Mast | On-node or LogixNG | SignalMastLogic (auto-discovered from panel) |
| Turnout Control | Sensor (button) + Turnout | On-node or LogixNG | Turnout + triggering logic |
| Yard Lighting | Sensor (button) + Lights | On-node or LogixNG | LogixNG conditional |

JMRI has no general "facility" concept. The closest equivalents are Signal Mast Logic (for signal facilities) and Sections (for dispatch). Bowties' facility model is richer — it groups channels by behavioral intent, which JMRI doesn't represent.

### Sync Surface Area

For tier 1 beans (the only ones Bowties writes), the mutable properties are limited:

| Property | Sensor/Turnout/Light | Signal Mast |
|---|---|---|
| System name | Immutable (contains event IDs) | Immutable (type + ordinal) |
| User name | Mutable — sync target | Mutable — sync target |
| Comment | Mutable — sync target | Mutable — sync target |
| Event IDs | Immutable (in system name) | Mutable (per-aspect setters) |
| Inverted flag | Mutable — rarely changed | N/A |
| Feedback mode (turnout) | Mutable — rarely changed | N/A |

**What could collide between Bowties and JMRI:**

- **User name changes** — if renamed in both tools between syncs. Rare in the Bowties-first flow.
- **Signal mast aspect events** — if re-mapped in JMRI. Very rare.
- **Deletion** — if a user deletes a Bowties-managed bean in JMRI. Detectable via the `bowties.managed` property.

**What cannot collide:**

- Event IDs on sensors/turnouts/lights — immutable in the system name, and the system name IS the identity. If Bowties and JMRI both have a sensor with the same system name, they're referencing the same event IDs by definition.
- Tier 2 structures (paths, topology, signal mast logic) — JMRI-owned, Bowties reads only.

### Prior Art: Offline Sync Model

Bowties already ships a three-way sync model for offline changes. The same conflict classification applies to JMRI sync:

| Base → JMRI | Base → Bowties | Action |
|---|---|---|
| Unchanged | Unchanged | No action |
| Changed | Unchanged | Accept JMRI change |
| Unchanged | Changed | Push Bowties change |
| Changed | Changed (same) | No action (convergent) |
| Changed | Changed (different) | Conflict — present to user |

The existing sync panel UI handles conflict presentation and resolution. The JMRI bridge would use the same pattern with the same UI treatment.

### Structural Changes: Delete + Recreate

When an event ID changes in Bowties (e.g., channel rewired to different hardware), the corresponding JMRI sensor or turnout must be deleted and recreated because the event IDs are baked into the system name.

This has a downstream impact: any JMRI panel references to the old system name break. The bridge can detect this and warn the user ("changing this event ID will require recreating the JMRI sensor; panel references will need updating").

In practice, event ID changes are rare in the Bowties-first flow — event IDs are assigned at channel creation and don't change unless the user deliberately rewires hardware. Signal masts are exempt from this concern entirely, since their event IDs are mutable properties.

---

## Feasibility Assessment

### Development Context

Bowties is built by one experienced architect using AI-assisted development with a structured workflow (TDD-first, architecture enforcement via ADRs and placement rules, multi-session build tracking). The AI workflow includes profile extraction skills, subagent delegation, and enrichment gates that maintain architecture consistency automatically.

Implementation velocity and architecture quality are not constraints. The profile authoring pipeline is itself AI-assisted — adding a new board profile is a ~2-hour guided session, not weeks of manual specification. The template DSL is designed to be AI-authorable. The JMRI bridge is well-specified enough for AI-assisted implementation.

The primary remaining risk is **UX validation with the target audience** — will non-technical users actually find the experience accessible? This is only answerable by putting it in front of people.

### Existing Infrastructure

The vision builds on substantial existing infrastructure, not greenfield speculation:

| Capability | Status |
|---|---|
| Profile system (v2 schema, loader, resolver, annotation) | **Shipping** — 5 bundled profiles |
| AI profile extraction pipeline (8 skills, PDF → profile) | **Proven** — Tower-LCC and TurnoutBoss authored this way |
| CDI signature-based firmware variant detection | **Shipping** — Tower-LCC legacy vs. rev-C7 |
| Configuration Modes with variant overlays | **Shipping** — connector slots, Left/Right pairing |
| Connector selection + constraint evaluation | **Shipping** — auto-stages compatible field values |
| Offline change model + three-way sync | **Shipping** — full conflict classification and resolution |
| Placeholder boards (pre-stage config without hardware) | **Early** — read-only view ships; editable placeholders and full testing are pending |
| Channel data model + persistence | **In-flight** — architecture proven on branch |

### Confidence Table: Bowties as Single Master (Primary User)

Non-technical users who use Bowties as their sole configuration tool. JMRI is a downstream display/operation platform. No external writes to reconcile.

| Capability | Confidence | Key Factor |
|---|---|---|
| Workspace toggle + guided wiring view | ~92% | Evolutionary over shipping code |
| Channel abstraction + auto-creation | ~88% | Architecture proven; bounded types for v1 |
| Profile system across v1 boards | ~82% | AI extraction pipeline is the authoring tool; adding boards is cheap |
| Template DSL + ABS on Tower-LCC Logic | ~85% | Clear architecture (YAML DSL + Rust adapter); Tower-LCC logic was purpose-built for signal logic |
| Facility model (template-first) | ~85% | Simplified by template-first decision; straightforward data model |
| JMRI bridge (tier 1 bean creation) | ~75% | Well-specified API; small sync surface; no conflict resolution needed |
| Template → LogixNG (via bridge) | ~60% | Two unbuilt systems in series; both well-documented |
| Facility comprehension + live state | ~75% | Standard UI work with clear data model |
| Planner wizard | ~65% | UX-heavy (needs user validation), implementation straightforward |

### Confidence Table: Bidirectional with JMRI (Advanced User)

Technically proficient users with existing JMRI layouts who edit in both tools. Requires conflict detection and resolution.

| Capability | Confidence | Key Factor |
|---|---|---|
| Tier 1 bean sync (sensors, turnouts, masts, lights) | ~65% | Three-way merge model proven for offline sync; collision surface is small (user names, comments) |
| Conflict detection + resolution UI | ~70% | Pattern exists in offline sync panel; same approach applies |
| Signal mast event ID updates (mutable) | ~75% | JMRI setters exist; no delete+recreate needed |
| Sensor/turnout event ID changes (immutable) | ~55% | Requires delete+recreate; panel references break; needs user warning flow |
| LogixNG ownership (Bowties-created conditionals) | ~45% | Needs clear ownership marker; user could edit in JMRI's LogixNG editor |
| JMRI topology import (read-only) | ~70% | Read-only from Bowties side; no write conflicts; well-specified GET endpoint |
| Full bidirectional with concurrent editing | ~40% | Race conditions, snapshot window, thread safety in JMRI managers |

### Comparison to External Review

An external review estimated 30–40% confidence for "template-driven working signal logic across multiple board families." That assessment assumed:
- The profile system was net-new (it ships today with 5 bundled profiles)
- The template→logic path required a general-purpose compiler (it's a parameterized write plan for purpose-built hardware)
- JMRI sync required solving a two-master conflict problem (it's single-master-first with a small sync surface)
- A solo developer constraint implied both skill and velocity limitations

With the actual context — existing infrastructure, decided architecture, AI-assisted implementation, and explicit v1 scoping to common patterns on Tower-LCC — confidence for the "Now" horizon (working ABS signal logic via templates) is **~85%**.

### Remaining Risks

| Risk | Mitigation |
|---|---|
| DSL expressiveness limits for patterns beyond ABS | Explicit deferral; DSL grows iteratively. APB and interlocking are v2 patterns. |
| UX accessibility for non-technical users | Only validated by real user testing. Prototype early, iterate. |
| JMRI bridge + LogixNG as two unbuilt systems in series | Decoupled: bridge works without LogixNG; LogixNG works without templates (manual channel push). |
| Board count scaling beyond v1 | AI extraction pipeline keeps per-board cost low (~2 hours). Market is dominated by RR-CirKits; coverage is good with few profiles. |

### Conclusion: Value vs. Risk by Audience

The two target audiences have inverted risk profiles:

**Primary user (non-technical, Bowties as single master):**

| Dimension | Assessment |
|---|---|
| **Value** | High — market expansion. Makes LCC accessible to people who cannot use it today. This is the product's reason to exist. |
| **Engineering risk** | Low (~85% confidence). Infrastructure exists, architecture is decided, implementation is bounded. |
| **UX risk** | The real unknown. Will channels, facilities, and templates make sense to non-engineers? Only testable with real users. |

**Advanced user (technically proficient, bidirectional with JMRI):**

| Dimension | Assessment |
|---|---|
| **Value** | Moderate — productivity gain. Eliminates tedious manual work, adds comprehension and debugging. But these users *can* already do it manually; this is convenience, not enablement. |
| **Engineering risk** | Higher (~40–70% depending on sync seam). Ownership boundaries, concurrent editing, delete+recreate flows. |
| **UX risk** | Low. These users are technical and tolerant of rough edges. |

**Strategic implication:** Invest v1 in the primary user, where the risk is UX-testable rather than engineering-blocking. The advanced user's sync complexity is solvable once the foundation is solid — and that audience tolerates interim limitations (read-only JMRI visibility, manual object creation in the meantime).

This sequencing means:
- v1 delivers the highest-value outcome (market expansion) with the lowest engineering risk
- The UX risk is testable early — put the template apply workflow in front of non-technical users and iterate
- Advanced user features build on the same infrastructure (channels, facilities, bridge) without requiring the foundation to change
- The hard sync problems (bidirectional editing, LogixNG ownership) are deferred to a point where the product has users and feedback, not solved speculatively
