# docs/technical/ Migration

- **Areas**: documentation, aiwiki
- **Origin**: specs/012-knowledge-base plan (Tier 2)
- **Status**: deferred
- **Date**: 2025-05-10

Migrate remaining docs/technical/ files to their proper homes. layout-file-format.md → product/. AI-audience reference files (up to 8) → aiwiki/reference/. Drop profile-extraction-guide.md (superseded by profile extraction skills). Remove docs/technical/ when empty.

## Prior Work

- profile-extraction-guide.md is superseded by the profile-0 through profile-7 skills
- layout-file-format.md documents the .bowties.yaml schema — belongs in product/ as it defines user-visible behavior
- Some files may serve both human and AI audiences; use judgment on final placement
