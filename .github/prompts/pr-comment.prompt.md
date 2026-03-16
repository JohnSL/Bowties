---
name: pr-comment
description: Generate a PR title and comment from changes not yet merged into main. Focuses on user value, not implementation details.
---

First, determine the current branch by running `git branch --show-current`.

- **If the current branch is `main`**: the changes come from a squash merge that has not yet been committed. Use `git diff HEAD` (staged + unstaged) to identify the changes.
- **Otherwise**: run `git log main..HEAD --oneline` and `git diff main...HEAD` to identify all changes on this branch that have not yet been merged into `main`.

Based on those changes, produce:

1. **PR Title** — A concise, imperative-mood title (≤72 characters) that describes what this PR delivers to users.

2. **PR Description** — A markdown comment suitable for pasting into GitHub. Structure it as:

   ## Summary
   One or two sentences describing what this PR does and why, from the user's perspective.

   ## What's new
   A bullet list of user-facing changes. Focus on what users can now do, see, or experience differently. Do not describe how the code works internally.

   ## Notes (optional)
   Any context that reviewers need — e.g. known limitations, follow-up work, or testing steps. Omit this section if there is nothing important to add.

**Rules:**
- Write for a non-technical audience where possible.
- Do not mention file names, function names, variable names, or implementation details.
- Do not include a "Breaking changes" section unless there actually are breaking changes.
- Keep the entire description under 400 words.
