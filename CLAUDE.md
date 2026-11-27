# Bowties — Claude Code

The always-on project instructions are shared with GitHub Copilot and live in `.github/copilot-instructions.md`:

@.github/copilot-instructions.md

## Claude Code adaptations

The instructions above, and the shared prompts, skills, and agent definitions, were written for GitHub Copilot. Where they name a Copilot-specific feature, use this mapping:

| Copilot | Claude Code |
|---|---|
| `.github/instructions/*.instructions.md` (`applyTo`) | Loaded through the stubs in `.claude/rules/` (`paths:` frontmatter). Edit the `.github/instructions/` file, not the stub. |
| Skills | `.claude/skills/`, shared by both tools. |
| `.github/prompts/*.prompt.md` (`/bugfix`, `/quickchange`, `/speckit.*`, ...) | Wrapper commands in `.claude/commands/` that read the canonical prompt. |
| Agents `change-analyze`, `tdd-cycle`, `tdd-refactor` | Subagents in `.claude/agents/` that read their canonical `.github/agents/*.agent.md`. |
| Model pins: GPT-6 Luna / GPT-6.1 Terra / GPT-6.1 Sol | `haiku` for Explore-style retrieval, `sonnet` for `tdd-cycle` and `tdd-refactor`, `opus` for `change-analyze`. |
| `Explore` subagent | The built-in `Explore` agent (main conversation only). |
| `manage_todo_list` | `TodoWrite` |
| `vscode_askQuestions` | `AskUserQuestion`. Skills that say to present decisions in a single chat message still mean a plain message. |
| GitHub MCP tools | The `gh` CLI. The Issue Capture Protocol still applies: propose, then wait for confirmation. |
| VS Code Stop hook (enrichment gate) | `.claude/settings.json` runs the same `enrichment-gate.ps1` with `-Format claude`. |

### Subagents cannot start subagents

- **`tdd-build` runs in the main conversation, not as a subagent.** When `/build` (or any workflow) says to delegate a slice to `tdd-build`, read `.github/agents/tdd-build.agent.md` and act as the coordinator yourself. Follow its batching rules, its loop, its per-batch baseline and expected-surface verification, and its slice-report contract. Invoke `tdd-cycle` and `tdd-refactor` as subagents. Skip its memory-pruning step: the worker summaries are already the compact form.
- **Red/green dead ends stay in the workers.** Work only from their Return-contract blocks. Do not re-run or replay a worker's investigation in the main conversation unless verification fails. Record any `Dead ends:` entries that carry design information as candidates for `aiwiki/` enrichment.
- **Escalations resume the same worker.** When `tdd-cycle` or `tdd-refactor` escalates, run `change-analyze` and present the options to the user. After the user decides, send the decision to the *same* worker with `SendMessage` so it keeps its context and baseline. Start a fresh worker only if the decision invalidates its batch.
- `change-analyze` cannot start `Explore` subagents. It does that retrieval itself, which is still isolated from the main conversation.

### Other differences

- **`/memories/session/`** does not exist here. Keep per-conversation caches (such as build pre-implementation checks) in the conversation. Durable progress still goes in `specs/<feature>/slices.md`.
- **Reference repositories.** `JMRI/`, `OpenLCB_Java/`, and other additional working directories are read/search-only, as the execution boundary above says. Never edit them or run tooling in them.
- **Spec Kit.** `/speckit.plan` runs `update-agent-context.ps1 -AgentType copilot`. Keep it that way: running it with `claude`, or with no agent type, rewrites this file.
