# Profile-extraction shared tooling

Shared Python used by the `profile-*` skills. One module, one CLI:

- `cdi_registry.py` — parses a node's CDI XML into a `CdiNode` tree and
  resolves the path notations the extraction files use (literal `/`
  inside element names, `[N]` / `[N-M]` index disambiguators, and
  `<repname>` collapse for replicated groups).
- `profile_tools.py` — single CLI with subcommands the skills invoke.

## Why a script and not just prose in each skill?

Two reasons:

1. **CDI path notation is fiddly and easy to get wrong by inspection.**
   Same-named sibling groups (e.g. `Action` and `Action` under
   `Conditionals/Logic` in Signal-LCC) and literal `/` inside element
   names (e.g. `Commands/Consumers`) are real and recurring. Centralising
   the parser means every skill resolves them the same way.
2. **Several skills must enumerate "every X in the CDI" exhaustively.**
   `profile-3` (every segment and group), `profile-4` (every leaf field
   and enum option), `profile-1` (every group containing eventids).
   Pre-emitting a skeleton from the CDI makes the *coverage* guarantee
   mechanical instead of vigilance-based.

## Running

The recommended runner is [uv](https://docs.astral.sh/uv/). Each script
declares its Python version and dependencies inline via [PEP 723][pep723],
so `uv run` creates and caches an ephemeral venv automatically — no
`requirements.txt` and no global `pip install` needed.

```pwsh
uv run .github/skills/_lib/profile_tools.py <subcommand> [args]
```

If `uv` is not installed (`winget install astral-sh.uv` on Windows,
`pipx install uv` on macOS/Linux), fall back to a regular Python ≥ 3.11
with PyYAML installed:

```pwsh
pip install pyyaml
python .github/skills/_lib/profile_tools.py <subcommand> [args]
```

## Conventions

Every subcommand takes a *node directory* (e.g. `profile-extractions/signal-lcc`)
that contains a `manual-outline.json` whose `cdiFile` field points at
the CDI XML. The script reads the CDI from there — no other paths need
to be passed.

## Subcommands

| Command | Used by skill | Purpose |
| --- | --- | --- |
| `validate <node-dir>` | profile-6 | Cross-check every extraction file against the CDI registry; write `validation-report.json`; exit non-zero on failure. |
| `assemble <node-dir>` | profile-7 | Combine `event-roles.json` + `relevance-rules.json` into `<Mfr>_<Model>.profile.yaml`, converting `[N-M]` paths to `#N` ordinal notation. |
| `skeleton sections <node-dir>` | profile-3 | Emit `section-descriptions.skeleton.yaml` with one entry per segment and group; LLM fills `description` + `citation`. |
| `skeleton fields <node-dir>` | profile-4 | Emit `field-descriptions.skeleton.yaml` with one entry per leaf field (including every enum option) and one per eventid; LLM fills descriptions. |
| `skeleton events <node-dir>` | profile-1 | Emit `event-roles.skeleton.json` with one entry per group containing eventids; LLM fills `role` / `citation` / `confidence`. |
| `enum-fields <node-dir>` | profile-2 | Print every enum field with its `<map>` so the LLM can spot relevance-rule candidates (`value 0 = None/Disabled`, mutually exclusive modes, …). |
| `check <node-dir> <cdiPath> [--value N]` | any | Ad-hoc lookup: resolve a path, print its kind/children/enum map, optionally validate one enum value. |

## Output locations

| Subcommand | Writes to |
| --- | --- |
| `validate` | `<node-dir>/validation-report.json` |
| `assemble` | `<node-dir>/<Mfr>_<Model>.profile.yaml` |
| `skeleton sections` | `<node-dir>/section-descriptions.skeleton.yaml` |
| `skeleton fields` | `<node-dir>/field-descriptions.skeleton.yaml` |
| `skeleton events` | `<node-dir>/event-roles.skeleton.json` |
| `enum-fields` / `check` | stdout |

Skeletons write to `*.skeleton.*` filenames on purpose: each profile-1/3/4
skill renames its skeleton to the canonical name (`event-roles.json`,
`section-descriptions.yaml`, `field-descriptions.yaml`) once the LLM has
filled in the TODO placeholders.

[pep723]: https://peps.python.org/pep-0723/
