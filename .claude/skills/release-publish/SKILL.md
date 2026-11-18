---
name: release-publish
description: 'Publish a Bowties release. Use when deriving the release tag from the current synced version files, creating and pushing that tag, and drafting end-user release notes markdown to paste into the GitHub draft release. Keywords -- release, tag, publish, release notes, markdown, end user.'
argument-hint: 'Optional release notes emphasis or reminder text'
user-invocable: true
---

# Release Publish

## When to use

Use this skill after `/release-prepare` has already pushed the release-preparation commit and you are ready to:

- confirm the current synced release version from the repo
- derive the release tag from that version
- create and push the tag
- draft end-user release notes markdown for the GitHub draft release

This skill should not change the version files. It publishes the already-prepared release state.

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

1. Validate repository state up front.
   - Confirm the worktree is clean enough for tagging.
   - Confirm the current version files are synchronized.

2. Derive the release tag from the current synced version.
   - If the current version is `X.Y.Z`, the release tag is `vX.Y.Z`.
   - Do not invent or infer a different release tag from memory.

3. Check whether the target tag already exists locally or remotely.
   - If it already exists, stop and report that blocker.

4. Show a tagging preview.
   Include:
   - current synced version
   - derived tag name
   - exact `git tag` command
   - exact `git push origin <tag>` command

5. Ask for confirmation before `git tag` and again before `git push origin <tag>`.

6. If approved, create and push the tag.

7. Gather release context for the notes.
   - Identify the previous release tag.
   - Summarize the user-visible changes since that tag.
   - Review `d:\src\github\LCC\Bowties\README.md`, `d:\src\github\LCC\Bowties\docs\user\installing.md`, and `d:\src\github\LCC\Bowties\docs\user\using.md` for user-facing wording that should inform the notes.

8. Produce markdown for the GitHub draft release.
   Use this structure when relevant:
   - `## Summary`
   - `## What's new`
   - `## Install and compatibility notes`
   - `## Documentation`
   - `## Notes`

9. Keep the release notes user-facing.
   - Focus on what users can now do, see, or install.
   - Do not mention file names, function names, variable names, refactors, or internal implementation details.
   - Omit empty sections.

10. Stop after producing the markdown and tell the user to paste it into the GitHub draft release once Actions has attached the artifacts.

## Output rules

- Do not pretend the GitHub release was published.
- Keep the notes concise and suitable for end users.
- If tagging cannot proceed safely, stop before any git mutation.
