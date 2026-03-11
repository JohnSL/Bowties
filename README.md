# Bowties

**Visual LCC/OpenLCB Configuration Tool for Model Railroads**

Bowties transforms complex LCC (Layout Command Control) event configuration into simple visual workflows. Understand your existing layout at a glance and create event links through intuitive drag-and-drop interactions—no protocol expertise required.

## Quick Start

### Prerequisites

- **Rust** (1.70+): Install via [rustup](https://rustup.rs/)
- **Node.js** (20+): For frontend development
- **LCC Network**: one of:
  - TCP hub: JMRI, standalone GridConnect bridge (port 12021 or 23)
  - USB-to-CAN (GridConnect): SPROG CANISB, SPROG USB-LCC, RR-Cirkits Buffer LCC, CAN2USBINO
  - USB-to-CAN (SLCAN): Canable, Lawicel CANUSB, any slcand-compatible adapter

### Installation

**macOS / Linux:**
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

**Windows (PowerShell):**
```powershell
# Install Rust toolchain (via winget, or download from https://rustup.rs)
winget install Rustlang.Rustup

# Clone repository
git clone <repository-url>
cd Bowties

# Build and run
cd app
npm install
npm run tauri dev
```

### First Launch

**Via USB adapter** (GridConnect or SLCAN):
1. Plug in your USB-to-CAN adapter
2. Click **Add connection**, select **GridConnect (USB/Serial)** or **SLCAN (USB/Serial)**
3. Choose the COM port and click **Connect**
4. **Discover Nodes** to scan your LCC network

**Via TCP hub** (JMRI or standalone bridge):
1. Click **Add connection**, select **TCP**
2. Enter host and port (e.g., `localhost:12021`) and click **Connect**
3. **Discover Nodes** to scan your LCC network

After connecting either way: view discovered nodes with manufacturer info and status.

✅ **Status:** Configuration viewing/editing and read-only Bowties view complete. Drag-and-drop event linking in development.

## Features

### ✅ Current

- TCP connection to GridConnect hubs (JMRI, standalone bridges)
- Direct USB-to-CAN adapter support (GridConnect Serial and SLCAN protocols)
- Supported USB adapters: SPROG CANISB, SPROG USB-LCC, RR-Cirkits Buffer LCC, CAN2USBINO, Canable, Lawicel CANUSB
- Node discovery (Verify Node ID protocol)
- SNIP data retrieval (manufacturer, model, versions)
- CDI retrieval and disk caching
- Configuration viewing and editing (read/write configuration memory)
- Read-only Bowties view (event relationship map across all nodes)
- Real-time traffic monitor
- Responsive node list interface
- Connection state persistence

### ⏳ Planned

- Drag-and-drop event linking (create new producer ↔ consumer pairs)

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

Licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

## Acknowledgments

- **LCC/OpenLCB** community for protocol specifications
- **Model Railroad** hobbyists for inspiration and feedback

---

**Status:** Active Development | **Phase:** 1 of 5 | **Next Milestone:** Configuration View (Phase 2)
