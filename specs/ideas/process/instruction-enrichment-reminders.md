# Layer-specific Instruction Enrichment Reminders

- **Areas**: instructions, enrichment
- **Origin**: specs/012-knowledge-base plan (Tier 2)
- **Status**: deferred
- **Date**: 2025-05-10

Add 2-3 line enrichment reminders to each of the 9 .github/instructions/*.instructions.md files, prompting AI to update aiwiki/ when touching modules in that layer.

## Prior Work

- copilot-instructions.md already covers enrichment always-on (Phase 3 deliverable)
- Layer-specific reminders are lower priority since the always-on instructions already mandate enrichment
- Would provide defense-in-depth for cases where always-on instructions are insufficient
- 9 instruction files: archive-specs, backend-tauri, frontend-components, frontend-orchestration, frontend-routes, frontend-stores, frontend-utils, lcc-rs, product-docs
