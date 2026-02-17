# Bowties

**Visual LCC/OpenLCB Configuration Tool for Model Railroads**

Bowties transforms complex LCC (Layout Command Control) event configuration into simple visual workflows. Understand your existing layout at a glance and create event links through intuitive drag-and-drop interactions—no protocol expertise required.

## Quick Start

### Prerequisites

- **Rust** (1.70+): Install via [rustup](https://rustup.rs/)
- **Node.js** (18+): For frontend development
- **LCC Network**: TCP-accessible GridConnect hub (port 12021 or 23)

### Installation

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone repository
git clone <repository-url>
cd Bowties

# Build and run
cd app
npm install
npm run tauri dev
```

### First Launch

1. **Connect** to your GridConnect hub (e.g., `localhost:12021`)
2. **Discover Nodes** to scan your LCC network
3. View discovered nodes with manufacturer info and status

🚧 **Status:** Phase 1 (Foundation) complete. Configuration and Event Bowties views in development.

## Features

### ✅ Current (Phase 1)

- TCP connection to GridConnect hubs
- Node discovery (Verify Node ID protocol)
- SNIP data retrieval (manufacturer, model, versions)
- Responsive node list interface
- Connection state persistence

### 🚧 In Development (Phase 2)

- Miller Columns configuration view
- CDI retrieval and caching
- Configuration value display

### ⏳ Planned

- Event Bowties view (visual event relationships)
- Drag-and-drop event linking
- Real-time Event Monitor
- Configuration editing

See [docs/project/roadmap.md](docs/project/roadmap.md) for complete timeline.

## Documentation

### Getting Started
- **[Quick Start Guide](docs/project/development.md)** *(coming soon)* - Setup and contributing
- **[User Workflows](docs/design/workflows.md)** - Example usage scenarios

### Design & Vision
- **[Product Vision](docs/design/vision.md)** - Long-term goals and UX patterns
- **[Feature Roadmap](docs/project/roadmap.md)** - Development timeline and priorities

### Technical Reference
- **[Current Architecture](docs/technical/architecture.md)** - What's built and how it works
- **[LCC Protocol Reference](docs/technical/protocol-reference.md)** - Protocol essentials
- **[MTI Quick Reference](docs/technical/mti-reference.md)** - Message type indicators
- **[lcc-rs API](docs/technical/lcc-rs-api.md)** - Rust library reference
- **[Tauri API](docs/technical/tauri-api.md)** - Frontend command reference

### Project Governance
- **[Constitution](.specify/memory/constitution.md)** - Development principles and standards

## Architecture

```
Frontend (SvelteKit)     IPC     Backend (Tauri/Rust)     Protocol     LCC Network
─────────────────────   ─────   ──────────────────────   ────────   ─────────────
  +page.svelte          ←──→    connection.rs           ←────→     GridConnect Hub
  NodeList.svelte              discovery.rs                         (TCP:12021)
  Svelte Stores                snip.rs                                    │
                                    ↓                                     │
                               lcc-rs library                             │
                               ├── gridconnect.rs                        │
                               ├── discovery.rs        ←─────────────────┘
                               ├── snip.rs                LCC Protocol
                               └── transport.rs           Messages
```

**Technology Stack:**
- **Frontend:** SvelteKit 2, TypeScript, Custom CSS
- **Backend:** Tauri 2, Rust 2021+
- **Protocol:** lcc-rs (custom Rust library)
- **Runtime:** Tokio (async I/O)

## Development

### Build

```bash
cd app

# Development build with hot-reload
npm run tauri dev

# Production build
npm run tauri build
```

### Testing

```bash
# Rust tests (lcc-rs library)
cd lcc-rs
cargo test

# Rust tests (Tauri backend)
cd app/src-tauri
cargo test

# Frontend tests (coming soon)
cd app
npm test
```

### Project Principles

**From [Constitution](.specify/memory/constitution.md):**

1. **Rust 2021+** - Type safety, memory safety, performance
2. **Test-Driven Development** - Protocol correctness through testing
3. **LCC Protocol Correctness** - 100% standards compliance
4. **UX-First Design** - Accessibility for hobbyists
5. **Event Management Excellence** - Core competency

## Contributing

1. Review [Constitution](.specify/memory/constitution.md) for development principles
2. Check [Roadmap](docs/project/roadmap.md) for current priorities
3. See [Architecture](docs/technical/architecture.md) for implementation details
4. Follow Rust API guidelines and use `cargo fmt` before commits

## License

*(License information TBD)*

## Acknowledgments

- **LCC/OpenLCB** community for protocol specifications
- **Model Railroad** hobbyists for inspiration and feedback

---

**Status:** Active Development | **Phase:** 1 of 5 | **Next Milestone:** Configuration View (Phase 2)
