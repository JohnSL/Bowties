# Plan: Copilot Knowledge Base Governance

## Goals
1. Orientation speed: AI discovers codebase structure via markdown, not code exploration
2. Architecture protection: AI checks ownership, duplication, and placement before implementing
3. Prior-work reuse: deferred ideas are discoverable and reusable when relevant work begins

## Locked Decisions
1. Glossary: product/glossary.md (humans + AI)
2. ADRs: product/architecture/adr/ (shared audience, lightweight, "all three must be true")
3. aiwiki/: code-level navigation (WHERE + HOW). product/ = behavior (WHAT + WHY)
4. specs/ideas/: structured prior-work cache with area tags for discoverability
5. grill-with-docs: adapted to Bowties (product/glossary.md, product/architecture/adr/)
6. Dark factory model: user owns behavior/design, AI owns architecture/implementation
7. Two-tier plan: Tier 1 = direct impact (do now). Tier 2 = defer, capture as ideas.
8. Incremental enrichment: owners.md + flows.md get module-inventory bootstrap, then grow during feature work
9. Full guardrail stack: always-on + prompts + SpecKit integration
10. Precedence: product/ + code > aiwiki/ > specs/
11. Visible pre-implementation checks: prompts and agents require AI to output analysis before coding
12. owners.md structure: summary section at top (~20 lines), detailed inventory per layer below
13. Test mapping: each module entry in owners.md includes its test file(s) and cross-layer test relationships
14. Multi-session bootstrap: tracked via specs/012-knowledge-base/bootstrap-checklist.md

## Ideas format convention
Each idea file in specs/ideas/:
- Title, Areas (tags), Origin (which spec/plan/conversation), Status (deferred/exploring/superseded), Date
- One-paragraph description
- Prior work section (reusable analysis and decisions)

---

# TIER 1: Direct Impact

### Phase Status

Detailed item-level progress tracked in [bootstrap-checklist.md](bootstrap-checklist.md).

| Phase | Status | Items |
|-------|--------|-------|
| Phase 0: Bootstrap Tracking | ✅ Done (Session 1) | 2/2 |
| Phase 1: Foundation | ✅ Done (Session 1) | 29/29 |
| Phase 2: Skills & Prompts | ✅ Done (Session 2) | 9/9 |
| Phase 3: Integration | ✅ Done (Session 2) | 7/7 |
| Phase 4: Pilot | ⬜ Next | 0/4 |

---

## T1-Phase 0: Bootstrap Tracking Setup ✅ DONE (Session 1)

### T1-0A. Create specs/012-knowledge-base/ spec directory
- spec.md: brief description of the knowledge base initiative
- bootstrap-checklist.md: session-by-session progress tracker

## T1-Phase 1: Foundation ✅ DONE (Session 1)

### T1-1A. Create product/glossary.md (full bootstrap ~35-40 terms)
- Format from CONTEXT-FORMAT.md: canonical terms, avoid-lists, one-sentence definitions, relationships, flagged ambiguities
- Term clusters: Protocol (LCC, OpenLCB, CAN, Node, Node Alias, Node ID, Event ID, Producer, Consumer, CDI, SNIP, PIP, MTI, Datagram), App model (Bowtie, Connector, Pill, Connection Element, Display Name, Display Name Fallback), Architecture roles (Route, Component, Orchestrator, Store, Util, Transport Actor, Node Proxy), Data/workflow (Layout, Sync Session, Modified Value, Pending Change, Offline Change, Config Read Session), Profile system (Profile, Relevance Rules, Cascade Rules, Guided Configuration)

### T1-1B. Create aiwiki/ structure with module-inventory bootstrap
- aiwiki/README.md: governance, scope, enrichment model, format rules
- aiwiki/owners.md: summary section, detailed inventory per layer, shared conventions, integration boundaries, test mapping
- aiwiki/flows.md: module participation per workflow (12+ workflows)
- aiwiki/architecture-health.md: empty template for depth assessments

### T1-1C. Create product/architecture/adr/
- README.md with format guidance

### T1-1D. Create specs/ideas/ with structured format
- README.md with format convention
- 8 Tier 2 idea files captured

### T1-1E. Rename grill-width-docs to grill-with-docs

## T1-Phase 2: Skills & Prompts ✅ DONE (Session 2)

### T1-2A. Adapt grill-with-docs (3 files in renamed directory)
- SKILL.md: CONTEXT.md → product/glossary.md, docs/adr/ → product/architecture/adr/
- CONTEXT-FORMAT.md → GLOSSARY-FORMAT.md
- ADR-FORMAT.md: updated path to product/architecture/adr/

### T1-2B. Fix improve-codebase-architecture SKILL.md
- 6 text replacements: CONTEXT.md, docs/adr/, ../grill-with-docs/ references
- Points to product/glossary.md and product/architecture/adr/

### T1-2C. Create feature-finish skill
- .github/skills/feature-finish/SKILL.md
- 9-step graduation: identify spec, diff, assess product/ impact, assess aiwiki/ impact, update, consistency check, capture deferrals as ideas, backlog check, summary
- Includes aiwiki/ enrichment verification
- Does NOT commit or merge

### T1-2D. Create workflow prompts
- bugfix.prompt.md: check specs/ideas/ for prior work, identify owner via aiwiki/owners.md, check for existing shared logic, encode regression as test, fix narrowly, run affected tests, enrich aiwiki/. Visible pre-implementation analysis required.
- quickchange.prompt.md: check specs/ideas/ for prior work, check aiwiki/ for affected layers and shared logic, implement with TDD, verify no duplication, run affected tests, enrich aiwiki/. Visible pre-implementation analysis required.

### T1-2E. Update speckit.implement agent
- Pre-implementation step: check specs/ideas/ and aiwiki/owners.md
- Architecture check: no duplication, correct layer placement
- Enrichment step: update aiwiki/
- ADR step: record architecture decisions

### T1-2F. Update speckit.analyze agent
- Prior-work discovery step: scan specs/ideas/ for matching area tags
- Surface relevant ideas during analysis

## T1-Phase 3: Integration ✅ DONE (Session 2)

### T1-3A. Harden copilot-instructions.md
- Source Of Truth: added aiwiki/ and product/architecture/adr/
- Copilot Knowledge Base section: read-order, enrichment guidance
- Pre-Implementation Checks section (8 checks)
- Post-Work Enrichment section
- Staleness detection line

### T1-3B. Update product/README.md
- Added glossary and ADR directory to key documents index

### T1-3C. Update docs/project/development.md
- Added aiwiki/ and specs/ideas/ to doc index

### T1-3D. Fix .github/instructions/ applyTo patterns
- Updated for renamed grill-with-docs directory

## T1-Phase 4: Pilot ⬜ NEXT

Use KB for 2-3 real features. Each feature:
1. AI reads aiwiki/ for orientation (does it find what it needs?)
2. AI checks specs/ideas/ for prior work
3. AI runs pre-implementation checks (placement, duplication, ADR history)
4. Implement with TDD
5. Enrich aiwiki/ for touched areas
6. Run feature-finish

### Evaluation
- Orientation: did AI need fewer exploratory tool calls?
- Architecture: did AI check for existing logic before implementing?
- Ideas: did prior-work cache save re-discovery time?
- Enrichment: did aiwiki/ grow naturally during feature work?

---

# TIER 2: Supporting Infrastructure (deferred, captured as ideas)

Each captured in specs/ideas/:

| Idea | Areas | File |
|------|-------|------|
| docs/design/ redistribution | documentation, cleanup | docs-design-redistribution.md |
| docs/technical/ migration | documentation, aiwiki | docs-technical-migration.md |
| spec-close skill | skills, spec-lifecycle | spec-close-skill.md |
| Layer-specific instruction enrichment reminders | instructions, enrichment | instruction-enrichment-reminders.md |
| ai-workflow-guide.md (user guide) | documentation, dark-factory | ai-workflow-guide.md |
| aiwiki/playbooks/ | aiwiki, workflows | aiwiki-playbooks.md |
| CI workflow trigger updates | ci, documentation | ci-path-updates.md |
| Git hooks | enforcement, ci | git-hooks-enforcement.md |

---

## Scope Summary

Tier 1 deliverables: ~18 files created/modified
- 1 spec directory with bootstrap checklist (specs/012-knowledge-base/)
- 1 glossary (product/glossary.md)
- 4 aiwiki files (README, owners, flows, architecture-health)
- 1 ADR directory + README (product/architecture/adr/)
- 1 ideas directory + README + 8 Tier 2 idea files (specs/ideas/)
- 3 skill files (grill-with-docs adapt, improve-codebase fix, feature-finish new)
- 2 prompt files (bugfix, quickchange)
- 2 agent updates (speckit.implement, speckit.analyze)
- 1 directory rename (grill-with-docs)
- 3 doc updates (copilot-instructions hardened, product/README, development.md)
- 1 instruction applyTo fix

Tier 2 deferred: 8 ideas captured in specs/ideas/
