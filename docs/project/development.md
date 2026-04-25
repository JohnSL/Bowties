# Developer Guide

This document covers building, testing, architecture, and contributing to Bowties.

## Prerequisites

- **Rust** (1.70+): Install via [rustup](https://rustup.rs/)
  - Windows: `winget install Rustlang.Rustup`
  - macOS/Linux: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Node.js** (20+): [nodejs.org](https://nodejs.org/)

## Building from source

```bash
# Clone the repository
git clone https://github.com/JohnSL/Bowties.git
cd Bowties

# Install frontend dependencies
cd app
npm install

# Development build with hot-reload
npm run tauri dev

# Production build
npm run tauri build
```

## Testing

```bash
# lcc-rs protocol library
cd lcc-rs
cargo test

# Tauri backend
cd app/src-tauri
cargo test

# Frontend
cd app
npm test

# Offline/sync/discovery regression gate
npm run test:refactor-gate
```

For changes in the offline layout, sync, discovery, or config-read workflow, run `npm run test:refactor-gate` before opening a PR. Add any narrower route/store/component tests needed for the slice you touched. Use `npm run check` selectively while the broader frontend typecheck baseline is still being cleaned up.

## Architecture

```
Frontend (SvelteKit)     IPC     Backend (Tauri/Rust)     Protocol     LCC Network
─────────────────────   ─────   ──────────────────────   ────────   ─────────────
  +page.svelte          ←──→    connection.rs           ←────→     TCP Hub
  NodeList.svelte              discovery.rs                         (port 12021)
  Svelte Stores                snip.rs                                    │
  ConnectionManager            │                                          │
                               lcc-rs library           ←────→     USB/Serial Adapter
                               ├── gridconnect.rs                   (GridConnect or SLCAN)
                               ├── discovery.rs        ←─────────────────┘
                               ├── snip.rs                LCC Protocol
                               └── transport.rs           Messages
```

**Technology stack:**

| Layer | Technology |
|-------|-----------|
| Frontend | SvelteKit 2, TypeScript, Custom CSS |
| Backend | Tauri 2, Rust 2021+ |
| Protocol | lcc-rs (custom Rust library) |
| Async runtime | Tokio |

### Frontend architecture pattern

Use `Container + Reactive Store + Pure Domain Logic` for offline/sync/discovery work.

- Route and feature-shell components wire state to the UI and emit user intent.
- Stores own durable state and deterministic state transitions.
- Orchestrators own sequencing across async calls, cache refreshes, and lifecycle transitions.
- Pure helpers own rules that can be tested without rendering.

If a `.svelte` file starts deciding lifecycle branches or coordinating multiple backend calls, that logic should usually move into an orchestrator or store before more features land on top of it.

## Project principles

From the [Constitution](.specify/memory/constitution.md):

1. **Rust 2021+** — Type safety, memory safety, performance
2. **Test-Driven Development** — Protocol correctness through testing
3. **LCC Protocol Correctness** — 100% standards compliance
4. **UX-First Design** — Accessibility for hobbyists
5. **Event Management Excellence** — Core competency

## Contributing

1. Review the [Constitution](.specify/memory/constitution.md) for development principles.
2. Check the [Roadmap](roadmap.md) for current priorities.
3. See [Architecture](../technical/architecture.md) for implementation details.
4. Follow Rust API guidelines and run `cargo fmt` before committing.

## Releasing

See [releasing.md](releasing.md) for the full release process (version number locations, tagging, and GitHub Actions workflow).

## Documentation index

| Document | Description |
|----------|-------------|
| [Architecture](../technical/architecture.md) | System design and component overview |
| [LCC Protocol Reference](../technical/protocol-reference.md) | Protocol essentials |
| [MTI Quick Reference](../technical/mti-reference.md) | Message type indicators |
| [lcc-rs API](../technical/lcc-rs-api.md) | Rust library reference |
| [Tauri API](../technical/tauri-api.md) | Frontend IPC command reference |
| [Feature Roadmap](roadmap.md) | Development timeline and priorities |
| [Constitution](../../.specify/memory/constitution.md) | Development principles and standards |

## Acknowledgments

- **LCC/OpenLCB** community for protocol specifications
- **Model Railroad** hobbyists for inspiration and feedback
