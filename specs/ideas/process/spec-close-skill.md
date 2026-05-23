# spec-close Skill

- **Areas**: skills, spec-lifecycle
- **Origin**: specs/012-knowledge-base plan (promoted to Tier 1 Phase 4)
- **Status**: promoted
- **Date**: 2025-05-10

Create a skill that helps close completed specs by analyzing completion, assessing unfinished work relevance, archiving the spec, and capturing still-relevant items as specs/ideas/ entries. User confirms relevance decisions before archiving.

## Design

Run after merging a feature to main:
1. Identify the spec directory under `specs/`
2. Analyze completion: compare spec requirements vs. what was implemented
3. Extract unfinished or deferred items as `specs/ideas/` entries (user confirms relevance)
4. Move spec directory to `specs/archive/`
5. Update `specs/backlog.md` if any items were resolved or revealed
6. Summary: what was archived, what ideas were captured, what backlog items changed

Does NOT delete anything — moves to archive. User confirms before the move.

## Prior Work

- Current archive process is manual (move to specs/archive/)
- No structured workflow for extracting reusable ideas from completed specs
- Several specs in specs/ may be candidates for closing (001-011)
- Natural companion to feature-finish: finish graduates knowledge before merge, spec-close archives after merge
