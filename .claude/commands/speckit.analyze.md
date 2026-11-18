---
description: "Perform a non-destructive cross-artifact consistency and quality analysis across spec.md, plan.md, and tasks.md after task generation."
argument-hint: "<feature description or notes>"
---

<!-- Claude Code wrapper. Canonical sources: .github/agents/speckit.analyze.agent.md and .github/prompts/speckit.analyze.prompt.md (shared with GitHub Copilot). Edit those, not this file. -->

1. Read `.github/agents/speckit.analyze.agent.md` and follow it exactly, ignoring its YAML frontmatter.
2. Then read `.github/prompts/speckit.analyze.prompt.md` and apply its Bowties-specific additions (ignore its `agent:` frontmatter).

Claude Code notes:
- Where the instructions show a "User Input" block with an arguments placeholder, use the user input at the end of this command.
- `handoffs` in the frontmatter are Copilot UI buttons. When you finish, suggest the matching next `/speckit.*` command instead.

User input:

$ARGUMENTS
