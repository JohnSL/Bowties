# Research: CDI XML Viewer

**Feature**: 001-cdi-xml-viewer  
**Phase**: 0 - Research & Decision Documentation  
**Date**: February 16, 2026

## Research Overview

This document consolidates research findings for implementing the CDI XML viewer feature. All technical unknowns from the planning phase have been resolved through analysis of existing codebase, framework documentation, and best practices.

## Research Topics

### 1. CDI Data Availability & Storage

**Question**: How is CDI data currently retrieved and stored in the application?

**Decision**: Use existing node data structures; CDI data is retrieved via LCC memory configuration protocol

**Rationale**: 
- CDI (Configuration Description Information) is stored in node memory space as defined by LCC standards (TN-9.7.4.1)
- The lcc-rs library likely already implements or will implement memory read operations via datagrams
- CDI XML is retrieved from a well-known memory address on the node
- Current implementation should cache retrieved CDI data in the node management state

**Alternatives considered**:
- Fetch CDI on-demand for each view: Rejected due to network latency and unnecessary protocol traffic
- Store CDI in separate database: Rejected as over-engineering for debugging tool

**Implementation approach**: 
- Backend Tauri command accepts node ID, retrieves CDI from in-memory node cache
- If CDI not yet retrieved, command returns appropriate error state
- Frontend handles both success (display XML) and error (show message) states

---

### 2. XML Formatting Approach

**Question**: What's the best approach for formatting XML with proper indentation?

**Decision**: Use browser-native XML formatting in frontend (JavaScript), not Rust backend

**Rationale**:
- XML formatting is presentation logic, belongs in UI layer
- JavaScript has excellent XML DOM parsing via `DOMParser`
- Libraries like `xml-formatter` or built-in `XMLSerializer` provide indentation
- Keeps backend simple (just returns raw XML string)
- Reduces backend dependencies and complexity

**Alternatives considered**:
- Format XML in Rust backend: Rejected because it adds dependency (e.g., `quick-xml`, `xmltree`) for purely presentational task
- Use external service: Rejected as overkill and adds network dependency
- No formatting (raw XML): Rejected as defeats purpose of debugging tool

**Implementation approach**:
```typescript
// Frontend: lib/utils/xmlFormatter.ts
export function formatXml(xmlString: string, indent: number = 2): string {
  const parser = new DOMParser();
  const xmlDoc = parser.parseFromString(xmlString, 'text/xml');
  
  // Check for parse errors
  const parseError = xmlDoc.querySelector('parsererror');
  if (parseError) {
    return xmlString; // Return unformatted if parse fails
  }
  
  // Use XMLSerializer with formatting (or xml-formatter library)
  // Implementation will use standard indentation approach
  return prettyPrintXml(xmlDoc, indent);
}
```

---

### 3. Modal/Dialog UI Pattern in Tauri

**Question**: What's the best practice for displaying modal dialogs in Tauri applications?

**Decision**: Use Svelte modal component with overlay, not native OS dialogs

**Rationale**:
- Tauri apps typically use web-based UI patterns for consistency across platforms
- Svelte modal components provide full control over styling and behavior
- Native dialogs (`tauri::api::dialog`) are limited to simple alerts/file pickers
- Web modals allow copy-to-clipboard, syntax highlighting, scrolling for large content

**Alternatives considered**:
- Native OS dialog: Rejected due to limited control over presentation and copy functionality
- New Tauri window: Rejected as heavyweight, adds window management complexity
- Inline expansion: Rejected as would disrupt node list layout

**Implementation approach**:
```svelte
<!-- Frontend: lib/components/CdiXmlViewer.svelte -->
<script>
  export let visible: boolean = false;
  export let xmlContent: string = '';
  export let nodeId: string = '';
  
  // Modal management, formatting, copy-to-clipboard
</script>

{#if visible}
  <div class="modal-overlay" on:click={close}>
    <div class="modal-content" on:click|stopPropagation>
      <header>
        <h2>CDI XML - Node {nodeId}</h2>
        <button on:click={copyToClipboard}>Copy</button>
        <button on:click={close}>Close</button>
      </header>
      <pre class="xml-content"><code>{formattedXml}</code></pre>
    </div>
  </div>
{/if}
```

---

### 4. Large XML Document Handling

**Question**: How to handle CDI XML documents that are very large (megabytes)?

**Decision**: Use virtual scrolling with limited initial render, warn if exceeds threshold

**Rationale**:
- Most CDI documents are small (< 100KB) based on typical LCC node configurations
- Browsers handle hundreds of KB of text rendering efficiently
- 10MB threshold in spec is conservative; real-world CDI rarely exceeds 1MB
- Virtual scrolling libraries exist if needed (e.g., `svelte-virtual-list`)

**Alternatives considered**:
- Load all content always: Rejected if document > 1MB (performance risk)
- Paginate XML: Rejected as breaks context and searchability
- Collapse elements by default: Considered for future enhancement

**Implementation approach**:
- Display warning if CDI XML > 1MB: "Large document may impact performance"
- render first 1000 lines, show "Load more" button if longer
- Provide "Copy raw XML" button to access full content without rendering
- Future: Add collapsible XML tree view for very large documents

---

### 5. Context Menu Integration

**Question**: How to add right-click context menu to node list in SvelteKit?

**Decision**: Use HTML5 `contextmenu` event with custom Svelte menu component

**Rationale**:
- Standard web approach, works consistently across platforms
- Svelte reactive system makes menu positioning and state management simple
- Can be enhanced later with keyboard shortcuts (Ctrl+I for "Inspect CDI")

**Alternatives considered**:
- Native context menu via Tauri plugin: Not available in Tauri 2 for custom menus
- Button in each row: Rejected as clutters UI, not discoverable
- Dedicated "Tools" menu: Considered for future enhancement

**Implementation approach**:
```svelte
<!-- In existing node list component -->
<script>
  let contextMenuVisible = false;
  let contextMenuX = 0;
  let contextMenuY = 0;
  let selectedNode = null;

  function handleContextMenu(event: MouseEvent, node: Node) {
    event.preventDefault();
    selectedNode = node;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    contextMenuVisible = true;
  }

  async function viewCdiXml() {
    // Show loading state
    // Call Tauri command
    // Open CdiXmlViewer modal
    contextMenuVisible = false;
  }
</script>

<div on:contextmenu={(e) => handleContextMenu(e, node)}>
  <!-- Node display -->
</div>

{#if contextMenuVisible}
  <ContextMenu x={contextMenuX} y={contextMenuY}>
    <MenuItem on:click={viewCdiXml}>View CDI XML</MenuItem>
    <!-- Other menu items -->
  </ContextMenu>
{/if}
```

---

### 6. Error Handling Patterns

**Question**: What error states need to be handled for CDI viewing?

**Decision**: Handle 4 specific error states with clear messaging

**Error states identified**:
1. **CDI not retrieved yet**: "CDI data has not been retrieved for this node. Retrieve configuration first."
2. **CDI retrieval failed**: "CDI retrieval failed: [error details]. Check node connection."
3. **Invalid XML**: Display raw content with error message: "XML parsing failed. Raw content shown below."
4. **No CDI available**: "This node does not provide CDI (Configuration Description Information)."

**Implementation approach**:
- Backend returns `Result<Option<String>, Error>` from Tauri command
- `Ok(Some(xml))` → Display formatted XML
- `Ok(None)` → "No CDI available" message  
- `Err(e)` → Error message with details
- Frontend shows appropriate message in modal or toast notification

---

## Technology Choices

### Backend (Rust/Tauri)

**Dependencies**: None required (uses existing Tauri + lcc-rs)
- Tauri command returns raw XML string from node cache
- Error handling via `Result` type
- No XML processing in Rust (handled by frontend)

### Frontend (TypeScript/SvelteKit)

**Dependencies**: Consider light XML formatting library
- **xml-formatter** (optional): Lightweight, 6KB, well-maintained
- Alternative: Implement simple indentation logic in-house (prefer this to avoid dependency)
- **No heavy XML libraries**: Avoid `libxmljs`, `saxes` (overkill for formatting)

**Approach**: Prefer zero new dependencies; implement simple indentation using DOM parsing and string manipulation.

---

## Best Practices Summary

### XML Display
- Use `<pre>` with `<code>` for monospaced, preserving whitespace
- Apply syntax highlighting (optional enhancement): Use `highlight.js` XML mode
- Enable text selection and copy (browser default behavior)
- Wrap long lines with horizontal scroll, not text wrapping

### UX
- Show loading indicator while fetching/formatting (>100ms operations)
- Provide clear close button and Escape key handler
- Click outside modal to close (standard pattern)
- Display node ID in modal header for context

### Performance  
- Lazy load modal component (don't render until needed)
- Format XML on-demand (not during initial node load)
- Consider debouncing if formatting >500ms (unlikely for most CDI)

### Accessibility
- Modal should trap focus (tab cycles within modal)
- Escape key closes modal
- Screen reader: Announce modal opening, provide close button label
- High contrast mode: Ensure modal overlay and borders are visible

---

## Implementation Checklist

- [ ] Backend: Create Tauri command `get_cdi_xml(node_id: String) -> Result<Option<String>>`
- [ ] Backend: Retrieve CDI from node cache, handle missing/error states
- [ ] Frontend: Create `xmlFormatter.ts` utility with simple indentation logic
- [ ] Frontend: Create `CdiXmlViewer.svelte` modal component
- [ ] Frontend: Add context menu to node list (or existing node display)
- [ ] Frontend: Wire up context menu → Tauri command → modal display
- [ ] Frontend: Implement copy-to-clipboard button
- [ ] Frontend: Add error state handling (4 scenarios)
- [ ] Testing: Unit tests for XML formatter edge cases
- [ ] Testing: Integration test for Tauri command (mock node with CDI)
- [ ] Testing: UI component tests for modal behavior
- [ ] Documentation: Update user docs with "View CDI XML" feature

---

## Open Questions / Future Enhancements

**Open Questions**: None remaining (all research complete)

**Future Enhancements** (out of scope for this feature):
- Syntax highlighting for XML (nice-to-have, not critical for debugging)
- Collapsible XML tree view for navigating large documents
- Search/filter within XML content
- Compare CDI XML between nodes
- Export CDI to file
- Edit CDI XML (dangerous, requires validation and write-back)

---

## Conclusion

All technical decisions resolved. Implementation can proceed to Phase 1 (design) with:
- **Backend**: Simple Tauri command returning raw CDI XML string
- **Frontend**: Custom modal component with in-house XML formatting
- **Zero new dependencies**: Uses existing tech stack
- **Clear error handling**: 4 identified error states with user-friendly messages
- **Standard UX patterns**: Right-click menu, modal overlay, copy button

Ready for **Phase 1: Design** (data model, contracts, quickstart).
