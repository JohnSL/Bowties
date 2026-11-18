---
description: "Convert existing tasks into actionable, dependency-ordered GitHub issues for the feature based on available design artifacts."
argument-hint: "<feature description or notes>"
---

<!-- Claude Code wrapper. Canonical sources: .github/agents/speckit.taskstoissues.agent.md and .github/prompts/speckit.taskstoissues.prompt.md (shared with GitHub Copilot). Edit those, not this file. -->

1. Read `.github/agents/speckit.taskstoissues.agent.md` and follow it exactly, ignoring its YAML frontmatter.

Claude Code notes:
- Where the instructions show a "User Input" block with an arguments placeholder, use the user input at the end of this command.
- `handoffs` in the frontmatter are Copilot UI buttons. When you finish, suggest the matching next `/speckit.*` command instead.
- Create issues with `gh issue create --repo JohnSL/Bowties` instead of the GitHub MCP `issue_write` tool. The Issue Capture Protocol still applies: propose the issues and wait for explicit confirmation before creating any.

User input:

$ARGUMENTS
