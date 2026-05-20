---
name: slices
description: Generate a slice-organized task file from a feature's architecture assessment. Produces a cross-session progress tracker with vertical slices, HITL/AFK labels, and checkboxes. Use when user says "slices", "generate slices", or after running /design.
---

# Slice Task Generation

Generate a slice-organized task file from a feature's architecture assessment. Runs after `/design`, before `/build`.

## Process

### 1. Load context

1. Detect current feature from branch name or `$env:SPECIFY_FEATURE`
2. Read `specs/<feature>/plan.md` — requires the Architecture Assessment section (from `/design`)
3. Read `specs/<feature>/spec.md` for user stories and acceptance criteria

If plan.md lacks an Architecture Assessment section, tell the user to run `/design` first.

### 2. Draft slices

From the Architecture Assessment's Vertical Slices section, expand each slice into a task breakdown:

For each slice:
1. **Test task first** — write the integration test that proves the slice works
2. **Implementation tasks** — one per layer touched, in dependency order (deepest layer first)
3. **Validation checkpoint** — run the test, confirm it passes

Use the format in [SLICE-FORMAT.md](SLICE-FORMAT.md).

### 3. Quiz the user

Present the proposed breakdown as a numbered list. For each slice show:
- Title and HITL/AFK label
- Blocked-by relationships
- Task count and layers touched
- Estimated complexity (small / medium / large)

Ask:
- Does the granularity feel right? (too coarse / too fine)
- Are the dependency relationships correct?
- Are the HITL/AFK labels right?
- Should any slices be merged or split?

Iterate until the user approves.

### 4. Generate slices.md

Write the approved breakdown to `specs/<feature>/slices.md` using the format in [SLICE-FORMAT.md](SLICE-FORMAT.md).

### Handoff

Tell the user: "Slice file generated. Run `/build` to start TDD implementation."
