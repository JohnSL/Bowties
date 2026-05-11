# AI Workflow Guide

- **Areas**: documentation, dark-factory
- **Origin**: specs/012-knowledge-base plan (Tier 2)
- **Status**: completed
- **Date**: 2025-05-10

Create docs/project/ai-workflow-guide.md documenting the "dark factory" operating model: how AI agents work with the codebase, which prompts/skills to use for different work types, what outputs to verify, where to intervene, and how to maintain the knowledge base.

## Prior Work

- Dark factory model: user owns behavior/design decisions, AI owns architecture/implementation
- Work types: bugfix (bugfix.prompt.md), quick change (quickchange.prompt.md), feature (speckit), architecture (improve-codebase-architecture skill)
- Output checks: pre-implementation analysis must be visible, test mapping from owners.md used for test runs
- Maintenance: aiwiki/ enrichment during feature work, ADRs for decisions, ideas capture for deferrals
