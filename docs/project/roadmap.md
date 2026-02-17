# Feature Roadmap

*This document tracks feature prioritization and development phases. See [docs/technical/architecture.md](../technical/architecture.md) for current implementation status.*

## Phase 1: Foundation ✅ (Completed)

**Timeline:** Completed 2026-02-16

**Goal:** Establish protocol foundation and basic desktop application

**Completed Features:**
- ✅ lcc-rs library structure
  - GridConnect frame parsing
  - MTI encoding/decoding
  - TCP transport with tokio
  - Node discovery protocol
  - SNIP datagram protocol
- ✅ Tauri desktop application setup
  - SvelteKit frontend
  - Tauri 2 backend
  - Type-safe IPC commands
- ✅ Connection management
  - TCP connection to GridConnect hub
  - Connection state persistence
  - Connection status UI
- ✅ Node discovery MVP
  - Verify Node ID protocol
  - SNIP data retrieval (batch)
  - Compact discovery UI
  - Node list table with status indicators

**Testing Status:**
- Unit tests for frame parsing
- Integration tests with test data
- Manual testing with real LCC hardware

---

## Phase 2: Configuration View (Next: Weeks 1-4)

**Goal:** Implement Miller Columns navigation and CDI display

### F2: CDI Caching and Retrieval (Priority: P1)

**Description:** Retrieve and cache CDI XML documents from nodes

**Requirements:**
- Implement Memory Configuration protocol (space 0xFF read)
- Parse multi-frame datagram responses
- Cache CDI by `{manufacturer}_{model}_{software_version}`
- Store in platform-specific app data directory
- Background prefetch for discovered nodes

**Tasks:**
- [ ] Memory read protocol in lcc-rs
- [ ] Datagram multi-frame assembly (expand existing)
- [ ] CDI cache manager (filesystem storage)
- [ ] Tauri command for CDI retrieval
- [ ] Loading states in UI

**Acceptance Criteria:**
- CDI retrieved within 5-30 seconds
- Cache hit returns CDI <100ms
- Graceful handling of missing/malformed CDI
- User can manually clear cache (settings)

### F4: Miller Columns Configuration View (Priority: P1)

**Description:** Five-column navigation through node configuration hierarchy

**Requirements:**
- Column 1: Nodes list
- Column 2: Segments (from CDI)
- Column 3: Groups (with replication support)
- Column 4: Elements (with status indicators)
- Column 5: Configuration panel (read-only initially)

**Tasks:**
- [ ] CDI XML parser (extract segments, groups, elements)
- [ ] Miller Columns component (SvelteKit)
- [ ] Navigation state management
- [ ] Element status indicators (✓ ⚠️ ○ 🔧)
- [ ] Configuration panel component (display only)

**Acceptance Criteria:**
- Navigate 5-level hierarchy smoothly
- CDI structure displayed correctly
- Status indicators reflect configuration state
- Context visible (breadcrumb / column selection)

### F3: Configuration Value Retrieval (Priority: P1)

**Description:** Read actual configuration values from nodes

**Requirements:**
- Read Event IDs from configuration memory (space 0xFD)
- Read user-assigned names and descriptions
- Read element-specific parameters
- Display in Configuration panel

**Tasks:**
- [ ] Memory Configuration read protocol
- [ ] Address mapping from CDI to memory
- [ ] Configuration value cache (session-based)
- [ ] Display in Column 5 panel

**Acceptance Criteria:**
- Values retrieved <500ms per element
- Event IDs displayed in dotted hex format
- User names/descriptions shown fallback to CDI names
- Errors handled gracefully

**Timeline:** 4 weeks  
**Dependencies:** CDI retrieval must complete first

---

## Phase 3: Event Bowties (Weeks 5-9)

**Goal:** Visual event relationship display and drag-to-create linking

### F5: Event Bowties View (Priority: P1)

**Description:** Canvas displaying event producers → consumers

**Requirements:**
- Discover all event links (via Identify Events protocol)
- Display connected bowties (default view)
- Expandable unconnected elements tree
- Filter by node, segment, connection state

**Tasks:**
- [ ] Event discovery protocol (0x0997, 0x0544/04C4)
- [ ] Event link calculation (match Event IDs across nodes)
- [ ] Bowtie rendering component (Svelteflow or custom canvas)
- [ ] Unconnected elements tree component
- [ ] Filter controls

**Acceptance Criteria:**
- All configured events displayed
- Producers on left, consumers on right
- Event IDs visible on connections
- Unconnected elements grouped by Node → Segment

### F7: Event Link Creation (Priority: P1)

**Description:** Drag-and-drop to create event links

**Requirements:**
- Drag producer onto consumer bowtie → Copy Event ID
- Drag consumer slot onto producer bowtie → Copy Event ID
- Write modified Event ID to node configuration memory
- Update bowtie visualization immediately

**Tasks:**
- [ ] Memory Configuration write protocol
- [ ] Drag-and-drop interaction (Svelte DnD)
- [ ] Event ID copy logic (determine direction)
- [ ] Confirmation prompts (if both have Event IDs)
- [ ] Real-time bowtie update

**Acceptance Criteria:**
- Drag-to-bowtie creates link <30 seconds
- Event ID copied correctly
- Bowtie updates without page refresh
- Undo/revert capability

**Timeline:** 5 weeks  
**Dependencies:** Configuration value read/write, Event discovery

---

## Phase 4: Event Monitor (Weeks 10-12)

**Goal:** Real-time event visibility for troubleshooting

### F6: Real-Time Event Monitor (Priority: P1)

**Description:** Live log of all events on the network

**Requirements:**
- Subscribe to all event messages (PCER: 0x095B4)
- Correlate events with producers/consumers
- Display event details (timestamp, ID, source)
- Pause, clear, filter log
- Jump to configuration from event

**Tasks:**
- [ ] Event subscription in lcc-rs
- [ ] Tauri event streaming (backend → frontend)
- [ ] Scrolling log component
- [ ] Event correlation logic
- [ ] Filter and search controls
- [ ] Jump-to-configuration links

**Acceptance Criteria:**
- Events appear <50ms after occurrence
- Producer/consumer correlation accurate
- Log doesn't degrade performance (>1000 events)
- Session-only history (cleared on disconnect)

**Timeline:** 3 weeks  
**Dependencies:** Event discovery for correlation

---

## Phase 5: Polish & Testing (Weeks 13-15)

**Goal:** Production-ready quality and documentation

**Tasks:**
- [ ] End-to-end testing (full workflows)
- [ ] Integration testing with multiple node types
- [ ] Performance optimization (large networks)
- [ ] Error message refinement
- [ ] User documentation and tutorials
- [ ] Screen recording demos
- [ ] Release packaging (Windows, macOS, Linux)

**Acceptance Criteria:**
- All P1 features working reliably
- Tested on real LCC hardware (Tower-LCC, others)
- Clear error messages for all failure modes
- User guide covers all workflows

**Timeline:** 3 weeks

---

## Priority 2: Enhanced Functionality (Post-MVP)

### F8: Node Metadata Management

- User-editable node names, descriptions
- Location tagging system
- Notes per element
- SQLite persistence

**Estimated Effort:** 2 weeks

### F9: Advanced Configuration UI

- Support all CDI data types (int, string, eventid)
- Validation based on CDI constraints (min/max)
- Multi-element editing (apply to multiple lines)

**Estimated Effort:** 3 weeks

### F10: Event ID Conflict Detection

- Scan for duplicate Event IDs
- Visual indicators in Configuration view
- Resolution wizard (reassign)

**Estimated Effort:** 2 weeks

### F11: Manual Event Link Creation

- [+ Manual] button wizard
- Dropdown selection for producer/consumer
- Preview before applying

**Estimated Effort:** 1 week

### F12: Testing & Simulation

- [Test] button opens filtered Event Monitor
- Manual event triggering (send events from app)
- Simulate button presses

**Estimated Effort:** 2 weeks

---

## Priority 3: Advanced Features (Future)

### F13: Layout Persistence

- Save/load projects (.bowties files)
- Store canvas positions, zoom, metadata
- Recent projects list

**Estimated Effort:** 2 weeks

### F14: Documentation Export

- Export bowties as PDF, SVG, PNG
- Generate text reports
- Multi-page PDFs

**Estimated Effort:** 3 weeks

### F15: Network Configuration Management

- Multiple network profiles
- Dev + production networks
- Network switching

**Estimated Effort:** 1 week

### F16: Advanced Event Monitor

- Event highlighting in Bowties
- Export logs (CSV, JSON)
- Historical logging (opt-in)
- Traffic statistics

**Estimated Effort:** 2 weeks

---

## Priority 4: Future Enhancements (Exploratory)

### F17: Auto-Layout Algorithms

- Hierarchical layout
- Force-directed layout
- Manual drag positioning

**Estimated Effort:** 4 weeks

### F18: Collaboration Features

- Export/import configuration templates
- Share bowtie diagrams
- QR code sharing

**Estimated Effort:** 3 weeks

### F19: Mobile Companion App

- View-only bowties on tablet
- Event monitor for field troubleshooting
- Quick element lookup

**Estimated Effort:** 8+ weeks (separate project)

---

## Feature Dependencies

```
Phase 1 (Foundation)
  └── Phase 2 (Configuration View)
       ├── F2: CDI Retrieval
       ├── F3: Config Values     [depends on F2]
       └── F4: Miller Columns    [depends on F2, F3]
            └── Phase 3 (Event Bowties)
                 ├── F5: Bowties View
                 └── F7: Link Creation [depends on F5]
                      └── Phase 4 (Event Monitor)
                           └── F6: Event Monitor
                                └── Phase 5 (Polish)
```

## Timeline Summary

**Total MVP (P1 Features):** ~15 weeks from Phase 1 completion

- **Phase 2:** Weeks 1-4 (Configuration View)
- **Phase 3:** Weeks 5-9 (Event Bowties)
- **Phase 4:** Weeks 10-12 (Event Monitor)
- **Phase 5:** Weeks 13-15 (Polish & Testing)

**Post-MVP (P2-P4):** Incremental releases based on user feedback

## Success Metrics

**MVP Completion Criteria:**
- All P1 features functional
- Tested with real LCC hardware
- User can complete all 6 workflows from [docs/design/workflows.md](../design/workflows.md)
- Performance targets met (see [docs/design/vision.md](../design/vision.md))

**User Validation:**
- Model railroad hobbyist can configure simple layout
- Event link creation faster than competing tools
- No protocol knowledge required for basic usage

---

*For current status, see [docs/technical/architecture.md](../technical/architecture.md)*  
*For feature details, see [docs/design/vision.md](../design/vision.md)*
