# Proposal: Configuration Modes & Profile Explorer

**Status:** Draft proposal — input for `/speckit.specify`.
**Origin:** Conversation analyzing GitHub issue #8 (TurnoutBoss profile submission) and the gap between the v1 profile schema and what real boards need.

---

## Problem

The v1 structure profile schema (see `specs/008-guided-configuration/contracts/profile-yaml-schema.json`) supports two narrow shapes:

1. **Group-level event roles** — every eventid leaf inside a CDI `<group>` gets the same Producer/Consumer role.
2. **Sibling-only relevance rules** — the controlling field of a relevance rule must be a sibling within the same replicated group instance as the affected group; the affected target must be a group (not a leaf).

Two concrete cases prove this is too narrow:

### Tower-LCC daughterboards
Tower-LCC already needed a richer shape: `connectorSlots`, `daughterboardReferences`, `connectorConstraintVariants`. Plugging a daughterboard into a connector reshapes large blocks of fields and events at once. This was built as a Tower-LCC-shaped extension, not a general capability.

### TurnoutBoss (issue #8)
The "Left" vs "Right" enum (`Layout Configuration Setup/How this TurnoutBoss is used on your layout.`) drives:
- Which detector blocks are wired (Detector 3 only on Left boards).
- Which Layout Connections neighbor is meaningful.
- **Per-leaf role inversion** in the `Occupancy` group: the same 12 eventid slots are Producer-or-Consumer depending on Left/Right. Cannot be expressed by group-level role assignment.
- Cross-segment relevance: e.g. "Detector 3 is irrelevant when Right" — controlling field is in segment `Layout Configuration Setup`, affected field is in segment `Hardware Configuration`.

Several other TurnoutBoss enums (`Facing Point Signals` single/double head, `Using turnout feedback sensors?`, `Control Buttons`) follow the same pattern at smaller scale: a configuration enum gates the relevance and/or role of fields and events elsewhere in the CDI.

These are **all instances of the same idea**: *a configuration choice overlays the rest of the CDI*. Tower-LCC just happened to express the most structural form of it first.

A second pain point: there is no way today to see what a board's configuration surface looks like **without owning the hardware and connecting it**. A user evaluating whether a board suits their layout, or a profile author validating their work, both have to round-trip through real hardware. This blocks pre-purchase exploration and slows profile contribution.

---

## Concept: Configuration Mode

Introduce a first-class concept called a **Configuration Mode** (or **Operational Variant**) in the profile schema.

A Configuration Mode is a *selector* over a CDI configuration choice (or installed-variant choice) that supplies a named set of **overlays** to apply to the rest of the profile.

Each mode declares:
- **What selects it.** Either an enum field path in the CDI (e.g. `Layout Configuration Setup/How this TurnoutBoss is used on your layout.`) or a structural slot+variant (e.g. `connector-a` = `BOD4-CP`).
- **Named variants.** One per legal value of the selector (e.g. `Left`, `Right`, or `BOD4`, `BOB-S`, etc.).
- **Per-variant overlays.** Each variant can declare any of:
  - **Event role overrides** (group-level, per-replication-instance, or per-leaf).
  - **Relevance rules** that fire when this variant is active (affected target may be a group, a replication instance, or a leaf; controlling reference may be anywhere in the CDI).
  - **Structural constraints** (replaces today's `connectorConstraintVariants`: required paths, enum entry counts, etc.).

Tower-LCC's existing connector/daughterboard support becomes one *kind* of Configuration Mode (slot + installed variant); TurnoutBoss Left/Right becomes another (enum-driven). The same evaluator handles both.

The schema must also relax two specific v1 restrictions:

- **Per-leaf role overrides.** Allow event role assignment at the eventid-leaf level, not just the group level. Required for TurnoutBoss `Occupancy`, `Turnout Control`, `Signal Controls`.
- **Cross-segment / leaf-targeted relevance.** Allow `relevanceRules.allOf[].field` to reference any CDI field by full path, and allow `affectedGroupPath` (rename if appropriate) to target a leaf or replication instance, not only a group.

These two relaxations are required regardless of the Configuration Mode work; the mode concept then *layers* on top.

---

## Feature: Profile Explorer

Add a new in-app entry point: **Explore Board**.

From this entry point a user can pick any bundled `.profile.yaml` (and its associated CDI XML, also bundled) and walk the same guided-configuration screens they would see for a real node, **without a connected node**.

Key behaviors:
- All Configuration Mode selectors are interactive. Flipping Left↔Right on TurnoutBoss must visibly re-shape the configuration surface (relevant sections, event roles, irrelevance banners).
- Daughterboard-style selectors behave the same way: choose `BOD4-CP` on connector A and see the line groups reshape.
- No writes. Nothing is persisted to disk for the explored profile; this is a read-only exploration sandbox.
- Surfaces the profile's `recipes.yaml`, field descriptions, and relevance explanations as informational content alongside the configuration view, so a prospective buyer can read what a board does.

Profile authors get the same screen as a built-in preview tool for their work.

---

## Validation Case

The TurnoutBoss profile submitted in issue #8 is the validation case for the whole spec:

1. Source files are already staged at `profile-extractions/turnout-boss/` (CDI XML, manual PDF, all phase-1 extraction outputs).
2. The spec must produce an assembled `app/src-tauri/profiles/Mustangpeak Engineering_TurnoutBoss.profile.yaml` (or the loader-matching filename variant) that expresses all of:
   - All eventRoles for groups whose leaves share a role, **including the per-leaf overrides** for Occupancy / Turnout Control / Signal Controls.
   - All seven relevance rules from `relevance-rules.json` (R001–R007), including the cross-segment and leaf-targeted cases.
   - A `Left`/`Right` Configuration Mode driven by `Layout Configuration Setup/How this TurnoutBoss is used on your layout.`, with the Occupancy role overlay defined per variant.
3. The Profile Explorer must render that profile correctly in both `Left` and `Right` modes and demonstrate that Detector 3, Signal D, the In-Motion event, etc. light up and dim out as the controlling fields change.

If this works end-to-end, the schema generalization is proven.

---

## Migration

Tower-LCC must migrate to the unified Configuration Mode shape without behavioral regression.

- The existing `connectorSlots` / `daughterboardReferences` / `connectorConstraintVariants` fields can either be (a) re-expressed under the new schema and the old fields removed, or (b) kept as backwards-compatible aliases that the loader normalizes to the new shape. The spec should pick one explicitly.
- Existing Tower-LCC behavior in the guided-configuration UI must remain identical.
- Tests covering current Tower-LCC connector/daughterboard behavior must continue to pass without modification of test intent.

---

## Non-Goals

- **Hardware planner / pre-build wizard.** Captured separately in `app-ux-vision/planner-proposal.md` and to be filed as a `kind/idea` GitHub issue. This proposal must not bake in planner-specific metadata; the planner is expected to layer on top of profiles in a later spec.
- **Profile-author tooling beyond the explorer.** No new authoring UI, no schema editor, no profile linter beyond what `profile-6-validate` already provides.
- **Recipe execution from the explorer.** The explorer shows recipes as informational text; it does not run them. Recipe execution against a live node remains in the existing guided-configuration flow.
- **Persisting explored configurations.** The explorer is read-only.
- **Status modules / runtime data.** Covered by the existing `specs/ideas/features/status-page-and-status-modules.md` idea; this spec does not pre-commit to that direction.

---

## Open Design Questions (to resolve during `/speckit.specify` and `/plan`)

1. **Selector kinds.** Should the schema model "enum-field-driven mode" and "slot-variant-driven mode" as two sibling concepts, or unify them under a single selector abstraction with two implementations?
2. **Default variant.** How does a mode behave before the user has chosen a variant (e.g. fresh node, never configured)? Use the CDI `<default>` value? Show all variants' content? Defer relevance evaluation until a value exists?
3. **Composition order.** When multiple modes/rules apply, in what order are overlays composed? Is there a need for explicit precedence, or does a deterministic order (e.g. mode declaration order, then per-variant rule order) suffice?
4. **Migration approach for Tower-LCC.** Inline rewrite vs backwards-compatible alias. Affects how many touchpoints need to change at once.
5. **Filename / loader conventions.** Bundled profile filenames currently use `{Manufacturer}_{Model}.profile.yaml`. Confirm the loader's exact matching rules (spaces, casing) before assembling TurnoutBoss.
6. **Explorer entry point placement.** Top-level nav item? Sub-section of an existing screen? Reachable from a "no node yet?" empty state?
7. **CDI XML bundling.** The Profile Explorer needs the per-board CDI XML at runtime. Confirm whether to bundle each CDI XML alongside its profile under `app/src-tauri/profiles/`, or load on demand from the `profiles/<node-name>/` source tree. Tower-LCC's source CDI is currently *not* checked in — that gap must be closed as part of this spec.

---

## Success Criteria

- One unified schema concept (Configuration Mode) expresses both Tower-LCC's daughterboard model and TurnoutBoss's Left/Right model.
- The TurnoutBoss profile assembles, validates against the new schema, and renders correctly in both modes in the explorer.
- Tower-LCC continues to behave identically to today in the guided-configuration UI.
- A user with no LCC hardware can pick TurnoutBoss in the Profile Explorer, flip Left↔Right, and observe the configuration surface change accordingly.
- The contract for "where does a new profile live, and how do I preview it before connecting hardware" is documented for profile authors.

---

## Pointers

- `profile-extractions/turnout-boss/` — staged validation-case source files (CDI XML, manual PDF, phase-1 extraction outputs).
- `profile-extractions/tower-lcc/` — existing Tower-LCC extraction outputs (CDI XML and manual PDF are missing — backfill if convenient).
- `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml` — current Tower-LCC profile, source of the existing connector/daughterboard shape to be generalized.
- `specs/008-guided-configuration/contracts/profile-yaml-schema.json` — current v1 schema to extend.
- `.github/skills/profile-7-assemble/SKILL.md` — current assembly skill; will need updating once schema lands.
- GitHub issue #8 — TurnoutBoss profile submission and reported CDI cache bugs (the bugs are tracked separately; only the profile submission is in scope here).
