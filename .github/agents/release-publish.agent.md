---
description: Publish a Bowties draft release by finding the release tag, checking release notes and docs status, and either using GitHub CLI or handing off a precise GitHub UI checklist.
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding.

## Scope

This agent handles the public release publication step after the tag has already been pushed and the GitHub Actions release workflow has created a draft release.

## Required files

- `d:\src\github\LCC\Bowties\docs\project\releasing.md`
- `d:\src\github\LCC\Bowties\README.md`
- `d:\src\github\LCC\Bowties\docs\user\installing.md`
- `d:\src\github\LCC\Bowties\docs\user\using.md`

## Workflow

1. Read `d:\src\github\LCC\Bowties\docs\project\releasing.md` before doing anything else. Treat it as the source of truth for the release publication workflow.

2. Determine which release to publish.
   - If `$ARGUMENTS` contains a version or tag, normalize it to both forms: `X.Y.Z` and `vX.Y.Z`.
   - Otherwise inspect recent tags and ask the user which release should be published.
   - If the target tag is ambiguous, use the ask-questions tool to resolve it before continuing.

3. Gather release context from git.
   - Identify the previous release tag.
   - Summarize the commits and user-visible changes between the previous tag and the target tag.
   - Focus on user value, install/support changes, and documentation-visible changes rather than implementation details.

4. Re-check the documentation status for the target release.
   - Review `README.md`, `docs/user/installing.md`, `docs/user/using.md`, and any release-relevant screenshots or release-process docs changed for the release.
   - If docs appear stale relative to the shipped changes, stop and tell the user exactly what should be updated before publication.

5. Draft release notes.
   Structure them as:
   - `## Summary`
   - `## What's new`
   - `## Install and compatibility notes` when relevant
   - `## Documentation` when docs changed or when there are user-facing docs to call out
   - `## Notes` only when important limitations or follow-up context matter

6. Check whether GitHub CLI is available and authenticated.
   - If `gh` is available and authenticated, inspect the draft release for the target tag and verify that artifacts are present.
   - If `gh` is unavailable or unauthenticated, prepare a manual GitHub UI handoff instead of trying to publish.

7. Show a publication preview before taking action.
   - Include the target tag, release note draft, docs-status summary, and whether publication will happen through GitHub CLI or through a manual handoff.

8. Ask for final confirmation before changing the release state.
   - If using `gh`, only publish after the user explicitly approves the final release notes and latest-release action.
   - If not using `gh`, stop after providing:
     - the final release notes text
     - a concise GitHub UI checklist for publishing the draft release and making it latest

## Output requirements

- Keep release notes user-facing and concise.
- If `gh` cannot be used, do not pretend publication happened.
- If release artifacts are missing or the draft release does not exist yet, stop with that blocker clearly stated.
