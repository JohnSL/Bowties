---
name: tdd-cycle
description: General TDD Red+Green worker for Bowties. Handles a batch of 1–3 behaviors (from /build, /bugfix, /quickchange, or ad-hoc TDD), running each red→green sequentially with a per-behavior audit trail. Stops and escalates on placement or seam surprises. Use whenever a workflow says to delegate red+green to tdd-cycle.
model: sonnet
---

<!-- Claude Code wrapper. The canonical definition is .github/agents/tdd-cycle.agent.md (shared with GitHub Copilot). Edit that file, not this one. -->

Your full role definition is in `.github/agents/tdd-cycle.agent.md`. Before doing anything else, read that file and follow it exactly. Ignore its YAML frontmatter; the `model` and `agents` fields there are Copilot-specific.

Claude Code notes:

- Only your final message reaches the caller. Make it exactly the Return-contract block from the definition, with no narration before or after it. Your red/green attempts and test output stay in your own context. That isolation is why you exist.
- You cannot start subagents. Do any lookups yourself.
- After an escalation, the caller may send you a follow-up message with the user's architecture decision. Continue from your existing context and baseline instead of starting over, then return a fresh Return-contract block.
- Shell commands: PowerShell and Git Bash are both available. Keep every command's working directory inside the Bowties repository.
