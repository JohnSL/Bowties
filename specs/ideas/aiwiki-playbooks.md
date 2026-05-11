# aiwiki/playbooks/

- **Areas**: aiwiki, workflows
- **Origin**: specs/012-knowledge-base plan (Tier 2)
- **Status**: deferred
- **Date**: 2025-05-10

Create step-by-step playbook files in aiwiki/playbooks/ for common workflows: feature-work.md, regression-fix.md, profile-extraction.md, trace-analysis.md. Each playbook documents the end-to-end sequence of modules, commands, and checks involved.

## Prior Work

- Lower priority: prompts and copilot-instructions.md already encode these workflows
- Profile extraction is covered by profile-0 through profile-7 skills
- Trace analysis is covered by the trace-analysis skill
- Main value would be for workflows not yet covered by dedicated skills
- flows.md already documents module participation per workflow at a summary level
