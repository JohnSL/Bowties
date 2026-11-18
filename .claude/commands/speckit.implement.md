---
description: "Execute the implementation plan by processing and executing all tasks defined in tasks.md"
argument-hint: "<feature description or notes>"
---

<!-- Claude Code wrapper. Canonical sources: .github/agents/speckit.implement.agent.md and .github/prompts/speckit.implement.prompt.md (shared with GitHub Copilot). Edit those, not this file. -->

1. Read `.github/agents/speckit.implement.agent.md` and follow it exactly, ignoring its YAML frontmatter.
2. Then read `.github/prompts/speckit.implement.prompt.md` and apply its Bowties-specific additions (ignore its `agent:` frontmatter).

Claude Code notes:
- Where the instructions show a "User Input" block with an arguments placeholder, use the user input at the end of this command.
- `handoffs` in the frontmatter are Copilot UI buttons. When you finish, suggest the matching next `/speckit.*` command instead.

User input:

$ARGUMENTS
