---
description: "Generate an actionable, dependency-ordered tasks.md for the feature based on available design artifacts."
argument-hint: "<feature description or notes>"
---

<!-- Claude Code wrapper. Canonical sources: .github/agents/speckit.tasks.agent.md and .github/prompts/speckit.tasks.prompt.md (shared with GitHub Copilot). Edit those, not this file. -->

1. Read `.github/agents/speckit.tasks.agent.md` and follow it exactly, ignoring its YAML frontmatter.

Claude Code notes:
- Where the instructions show a "User Input" block with an arguments placeholder, use the user input at the end of this command.
- `handoffs` in the frontmatter are Copilot UI buttons. When you finish, suggest the matching next `/speckit.*` command instead.

User input:

$ARGUMENTS
