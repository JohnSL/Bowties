# aiwiki/playbooks/

- **Areas**: aiwiki, workflows
- **Origin**: specs/012-knowledge-base plan (Tier 2)
- **Status**: superseded
- **Date**: 2025-05-10

Create step-by-step playbook files in aiwiki/playbooks/ for common workflows: feature-work.md, regression-fix.md, profile-extraction.md, trace-analysis.md. Each playbook documents the end-to-end sequence of modules, commands, and checks involved.

## Superseded By

Prompts and existing skills now cover the workflows playbooks were meant to address:
- `bugfix.prompt.md` — regression fix workflow with pre-implementation analysis, TDD, and enrichment
- `quickchange.prompt.md` — focused change workflow with duplication prevention and enrichment
- `feature-finish` skill — graduation workflow for feature completion
- Profile extraction covered by `profile-0` through `profile-7` skills
- Trace analysis covered by `trace-analysis` skill
- `flows.md` already documents module participation per workflow at summary level

No additional value from a separate playbooks directory. If a new workflow type emerges that isn't covered by a prompt or skill, create a prompt for it rather than a playbook.
