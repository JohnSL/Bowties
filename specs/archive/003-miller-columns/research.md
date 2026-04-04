# Research: Miller Columns Configuration Navigator

**Feature**: 003-miller-columns  
**Date**: 2026-02-17  
**Status**: Phase 0 Complete

## Overview

This document consolidates research findings for implementing the Miller Columns navigation feature. Research focused on three critical areas: CDI XML parsing in Rust, Miller Columns UI patterns in SvelteKit, and testing strategies for hierarchical components.

---

## 1. CDI XML Parsing in Rust

### Decision: Use `roxmltree` for CDI XML Parsing

**Rationale:**
- **Read-only DOM-like API** - Perfect for read-only CDI navigation
- **Zero allocations** - Stores tree in single buffer, excellent performance
- **Namespace-aware** - Handles CDI schema correctly
- **Simple API** - Easier than quick-xml's streaming for hierarchical navigation
- **Battle-tested** - Widely used in Rust ecosystem

**Alternatives Considered:**
- `quick-xml` - Streaming parser, lower memory but more complex code for tree navigation
- `serde-xml-rs` - Rejected: Variable depth and replication don't fit serde's derive model

**Implementation:**
```toml
# Add to lcc-rs/Cargo.toml
roxmltree = "0.20"
```

### CDI XML Structure (Per S-9.7.4.1)

```
<cdi>
  └─ <identification> (optional - manufacturer, model, versions)
  └─ <acdi> (optional - standardized node info)
  └─ <segment> (0+ segments, defines memory spaces)
      └─ <group> (logical grouping, supports replication)
          └─ <int|string|eventid|float|action|blob> (data elements)
          └─ <group> (groups can nest recursively - UNLIMITED DEPTH)
```

**Key Attributes:**
- `<segment>`: `space` (memory space #), `origin` (start address)
- `<group>`: `replication` (repeat count, default 1), `offset`
- Data elements: `size` (bytes), `offset`, `min`, `max`, `map`

**Variable Hierarchy Depth:**
- Minimum: 3 levels (Node → Segment → Element)
- Maximum: Unlimited (Tower-LCC shows 8 levels with nested groups)

### Rust Type Design

```rust
// Core CDI structure
pub struct Cdi {
    pub identification: Option<Identification>,
    pub acdi: Option<Acdi>,
    pub segments: Vec<Segment>,
}

pub struct Segment {
    pub name: Option<String>,
    pub description: Option<String>,
    pub space: u8,
    pub origin: i32,
    pub elements: Vec<DataElement>,
}

// Enum for all data element types
pub enum DataElement {
    Group(Group),
    Int(IntElement),
    String(StringElement),
    EventId(EventIdElement),
    Float(FloatElement),
    Action(ActionElement),
    Blob(BlobElement),
}

pub struct Group {
    pub name: Option<String>,
    pub description: Option<String>,
    pub offset: i32,
    pub replication: u32,              // Default: 1
    pub repname: Vec<String>,          // Instance naming template
    pub elements: Vec<DataElement>,    // RECURSIVE!
    pub hints: Option<GroupHints>,
}

pub struct IntElement {
    pub name: Option<String>,
    pub description: Option<String>,
    pub size: u8,                      // 1, 2, 4, or 8 bytes
    pub offset: i32,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub default: Option<i64>,
    pub map: Option<Map>,
}

pub struct EventIdElement {
    pub name: Option<String>,
    pub description: Option<String>,
    pub offset: i32,
    // Event IDs are always 8 bytes
}
```

### Replication Handling Strategy

**Decision: Expand During Parsing** (simpler for UI)

```rust
impl Group {
    /// Expand replicated group into N individual instances
    /// group[replication=16] → 16 Group structs with computed addresses
    fn expand_replications(&self, base_address: i32) -> Vec<ExpandedGroup> {
        (0..self.replication).map(|i| {
            ExpandedGroup {
                index: i,
                name: self.compute_repname(i),
                address: base_address + (i as i32 * self.size_per_replication()),
                elements: self.elements.clone(),
            }
        }).collect()
    }
    
    fn compute_repname(&self, index: u32) -> String {
        // Handle repname numbering per spec rules
        // Example: "Line" + repname → "Line 1", "Line 2", ...
        if let Some(template) = self.repname.first() {
            format!("{} {}", self.name.as_deref().unwrap_or(""), index + 1)
        } else {
            format!("{} {}", self.name.as_deref().unwrap_or("Group"), index + 1)
        }
    }
}
```

**Alternative Considered:** Keep compact, expand on-demand (lower memory) - Rejected for initial implementation due to UI complexity.

### Empty Group Filtering (Per CDI Footnote 4)

**Requirement:** S-9.7.4.1 Footnote 4 states: "Configuration Tools shall not render a group element with no child elements (no name, no description, no link, and no data elements contained)."

```rust
impl Group {
    fn should_render(&self) -> bool {
        self.name.is_some() 
            || self.description.is_some() 
            || !self.elements.is_empty()
    }
}
```

### Existing Code in lcc-rs

**Found:**
- `CdiData` struct in types.rs (stores raw XML only, no parsing)
- `AddressSpace::Cdi` enum variant in memory_config.rs
- Protocol support for reading CDI from space 0xFF

**Not Found:**
- No CDI parsing implementation
- No CDI structure types beyond raw XML storage

**Next Steps:**
1. Create `lcc-rs/src/cdi/mod.rs` with type definitions
2. Create `lcc-rs/src/cdi/parser.rs` with roxmltree parsing logic
3. Create `lcc-rs/src/cdi/hierarchy.rs` for navigation helpers
4. Implement `parse_cdi(xml: &str) -> Result<Cdi>`
5. Add unit tests for parsing, replication, empty group filtering

---

## 2. Miller Columns UI in SvelteKit

### Decision: Flexbox Layout with Dynamic Column Injection

**Rationale:**
- Existing codebase uses flexbox extensively (CdiXmlViewer, NodeList)
- Dynamic column count works naturally (just add/remove child divs)
- Simpler overflow handling for horizontal scrolling
- Better browser support than CSS Grid for dynamic content

**CSS Structure:**

```css
.miller-columns-container {
  display: flex;
  flex-direction: row;
  overflow-x: auto;
  height: 100vh;
  gap: 0; /* Columns have borders */
}

.column {
  flex: 0 0 250px; /* Fixed 250px width per column */
  min-width: 250px;
  max-width: 250px;
  overflow-y: auto;
  border-right: 1px solid var(--border-color);
}

.details-panel {
  flex: 1; /* Takes remaining space */
  min-width: 300px;
  overflow-y: auto;
}
```

**Why Not CSS Grid:** `grid-template-columns: repeat(auto-fit, 250px)` doesn't work well with dynamic content and fixed panel widths.

### State Management Pattern: Single Class-Based Store with Svelte 5 Runes

**Decision:** Follow existing pattern from nodes.ts - class with `$state` runes

```typescript
// File: app/src/lib/stores/millerColumns.ts

interface NavigationStep {
  depth: number;
  itemId: string;
  itemType: 'node' | 'segment' | 'group' | 'element';
  label: string;
}

interface ColumnData {
  depth: number;
  type: 'nodes' | 'segments' | 'groups' | 'elements';
  items: ColumnItem[];
  parentPath: string[];
}

interface ColumnItem {
  id: string;
  name: string;
  type?: string;
  hasChildren: boolean;
  metadata?: Record<string, unknown>;
}

class MillerColumnsStore {
  // Navigation path: array of selections from root to current position
  private _path = $state<NavigationStep[]>([]);
  
  // Content for each visible column (indexed by depth level)
  private _columns = $state<ColumnData[]>([]);
  
  // Currently selected item at each level
  private _selections = $state<Map<number, string>>(new Map());
  
  // Loading states per column
  private _loading = $state<Map<number, boolean>>(new Map());
  
  // Details panel content
  private _detailsContent = $state<ElementDetails | null>(null);

  get path() { return this._path; }
  get columns() { return this._columns; }
  get selections() { return this._selections; }
  get detailsContent() { return this._detailsContent; }

  // Navigate to item at specific depth level
  async navigateTo(depth: number, itemId: string) {
    // 1. Clear columns deeper than current depth
    this._columns = this._columns.slice(0, depth + 1);
    this._path = this._path.slice(0, depth + 1);
    
    // 2. Set selection at this level
    this._selections.set(depth, itemId);
    
    // 3. Fetch next column content (if applicable)
    this._loading.set(depth + 1, true);
    const nextColumnData = await this.fetchColumnData(depth + 1, itemId);
    
    if (nextColumnData) {
      this._columns = [...this._columns, nextColumnData];
    }
    
    this._loading.set(depth + 1, false);
    
    // 4. Update details panel
    this._detailsContent = await this.fetchElementDetails(itemId);
  }

  // Cache column data to avoid re-fetching
  private columnCache = new Map<string, ColumnData>();
  private abortControllers = new Map<number, AbortController>();

  async fetchColumnData(depth: number, parentId: string) {
    const cacheKey = `${depth}:${parentId}`;
    
    if (this.columnCache.has(cacheKey)) {
      return this.columnCache.get(cacheKey);
    }
    
    // Cancel previous request for this depth
    this.abortControllers.get(depth)?.abort();
    const controller = new AbortController();
    this.abortControllers.set(depth, controller);
    
    const data = await invoke('get_cdi_children', { 
      parentId,
      signal: controller.signal 
    });
    
    this.columnCache.set(cacheKey, data);
    return data;
  }
}

export const millerColumnsStore = new MillerColumnsStore();
```

**Key Advantages:**
- Matches existing codebase pattern (class with `$state`)
- Single source of truth for navigation state
- Reactive getters automatically update components
- Centralized column lifecycle management
- Built-in caching and request cancellation

### Animation Strategy: CSS Transforms + Svelte Transitions

**Decision:** GPU-accelerated CSS animations <200ms

```svelte
<!-- Column.svelte -->
{#each columns as column, index (column.depth)}
  <div 
    class="column"
    style="animation-delay: {index * 30}ms"
    transition:slide={{ duration: 150, axis: 'x', easing: cubicOut }}
  >
    <!-- Column content -->
  </div>
{/each}
```

```css
.column {
  animation: slideInColumn 150ms cubic-bezier(0.4, 0, 0.2, 1);
  transform-origin: left center;
  
  /* Isolate layout calculations for performance */
  contain: layout style;
}

@keyframes slideInColumn {
  from {
    opacity: 0;
    transform: translateX(-20px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
```

**Performance Optimizations:**
- Use `transform` and `opacity` (GPU-accelerated) NOT `width` or `left`
- Duration <200ms per spec requirement
- Stagger column additions by 30ms for visual flow
- Use `cubic-bezier` easing for natural motion
- CSS `contain: layout style` for column isolation

### Performance Optimization for Large Lists (No Virtual Scrolling)

**Decision:** Direct rendering with optimizations

**1. Debounced Selection (Prevent Race Conditions)**
```typescript
let selectionTimeout: ReturnType<typeof setTimeout>;

function handleItemClick(itemId: string, depth: number) {
  clearTimeout(selectionTimeout);
  selectionTimeout = setTimeout(() => {
    millerColumnsStore.navigateTo(depth, itemId);
  }, 50); // 50ms debounce per spec
}
```

**2. Request Cancellation**
Already shown in store implementation above with AbortController.

**3. Progressive Rendering for 100+ Items**
```typescript
function renderLargeList(items: ColumnItem[]) {
  let visibleItems = $state(items.slice(0, 50));
  
  if (items.length > 50) {
    requestIdleCallback(() => {
      visibleItems = items;
    });
  }
  
  return visibleItems;
}
```

**4. CSS Performance**
```css
.column-item {
  contain: paint; /* Reduce paint area */
  transition: background-color 0.1s ease; /* Avoid layout thrash */
}
```

**Performance Targets Achievable:**
- <100ms navigation: Debounced clicks + AbortController
- <200ms column transitions: GPU-accelerated transforms
- <500ms column population: Cached responses + progressive rendering

### Component Structure

```
lib/components/MillerColumns/
├── MillerColumnsNav.svelte      # Main container (flex layout)
├── Column.svelte                 # Reusable column component
├── ColumnItem.svelte             # Individual item (optimized rendering)
├── NodesColumn.svelte            # Specialized column for nodes
├── NavigationColumn.svelte       # Generic column for segments/groups/elements
├── DetailsPanel.svelte           # Right panel (not a column)
└── Breadcrumb.svelte             # Navigation breadcrumb
```

### Existing Miller Columns Libraries

**Finding:** **No production-ready Svelte 5 Miller Columns libraries exist**

Searched options:
- `svelte-miller-columns` (npm) - Last updated 2019, Svelte 2, abandoned
- GitHub examples - Proof-of-concepts only, not maintained

**Decision:** Build from scratch using patterns above

**Rationale:**
- Full control over performance
- Integration with Tauri backend
- Compliance with specific CDI hierarchy requirements
- Modern Svelte 5 runes patterns

**Inspiration:** macOS Finder (canonical reference) - 250px columns, horizontal scroll, independent vertical scroll

---

## 3. Testing Strategies for Hierarchical Components

### Current Testing Infrastructure Status

**Backend (Rust):** ✅ **Mature Testing Setup**
- Integration tests in `lcc-rs/tests/protocol_integration.rs`
- Mock transport pattern with `MockTransport` implementing `LccTransport`
- Async testing with `#[tokio::test]`
- Inline `#[cfg(test)]` modules in source files

**Frontend (SvelteKit):** ⚠️ **NO TESTING INFRASTRUCTURE EXISTS**
- No Vitest, Jest, or Testing Library in package.json
- No `.test.ts` or `.spec.ts` files found
- Only TypeScript type checking via `svelte-check`

**Action Required:** Install testing infrastructure before implementing tests

### Decision: Vitest + Testing Library for Frontend

**Installation:**
```bash
npm install -D vitest @testing-library/svelte @testing-library/jest-dom @vitest/ui jsdom
```

**Configuration (vite.config.js):**
```javascript
import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
  plugins: [sveltekit()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    include: ['src/**/*.{test,spec}.{js,ts}']
  }
});
```

### Testing Strategy: Three-Layer Approach

**Layer 1: Component Unit Tests (Vitest + Testing Library)**

Test individual columns with mocked data:

```typescript
// src/lib/components/MillerColumns/MillerColumnsNav.test.ts
import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import MillerColumnsNav from './MillerColumnsNav.svelte';

describe('MillerColumnsNav - Dynamic Depth', () => {
  it('renders 3-level hierarchy (Node → Segment → Element)', () => {
    const mockCdiData = createShallowCdi();
    render(MillerColumnsNav, { props: { cdiData: mockCdiData } });
    
    expect(screen.getAllByRole('list')).toHaveLength(3);
  });

  it('renders 8-level hierarchy with nested groups', () => {
    const mockCdiData = createDeepCdi(8);
    render(MillerColumnsNav, { props: { cdiData: mockCdiData } });
    
    expect(screen.getAllByRole('list')).toHaveLength(8);
  });

  it('dynamically adds column when navigating into nested group', async () => {
    const { container } = render(MillerColumnsNav);
    
    const initialColumns = container.querySelectorAll('[data-column]');
    expect(initialColumns).toHaveLength(3);
    
    await fireEvent.click(screen.getByText('Conditionals'));
    
    const updatedColumns = container.querySelectorAll('[data-column]');
    expect(updatedColumns).toHaveLength(4);
  });
});
```

**Layer 2: Tauri Integration Tests (Mocked Frontend)**

Mock Tauri commands for fast tests:

```typescript
// tests/mocks/tauri.ts
import { vi } from 'vitest';

export const mockTauriInvoke = vi.fn((command: string, args: any) => {
  if (command === 'get_cdi_children') {
    return Promise.resolve({
      items: generateMockItems(args.parentId)
    });
  }
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockTauriInvoke
}));
```

**Layer 3: Backend Integration Tests (Rust)**

```rust
// app/src-tauri/src/commands/cdi.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_cdi_hierarchy() {
        let xml = include_str!("../../../tests/fixtures/tower-lcc.xml");
        let parsed = parse_cdi_structure(xml).await.unwrap();
        
        assert_eq!(parsed.segments.len(), 5);
        assert_eq!(parsed.segments[2].groups.len(), 32); // Conditionals
        assert_eq!(parsed.max_depth, 8);
    }

    #[tokio::test]
    async fn test_navigate_to_deep_element() {
        let path = vec!["Conditionals", "Logic 1", "Variable #1"];
        let element = navigate_cdi_path(&mock_cdi(), &path).await.unwrap();
        
        assert_eq!(element.name, "Trigger");
        assert_eq!(element.depth, 5);
    }
}
```

### Mock CDI Data Strategy

**Decision:** Fixtures based on real CDI structures

```typescript
// tests/fixtures/cdi-samples.ts
export const CDI_SHALLOW = `
<?xml version="1.0"?>
<cdi>
  <segment space='253'>
    <name>Basic Config</name>
    <int size='1'><name>Address</name></int>
    <eventid><name>Event 1</name></eventid>
  </segment>
</cdi>
`;

export const CDI_WITH_REPLICATION = `
<segment space='253' origin='128'>
  <name>Port I/O</name>
  <group replication='16'>
    <name>Line</name>
    <repname>Line</repname>
    <eventid><name>Command</name></eventid>
  </group>
</segment>
`;

export const CDI_DEEP_NESTED = `
<segment space='253'>
  <name>Conditionals</name>
  <group replication='32'>
    <name>Logic</name>
    <group>
      <name>Variable #1</name>
      <eventid><name>set true</name></eventid>
      <eventid><name>set false</name></eventid>
    </group>
  </group>
</segment>
`;

// Helper to create dynamic depth
export function createCdiWithDepth(levels: number): string {
  let xml = '<?xml version="1.0"?><cdi><segment space="253"><name>Test</name>';
  for (let i = 0; i < levels - 2; i++) {
    xml += `<group><name>Level ${i}</name>`;
  }
  xml += '<eventid><name>Deep Element</name></eventid>';
  for (let i = 0; i < levels - 2; i++) {
    xml += '</group>';
  }
  xml += '</segment></cdi>';
  return xml;
}
```

**Real CDI Fixtures:**
- Store actual Tower-LCC.xml in `tests/fixtures/`
- Use for integration tests to catch edge cases

### Key Test Scenarios

**1. Navigation State Tests:**
- Breadcrumb maintains full path across depth changes
- Columns removed when navigating backward
- Selection cleared when switching nodes

**2. Column Rendering Tests:**
- Replicated groups show instance numbers (Line 1...Line 16)
- Event ID elements display type indicator
- Empty groups filtered per CDI Footnote 4

**3. CDI Parsing Tests (Rust):**
- Replication count parsed correctly (`replication='32'`)
- Address calculation with offsets
- Malformed XML graceful error handling

**4. Integration Tests:**
- Full workflow: Node → Conditionals → Logic #12 → Variable #1 → Trigger
- Performance: 100 replicated groups render without freeze
- Error handling: Missing CDI, malformed XML

### Testing Implementation Phases

**Phase 1:** Setup infrastructure (Vitest + fixtures)  
**Phase 2:** CDI parser tests (Rust unit + integration)  
**Phase 3:** Component tests (frontend unit)  
**Phase 4:** Integration tests (full workflow)

---

## Summary of Decisions

| Area | Decision | Rationale |
|------|----------|-----------|
| **XML Parsing** | roxmltree | Read-only DOM API, zero allocations, namespace-aware |
| **Replication** | Expand during parsing | Simpler for UI display |
| **UI Layout** | Flexbox with dynamic columns | Matches existing patterns, simple overflow |
| **State Management** | Class-based store with $state | Matches existing nodes.ts pattern |
| **Animations** | CSS transforms <150ms | GPU-accelerated, meets <200ms requirement |
| **Virtual Scrolling** | No (direct rendering + optimizations) | Per spec requirement, use progressive rendering |
| **Testing Framework** | Vitest + Testing Library | Modern, SvelteKit-compatible |
| **Mock Data** | Real CDI fixtures + generators | Validate against actual structures |
| **Component Build** | From scratch | No production-ready Svelte 5 libraries exist |

---

## Next Steps for Phase 1

1. Add `roxmltree` to lcc-rs dependencies
2. Create CDI parsing types in `lcc-rs/src/cdi/`
3. Create Svelte store in `app/src/lib/stores/millerColumns.ts`
4. Create component structure in `app/src/lib/components/MillerColumns/`
5. Set up Vitest testing infrastructure
6. Create mock CDI fixtures in `tests/fixtures/`

**Research complete. Ready for Phase 1 design.**
