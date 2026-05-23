---
description: "Show available work items grouped by functional area, with source and origin tracing."
---

## Gather Items

Use the Explore subagent to collect items from these two sources:

1. **`specs/backlog.md`**: Read all bullet items. For each, extract a short title and one-line summary. If the item mentions a spec by number or name, record the origin as `spec NNN: Title` (look up the spec title from `specs/NNN-*/spec.md` if needed).

2. **Open GitHub issues labeled `kind/idea`** (`gh issue list --repo JohnSL/Bowties --label kind/idea --state open --json number,title,labels,body`): For each, extract the title, the `area/*` labels, and the Origin field from the body. If the origin references a spec, format as `spec NNN: Title`. Also include any residual files under `specs/ideas/**/*.md` (excluding `README.md`) until migration completes — read each, skip status `completed`/`superseded`, extract title, areas, origin, and bucket from the parent subfolder name.

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
| Short title — one-line description | `backlog` or `issue #N` or `idea` | `spec NNN: Title` or `—` |

## Rules

- Show the full spec number and title in the Origin column (e.g., `spec 010: Offline Layout Editing`), not just a number.
- If an item has a dependency, note it in the Item column (e.g., `*(depends on: X)*`).
- Do not scan `specs/*/` directories for unfinished spec tasks — only `specs/backlog.md`, open `kind/idea` issues, and any residual `specs/ideas/**` files.
- Prefer the issue's `area/*` labels when assigning to a group. For residual idea files, prefer the bucket (`features`/`refactors`/`docs`/`process`): `features/` → Features; `refactors/` → Tooling & Infrastructure or a more specific group based on area tags; `docs/` → Documentation & Cleanup; `process/` → Tooling & Infrastructure.
- Do not suggest or implement changes. This prompt is read-only.
