# Implementation Plan: Profile Schema, Event Roles, and Conditional Relevance

**Branch**: `008-guided-configuration` | **Date**: 2026-03-01 | **Spec**: [spec-phase2.md](spec-phase2.md)
**Input**: `specs/008-guided-configuration/spec-phase2.md` — Phase 2 of Guided Configuration

## Summary

Phase 2 defines the **structure profile** (`.profile.yaml`) file format, loads profiles automatically by node type (manufacturer + model) in the Rust backend, and applies the profile data synchronously before the frontend receives any config tree: (1) profile-declared event roles override the heuristic pipeline on event group leaves; (2) profile-declared relevance rules are embedded as annotations on `GroupNode`s and evaluated reactively in the frontend to collapse irrelevant sections with explanation banners. A Tower-LCC structure profile is authored and bundled. Phase 2A tooling delivers a CDI template generator script and a `profile-7-assemble` skill.

## Technical Context

**Language/Version**: Rust 2021 (stable 1.75+) — backend; TypeScript 5.x / SvelteKit 2.x / Svelte 5 — frontend

**Primary Dependencies (new)**:
- `serde_yaml_ng = "0.10"` — YAML deserialization in Rust (community successor to deprecated `serde_yaml 0.9`; drop-in API; full git history; ~2M downloads)
- All other crates already present: `serde`, `thiserror`, `tokio`, `tauri 2.x`, `lazy_static`

**Primary Dependencies (existing, relevant)**:
- `lcc_rs::cdi::EventRole` — `Producer | Consumer | Ambiguous`
- `crate::node_tree::{NodeConfigTree, GroupNode, LeafNode, merge_event_roles}` — tree merge pipeline
- SvelteKit: `nodeTreeStore` (Svelte 5 `$state` map), `pendingEditsStore`
- Frontend component chain: `SegmentView` → `TreeGroupAccordion` → `PillSelector` / `TreeLeafRow`

**Storage**: YAML files on disk (`.profile.yaml`). Two discovery paths:
1. **Bundled** — shipped in Tauri `bundle.resources`, read via `app_handle.path().resolve(path, BaseDirectory::Resource)`
2. **User-placed** — `app_handle.path().app_data_dir().join("profiles")`, takes precedence over bundled

No database changes.

**Testing**: `cargo test` (unit + integration for `profile` module); Vitest (frontend `TreeGroupAccordion` reactivity tests)

**Target Platform**: Desktop — Windows, macOS, Linux (Tauri 2.x cross-platform)

**Project Type**: Tauri 2 (Rust backend + SvelteKit frontend) — changes to both layers

**Performance Goals**:
- Profile load + resolve: < 50ms per node type (budget: 400 kB Tower-LCC profile, name-path resolution over ~250 CDI groups)
- Relevance state update on controlling-field change: ≤ 200ms visual transition (driven by Svelte reactivity, no IPC round-trip)
- No change to CDI read or config read performance

**Constraints**:
- Profile MUST be fully applied before `get_node_tree` returns — no asynchronous update, no secondary event
- Nodes without a matching profile MUST behave identically to pre-feature (zero visible profile UI)
- Profile file with any structural error MUST be silently skipped with a log warning; no panic, no user-visible error
- `allOf` rules with > 1 condition silently skipped with log warning (V1 evaluator processes only single-field rules)

**Scale/Scope**:
- 1 bundled built-in profile (Tower-LCC) at launch; format supports N profiles
- Tower-LCC CDI: ~250 named groups across 6 segments; ~400 event slots; 3 relevance rules (consumer events, producer events, Delay group)
- Profile file sizes expected < 50 KB (YAML is human-authored)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Before Design | After Design |
|------|--------------|--------------|
| **I. Rust 2021+** | ✅ `profile` module uses Rust 2021; `serde_yaml_ng` supports stable Rust 1.70+ | ✅ All new types use `derive`, `?`, `async/await`; no `unwrap()` in production paths |
| **II. Cargo-based** | ✅ Single new `[dependencies]` line in `app/src-tauri/Cargo.toml` | ✅ Profile files bundled via `tauri.conf.json` — no non-Cargo toolchain |
| **III. TDD (MANDATORY)** | ✅ Profile module gets unit tests for YAML deserialisation, path resolver, event role merge | ✅ `profile::resolver` gets property tests: resolve(path) roundtrip; `profile::loader` gets integration test using fixture YAML |
| **IV. LCC Protocol Correctness** | ✅ Profile is application-layer metadata; zero protocol changes | ✅ No MTI, datagram, or addressing logic touched |
| **V. UX-First** | ✅ Correct event role badges + irrelevant-section suppression are primary user-visible outcomes | ✅ Explanation banner text drawn verbatim from profile (no app-generated paraphrase) |
| **VI. TCP-Only** | ✅ No transport changes | ✅ |
| **VII. Event Management Excellence** | ✅ Accurate role labels + bowtie ambiguity resolution directly improve event wiring UX | ✅ Profile-declared roles feed `build_bowtie_catalog` same-node resolution path |

## Project Structure

### Documentation (this feature)

```text
specs/008-guided-configuration/
├── plan.md                    # This file — Phase 2 plan
├── research.md                # Phase 1 research (extraction tooling)
├── research-phase2.md         # Phase 2 research (profile format, integration architecture)
├── data-model.md              # Phase 1 data model (extraction schemas)
├── data-model-phase2.md       # Phase 2 data model (StructureProfile types, GroupNode changes)
├── quickstart.md              # Phase 1 quickstart (extraction workflow)
├── quickstart-phase2.md       # Phase 2 quickstart (implementation guide)
├── contracts/
│   ├── prompt-a-event-roles.md     # Phase 1 (extraction)
│   ├── prompt-b-relevance-rules.md # Phase 1 (extraction)
│   ├── prompt-c-section-descriptions.md
│   ├── prompt-d-field-descriptions.md
│   ├── prompt-e-recipes.md
│   ├── validation-workflow.md
│   ├── profile-yaml-schema.json    # Phase 2 — .profile.yaml JSON Schema
│   ├── backend-profile-module.md   # Phase 2 — Rust profile module API contract
│   └── group-node-updated.md       # Phase 2 — Updated GroupConfigNode type contract
└── tasks.md                   # Phase 2 tasks (/speckit.tasks command)
```

### Source Code (repository root)

```text
app/src-tauri/
├── Cargo.toml                   # + serde_yaml_ng = "0.10"
├── profiles/                    # Bundled built-in profiles (Tauri resources)
│   └── RR-CirKits_Tower-LCC.profile.yaml
├── src/
│   ├── lib.rs                   # + mod profile;
│   ├── commands/
│   │   └── cdi.rs               #   get_node_tree — apply profile before return
│   ├── node_tree.rs             #   GroupNode + relevance_annotation field
│   └── profile/
│       ├── mod.rs               #   pub API: ProfileStore, load_profile()
│       ├── types.rs             #   StructureProfile, EventRoleDecl, RelevanceRule
│       ├── loader.rs            #   YAML loader: built-in resources + user data dir
│       └── resolver.rs          #   CDI name-path → tree index-path translation

app/src/
├── lib/
│   ├── types/
│   │   └── nodeTree.ts          # GroupConfigNode + RelevanceAnnotation field
│   └── components/ElementCardDeck/
│       ├── TreeGroupAccordion.svelte  # + relevance rule evaluation
│       └── TreeLeafRow.svelte         # + "not applicable" muted treatment

.github/skills/
└── profile-7-assemble/
    └── SKILL.md                 # New skill: assemble .profile.yaml from Phase 1 outputs

scripts/
└── cdi-template-generator/
    ├── README.md
    └── generate-profile-template.py  # CDI XML → empty .profile.yaml skeleton
```

**Structure Decision**: Tauri 2 (backend + frontend) structure. New `profile/` module is backend-only Rust code; frontend changes are limited to type additions and component logic in the existing `ElementCardDeck` component tree.

## Complexity Tracking

> No constitution violations. No complexity exceptions needed.
