---
description: "Show available work items grouped by functional area, with source and origin tracing."
---

## Gather Items

Use the Explore subagent to collect items from these two sources:

1. **`specs/backlog.md`**: Read all bullet items. For each, extract a short title and one-line summary. If the item mentions a spec by number or name, record the origin as `spec NNN: Title` (look up the spec title from `specs/NNN-*/spec.md` if needed).

2. **`specs/ideas/*.md`** (excluding README.md): Read each file. Skip items with status `completed` or `superseded`. Extract the title (from `# heading`), areas (from `Areas` field), and origin (from `Origin` field). If the origin references a spec, format as `spec NNN: Title`.

## Group and Present

Group items by functional area. Assign each item to the most fitting group based on its content and area tags. Use these groups (add or remove groups as needed to fit the actual data — do not show empty groups):

- **Features** — new capabilities, enhancements
- **Bugs & Fixes** — defects, correctness issues, misconfigurations
- **Profiles & Connectors** — profile authoring, connector rules, daughterboard evidence
- **Documentation & Cleanup** — doc reorganization, migration, archiving
- **Tooling & Infrastructure** — CI, skills, prompts, enforcement, release workflow

For each group, output a markdown table:

| Item | Source | Origin |
|------|--------|--------|
| Short title — one-line description | `backlog` or `idea` | `spec NNN: Title` or `—` |

## Rules

- Show the full spec number and title in the Origin column (e.g., `spec 010: Offline Layout Editing`), not just a number.
- If an item has a dependency, note it in the Item column (e.g., `*(depends on: X)*`).
- Do not scan `specs/*/` directories for unfinished spec tasks — only `specs/backlog.md` and `specs/ideas/`.
- Do not suggest or implement changes. This prompt is read-only.
