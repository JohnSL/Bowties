---
description: "Execute the implementation planning workflow using the plan template to generate design artifacts."
argument-hint: "<feature description or notes>"
---

<!-- Claude Code wrapper. Canonical sources: .github/agents/speckit.plan.agent.md and .github/prompts/speckit.plan.prompt.md (shared with GitHub Copilot). Edit those, not this file. -->

1. Read `.github/agents/speckit.plan.agent.md` and follow it exactly, ignoring its YAML frontmatter.

Claude Code notes:
- Where the instructions show a "User Input" block with an arguments placeholder, use the user input at the end of this command.
- `handoffs` in the frontmatter are Copilot UI buttons. When you finish, suggest the matching next `/speckit.*` command instead.
- When the instructions run `update-agent-context.ps1`, keep `-AgentType copilot` exactly as written. Do **not** switch it to `claude` or omit it: that would rewrite `CLAUDE.md`.

User input:

$ARGUMENTS
