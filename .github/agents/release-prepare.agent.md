---
description: Prepare a Bowties release by reading the current synced version, asking for the next version, reviewing user docs, updating release files, validating, and pausing before push and tag operations.
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding.

## Scope

This agent prepares a Bowties release locally. It should not publish the GitHub release itself.

## Required files

- `d:\src\github\LCC\Bowties\docs\project\releasing.md`
- `d:\src\github\LCC\Bowties\README.md`
- `d:\src\github\LCC\Bowties\docs\user\installing.md`
- `d:\src\github\LCC\Bowties\docs\user\using.md`
- `d:\src\github\LCC\Bowties\app\package.json`
- `d:\src\github\LCC\Bowties\app\src-tauri\Cargo.toml`
- `d:\src\github\LCC\Bowties\app\src-tauri\tauri.conf.json`
- `d:\src\github\LCC\Bowties\lcc-rs\Cargo.toml`

## Workflow

1. Read `d:\src\github\LCC\Bowties\docs\project\releasing.md` before doing anything else. Treat it as the source of truth for the release workflow.

2. Validate repository state up front.
   - Check the current branch.
   - Check for uncommitted or untracked changes.
   - Check whether the target tag already exists locally or remotely if a candidate version is already known.
   - If the worktree is dirty in a way that makes the release state ambiguous, stop and ask the user how to proceed.

3. Detect the current Bowties version from all four release-version surfaces:
   - `d:\src\github\LCC\Bowties\app\package.json`
   - `d:\src\github\LCC\Bowties\app\src-tauri\Cargo.toml`
   - `d:\src\github\LCC\Bowties\app\src-tauri\tauri.conf.json`
   - `d:\src\github\LCC\Bowties\lcc-rs\Cargo.toml`
   Report the discovered values clearly.

4. Fail fast if the current version is not already synchronized across those four files. Do not guess which one is authoritative.

5. Determine the target version.
   - Parse the current version as semantic versioning.
   - Compute the suggested patch, minor, and major bumps.
   - If `$ARGUMENTS` contains an explicit semantic version, treat it as a candidate override, not as an automatic instruction.
   - Use the ask-questions tool to ask for the target version. The question must show the detected current version and include these choices when possible:
     - use the suggested patch version
     - use the suggested minor version
     - use the suggested major version
     - use the explicit version from `$ARGUMENTS` if present
     - enter a custom version
   - Allow freeform input so the user can type a custom version.
   - Do not edit files until the user has answered.

6. Confirm the documentation gate before changing versions.
   - Review these user-facing surfaces:
     - `d:\src\github\LCC\Bowties\README.md`
     - `d:\src\github\LCC\Bowties\docs\user\installing.md`
     - `d:\src\github\LCC\Bowties\docs\user\using.md`
     - `d:\src\github\LCC\Bowties\docs\images\`
     - `d:\src\github\LCC\Bowties\docs\project\releasing.md`
   - Inspect recent release-scope changes and identify likely doc drift such as installer names, supported hardware, screenshots, workflow wording, or platform availability.
   - Use the ask-questions tool to confirm which doc surfaces need updates for this release.
   - If documentation appears stale, update it as part of the same release-preparation work before committing.

7. Update the four version files to the approved target version using minimal edits.

8. Run release validation before any commit or tag operation.
   - Run `cargo test` in `d:\src\github\LCC\Bowties\lcc-rs`.
   - Run `cargo test` in `d:\src\github\LCC\Bowties\app\src-tauri`.
   - Run `npm test` in `d:\src\github\LCC\Bowties\app`.
   - If the release touches offline layout, sync, discovery, or config-read behavior, also run `npm run test:refactor-gate` in `d:\src\github\LCC\Bowties\app`.
   - If validation fails, stop before any git push or tag push.

9. Present a preview before mutating git state.
   - Summarize the detected current version, chosen target version, changed files, docs updates, and validation results.
   - Show the exact commit message you intend to use.
   - Show the exact tag name you intend to create.

10. Ask for explicit confirmation before each irreversible stage.
   - First confirmation: before `git commit`.
   - Second confirmation: before `git push`.
   - Third confirmation: before `git tag` and `git push origin <tag>`.

11. If approved, perform the commit, push, tag creation, and tag push. Stop after the tag is pushed and hand off to the separate publish workflow.

## Output requirements

- Be explicit about the current version and the chosen target version.
- When stopping for confirmation, summarize the pending action in one short list.
- If you cannot safely continue, stop with the smallest specific blocker.
