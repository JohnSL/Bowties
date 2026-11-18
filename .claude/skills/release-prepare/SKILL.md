---
name: release-prepare
description: 'Prepare a Bowties release. Use when choosing the next version, reviewing user docs, updating the release version files, validating the repo, and pushing the release-preparation commit before tagging. Keywords -- release, version bump, semver, docs review, validation, commit.'
argument-hint: 'Optional target version override, for example 0.2.1'
user-invocable: true
---

# Release Prepare

## When to use

Use this skill when you are preparing a Bowties release locally and want to:

- detect the current synced version from the repo
- choose the next version with the ask-questions flow
- review user-facing documentation for release drift
- update the version files and any necessary docs
- run release validation
- create and push the release-preparation commit

Do not use this skill to create or push the release tag. Tagging belongs to `/release-publish`.

## Source of truth

Read `d:\src\github\LCC\Bowties\docs\project\releasing.md` before doing anything else and follow it exactly.

## Required version surfaces

Read and compare these four files:

- `d:\src\github\LCC\Bowties\app\package.json`
- `d:\src\github\LCC\Bowties\app\src-tauri\Cargo.toml`
- `d:\src\github\LCC\Bowties\app\src-tauri\tauri.conf.json`
- `d:\src\github\LCC\Bowties\lcc-rs\Cargo.toml`

If they do not already agree on the current version, stop and report the mismatch.

## Workflow

1. Validate repository state first.
   - Check the current branch.
   - Check for uncommitted or untracked changes.
   - If the worktree is dirty in a way that makes release prep ambiguous, stop and ask the user how to proceed.

2. Detect the current version from the four required version surfaces and report it clearly.

3. Determine the target version.
   - Parse the current version as semantic versioning.
   - Compute suggested patch, minor, and major bumps.
   - If the slash command includes an explicit semantic version, treat it as a candidate override, not an automatic instruction.
   - Use the ask-questions tool to show the detected current version and ask for the target version.
   - Include patch, minor, major, explicit-argument, and custom-version choices when possible.
   - Do not edit files until the user answers.

4. Enforce the documentation gate before changing versions.
   Review these release-facing surfaces:
   - `d:\src\github\LCC\Bowties\README.md`
   - `d:\src\github\LCC\Bowties\docs\user\installing.md`
   - `d:\src\github\LCC\Bowties\docs\user\using.md`
   - `d:\src\github\LCC\Bowties\docs\images\`
   - `d:\src\github\LCC\Bowties\docs\project\releasing.md`

5. Update the version files and any necessary release-doc surfaces using minimal edits.

6. Run release validation.
   - `cargo test` in `d:\src\github\LCC\Bowties\lcc-rs`
   - `cargo test` in `d:\src\github\LCC\Bowties\app\src-tauri`
   - `npm test` in `d:\src\github\LCC\Bowties\app`
   - Also run `npm run test:refactor-gate` in `d:\src\github\LCC\Bowties\app` when the release includes offline layout, sync, discovery, or config-read changes.

7. Show a preview before mutating git state.
   Include:
   - current version
   - chosen target version
   - files changed
   - doc updates made
   - validation results
   - proposed commit message

8. Ask for confirmation before `git commit` and before `git push`.

9. If approved, create and push the release-preparation commit.

10. Stop after the commit is pushed and hand off to `/release-publish`.

## Output rules

- Be explicit about the current version and the selected target version.
- Stop with the smallest specific blocker when the workflow cannot proceed safely.
- Do not create or push the release tag from this skill.
