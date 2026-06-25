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

Facilities can also exist without templates (see Facility Lifecycle section, forthcoming). Users who configure logic directly can manually group their channels into a facility for the same comprehension and debugging benefits.

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
- Directly-configured logic can still be grouped into a facility manually, providing the same comprehension view and debugging surface
- The facility model does not require template origin — it supports both template-created and manually-assembled facilities

This means the system does not need to solve every possible logic scenario in the DSL. It needs to make the common path effortless while keeping the advanced path accessible.
