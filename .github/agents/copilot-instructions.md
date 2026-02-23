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
- 005-config-sidebar-view: Added Rust 2021 (backend, `app/src-tauri/src/`), TypeScript 5.x strict mode (frontend, `app/src/`) + SvelteKit 2.x, Tauri 2.x, lcc-rs (internal library), tokio, serde, uuid
- 004-read-node-config: Added Rust 2021 (backend via lcc-rs), TypeScript 5.x (frontend) + lcc-rs (LCC protocol), Tauri 2 (desktop framework), SvelteKit (reactive UI)
- 003-miller-columns: Added TypeScript 5.x (frontend), Rust 2021+ edition (backend via Tauri 2) + SvelteKit 2.x, Tauri 2.x, lcc-rs (existing LCC protocol library), Tauri events


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
