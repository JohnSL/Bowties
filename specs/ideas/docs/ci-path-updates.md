# CI Workflow Path Updates

- **Areas**: ci, documentation
- **Origin**: specs/012-knowledge-base plan (Tier 2)
- **Status**: deferred
- **Date**: 2025-05-10

Update .github/workflows/frontend-regression-gate.yml paths if docs/technical/ files are moved during the docs/technical/ migration idea. CI triggers may reference paths that no longer exist after reorganization.

## Prior Work

- Blocked by: docs-technical-migration idea (this is a follow-on)
- Check actual CI workflow file for path references before acting
- May also affect any other workflows that reference docs/ paths
