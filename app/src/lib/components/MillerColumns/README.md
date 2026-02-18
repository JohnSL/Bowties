# Miller Columns CDI Navigator

## Overview

The Miller Columns component provides a multi-column navigation interface for exploring LCC (Layout Command Control) Configuration Description Information (CDI) hierarchies. It implements the Miller Columns pattern (popularized by macOS Finder) for intuitive hierarchical navigation.

## Components

### `MillerColumnsNav.svelte`
Main container component that orchestrates the navigation experience.

**Features:**
- Error boundary for parsing errors (T100)
- Horizontal scroll indicators (T109)
- ARIA labels for accessibility (T107)
- Loading overlay for async operations
- Error banner with dismissible alerts

**Props:** None (uses store)

**Usage:**
```svelte
<script>
  import MillerColumnsNav from '$lib/components/MillerColumns/MillerColumnsNav.svelte';
</script>

<MillerColumnsNav />
```

### `NodesColumn.svelte`
Leftmost column displaying discovered LCC nodes.

**Features:**
- Lists all discovered nodes from the network
- Indicates CDI availability with badge
- Triggers segment column load on selection

### `NavigationColumn.svelte`
Reusable column component for segments, groups, and elements.

**Features:**
- Dynamic rendering based on column type (segments/groups/elements)
- Keyboard navigation (arrows, Enter) (T106)
- Loading indicators (T099)
- Parsing issue indicators (T098)
- Element type icons
- Instance badges for replicated groups

**Props:**
```typescript
{
  column: ColumnData;         // Column configuration and items
  selectedItemId: string | null;  // Currently selected item ID
}
```

### `DetailsPanel.svelte`
Rightmost panel showing element details.

**Features:**
- Element metadata (name, description, data type)
- Constraints (range, map, length)
- Memory address
- Full breadcrumb path
- "No CDI data" message (T097)

**Props:** None (uses store)

### `Breadcrumb.svelte`
Navigation breadcrumb showing current path.

**Features:**
- Click segments to navigate back
- Path truncation for deep hierarchies
- Tooltips for full path
- Instance numbers for replicated groups

## State Management

All components use the `millerColumnsStore` for state management.

**Store interface:**
```typescript
interface MillerColumnsState {
  selectedNode: { nodeId: string; nodeName: string } | null;
  columns: ColumnData[];
  breadcrumb: NavigationStep[];
  selectedElementDetails: ElementDetails | null;
  isLoading: boolean;
  error: string | null;
}
```

**Store actions:**
```typescript
millerColumnsStore.selectNode(nodeId, nodeName)
millerColumnsStore.addColumn(column)
millerColumnsStore.removeColumnsAfter(depth)
millerColumnsStore.updateBreadcrumb(step)
millerColumnsStore.setLoading(isLoading)
millerColumnsStore.setError(error)
millerColumnsStore.setElementDetails(details)
millerColumnsStore.reset()
```

## Accessibility Features

### Keyboard Navigation (T106)
- **Arrow Keys**: Navigate up/down within a column
- **Enter/Space**: Select highlighted item
- **Tab**: Move between columns

### Screen Reader Support (T107)
- ARIA roles: `navigation`, `listbox`, `option`, `region`
- ARIA labels: Column titles, item counts, error messages
- ARIA live regions: Loading states, errors

### Visual Indicators
- Focus outlines for keyboard navigation
- Loading spinners for async operations
- Error states with icons and descriptive messages
- Scroll indicators for horizontal overflow (T109)

## Error Handling

### T097: No CDI Data
Shows helpful message when node doesn't provide CDI.

### T098: Parsing Issues
Displays warning icon and error message for malformed CDI XML.

### T099: Loading States
Spinner appears during async operations (column population).

### T100: Error Boundary
Catches and displays fatal parsing errors with recovery option.

## Performance Optimizations

### Backend (T103-T104)
- **CDI Parsing Cache**: Parsed CDI structs cached by node ID
- **Performance Tracking**: Logs warnings if operations >500ms

### Frontend (T101-T102)
- **Debouncing**: 50ms debounce on navigation clicks
- **Request Cancellation**: Aborts previous pending requests

## API Integration

Uses Tauri commands for backend communication:

```typescript
import { 
  getDiscoveredNodes,
  getCdiStructure,
  getColumnItems,
  getElementDetails 
} from '$lib/api/cdi';

// Get nodes for Nodes column
const { nodes } = await getDiscoveredNodes();

// Load segments after node selection
const structure = await getCdiStructure(nodeId);

// Navigate to next column
const items = await getColumnItems(nodeId, parentPath, depth);

// Load element details
const details = await getElementDetails(nodeId, elementPath);
```

## Testing

### Unit Tests
- Component rendering tests (DetailsPanel, NavigationColumn, NodesColumn)
- Store action tests
- TypeScript type validation

### Integration Tests
- End-to-end navigation flows
- Error handling scenarios
- Keyboard navigation

### Manual Testing
See `quickstart.md` for test scenarios (T112).

## Styling

CSS variables for theming:
```css
--bg-color: Background color
--text-primary: Primary text color
--text-secondary: Secondary text color
--border-color: Border color
--primary-color: Primary brand color
--error-color: Error state color
--item-hover: Item hover background
--item-selected: Selected item background
```

## Development

### Adding a New Column Type
1. Define column type in `MillerColumnsState`
2. Add rendering logic in `NavigationColumn.svelte`
3. Implement backend command in `app/src-tauri/src/commands/cdi.rs`
4. Add TypeScript wrapper in `app/src/lib/api/cdi.ts`

### Debugging
- Check browser console for client-side errors
- Check Tauri logs for backend errors
- Use `[PERF]` logs to identify slow operations

## References

- [Miller Columns Pattern](https://en.wikipedia.org/wiki/Miller_columns)
- [WCAG 2.1 Accessibility Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [LCC CDI Specification](../../../../../../specs/003-miller-columns/)
