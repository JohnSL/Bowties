# Git Hooks for Knowledge Base Enforcement

- **Areas**: enforcement, ci
- **Origin**: specs/012-knowledge-base plan (promoted to Tier 1 Phase 5)
- **Status**: deferred (pending pilot completion)
- **Date**: 2025-05-10

Add git hooks to enforce knowledge base maintenance. Hooks run via Git Bash on all platforms (Windows, Linux, Mac) using `.githooks/` directory and `git config core.hooksPath .githooks`.

## Design

### Pre-commit hook (non-blocking warning)

1. Get staged files: `git diff --cached --name-only`
2. Check if any match high-risk paths (see below)
3. If high-risk files changed but no `aiwiki/` or `product/` files also staged → print warning
4. Always exit 0 (non-blocking)

### Pre-push hook (blocking)

1. Get commits being pushed: `git log @{push}..HEAD --name-only`
2. Scan commit messages for override tags:
   - `[kb-skip:reason]` → skip checks for that commit, log the reason visibly
   - `[kb-required]` → force checks even for low-risk changes
3. For each commit without `[kb-skip]`:
   a. Check if changed files match high-risk paths (or `[kb-required]` present)
   b. Verify `aiwiki/` and `product/` were also updated in the set of commits being pushed (not necessarily same commit)
   c. If high-risk files changed but neither `aiwiki/` nor `product/` updated across the push → exit 1 with actionable error
4. Mismatch policy: if `aiwiki/` was updated but `product/` behavior docs weren't synced → block push

### High-risk path list (tuned during pilot)

- `app/src/lib/orchestration/**`
- `app/src/lib/stores/**`
- `app/src-tauri/src/**`
- `lcc-rs/src/**`
- `product/architecture/**`
- `product/user-stories/**`

### Implementation constraints

- Pure bash — no PowerShell, Python, or Node dependencies
- Only uses: `git diff`, `git log`, `grep`, `echo`, `test`, `exit`
- Works identically from Git Bash, VS Code terminal, or any shell on any OS
- One-time setup: `git config core.hooksPath .githooks`

## Prior Work

- Separate future plan — not needed until KB is established and proven via pilot
- Risk of false positives if hooks are too aggressive early on
- Pilot evaluation includes false-positive scoring to tune the high-risk path list
- May conflict with existing CI checks; coordinate with frontend-regression-gate workflow
