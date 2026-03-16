# bowtie Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-14

## Active Technologies
- Rust 2021 edition (backend), TypeScript 5.6 (frontend) (001-node-snip-data)
- In-memory for this feature (future: SQLite for CDI cache) (001-node-snip-data)
- Rust 2021+ (backend), TypeScript 5.x with SvelteKit 2.x (frontend) + Tauri 2.x, lcc-rs (protocol library), tokio (async runtime) (001-cdi-xml-viewer)
- In-memory CDI data cache (already retrieved by node discovery/management) (001-cdi-xml-viewer)
- TypeScript 5.x (frontend), Rust 2021+ edition (backend via Tauri 2) + SvelteKit 2.x, Tauri 2.x, lcc-rs (existing LCC protocol library), Tauri events (003-miller-columns)
- In-memory CDI cache (already retrieved by Feature F2 - CDI Caching dependency) (003-miller-columns)
- Rust 2021 (backend via lcc-rs), TypeScript 5.x (frontend) + lcc-rs (LCC protocol), Tauri 2 (desktop framework), SvelteKit (reactive UI) (004-read-node-config)
- In-memory cache (Map<NodeId, Map<ElementPath, TypedValue>>), no persistence (004-read-node-config)
- Rust 2021 (backend, `app/src-tauri/src/`), TypeScript 5.x strict mode (frontend, `app/src/`) + SvelteKit 2.x, Tauri 2.x, lcc-rs (internal library), tokio, serde, uuid (005-config-sidebar-view)
- In-memory config value cache (`millerColumns.ts` Svelte store, `ConfigValueMap`); backend CDI parse cache (`CDI_PARSE_CACHE` lazy_static `Arc<RwLock<HashMap<String, lcc_rs::cdi::Cdi>>>`) (005-config-sidebar-view)
- Rust 2021 (stable ≥1.70) + TypeScript strict / SvelteKit 2.x + `lcc-rs` (workspace crate), `tokio`, `serde`, `tauri 2.x`; SvelteKit 2.x + Tauri JS API (006-bowties-event-discovery)
- In-memory only — `AppState.nodes` cache (already exists); no new persistence in this phase (006-bowties-event-discovery)
- Rust 2021 edition (stable 1.70+), TypeScript (strict), SvelteKit 2.x + lcc-rs (path dep), Tauri 2, tokio 1.41, serde 1.0, roxmltree 0.20, thiserror (2.0 in lcc-rs, 1.0 in app) (007-edit-node-config)
- N/A (values written directly to LCC node memory via protocol) (007-edit-node-config)
- N/A — this phase is prompt engineering and document production, not code + Copilot Chat (or any capable LLM), `pdf-utilities` MCP extension for PDF text extraction (008-guided-configuration)
- Output files in `specs/008-guided-configuration/` as structured markdown/JSON (008-guided-configuration)
- Rust 2021 (stable 1.75+) — backend; TypeScript 5.x / SvelteKit 2.x / Svelte 5 — frontend (008-guided-configuration)
- YAML files on disk (`.profile.yaml`). Two discovery paths: (008-guided-configuration)
- Rust 2021 (stable 1.70+) backend; TypeScript 5.6 / Svelte 5 / SvelteKit 2.9 frontend + Tauri 2, tokio 1.41, serde_yaml_ng 0.10, lcc-rs (workspace crate), TailwindCSS 4.2 (009-editable-bowties)
- User-managed YAML layout file (serde_yaml_ng); in-memory bowtie catalog (AppState); pending edits (frontend Svelte store) (009-editable-bowties)

- Python 3.12 (latest stable as of 2026), managed via UV + PySerial (serial port communication), IntelHex (firmware loading), UV (Python version management) (001-python3-migration)

## Project Structure

```text
src/
tests/
```

## Commands

cd src; pytest; ruff check .

## Code Style

Python 3.12 (latest stable as of 2026), managed via UV: Follow standard conventions

## Recent Changes
- 009-editable-bowties: Added Rust 2021 (stable 1.70+) backend; TypeScript 5.6 / Svelte 5 / SvelteKit 2.9 frontend + Tauri 2, tokio 1.41, serde_yaml_ng 0.10, lcc-rs (workspace crate), TailwindCSS 4.2
- 008-guided-configuration: Added Rust 2021 (stable 1.75+) — backend; TypeScript 5.x / SvelteKit 2.x / Svelte 5 — frontend
- 007-edit-node-config: Added Rust 2021 edition (stable 1.70+), TypeScript (strict), SvelteKit 2.x + lcc-rs (path dep), Tauri 2, tokio 1.41, serde 1.0, roxmltree 0.20, thiserror (2.0 in lcc-rs, 1.0 in app)


<!-- MANUAL ADDITIONS START -->
## LCC/OpenLCB Protocol Implementation Reference

When implementing any LCC/OpenLCB protocol feature, consult the `OpenLCB_Java` and `JMRI` folders in this workspace as the authoritative reference implementations. These are the most widely used implementations and represent the community standard for correct protocol behavior.

- `OpenLCB_Java/` — core OpenLCB/LCC protocol library
- `JMRI/` — production application built on OpenLCB_Java; contains extensive real-world usage examples
<!-- MANUAL ADDITIONS END -->
