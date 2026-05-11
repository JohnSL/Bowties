# Git Hooks for Knowledge Base Enforcement

- **Areas**: enforcement, ci
- **Origin**: specs/012-knowledge-base plan (Tier 2)
- **Status**: deferred
- **Date**: 2025-05-10

Add git hooks to enforce knowledge base maintenance: pre-commit warnings when product/ or aiwiki/ files aren't updated alongside code changes, pre-push blocks for high-risk changes (lifecycle ownership, normalization, sync triggers) without corresponding doc/test updates.

## Prior Work

- Separate future plan — not needed until KB is established and proven via pilot
- Risk of false positives if hooks are too aggressive early on
- Consider starting with warnings only, upgrading to blocks after pilot evaluation
- May conflict with existing CI checks; coordinate with frontend-regression-gate workflow
