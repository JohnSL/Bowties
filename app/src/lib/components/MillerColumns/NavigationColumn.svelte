<script lang="ts">
  import { millerColumnsStore, type ColumnData, type ColumnItem } from '$lib/stores/millerColumns';
  import { getColumnItems, getElementDetails } from '$lib/api/cdi';

  export let column: ColumnData;
  export let selectedItemId: string | null = null;

  // T099: Loading state for column population
  let isLoadingColumn = false;

  // T098: Parsing issue indicator
  let hasParsingIssue = false;
  let parsingIssueMessage = '';

  // T106: Keyboard navigation support
  let selectedIndex = 0;
  let listElement: HTMLUListElement;

  /**
   * T106: Handle keyboard navigation
   */
  function handleKeyDown(event: KeyboardEvent) {
    if (!column.items.length) return;

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, column.items.length - 1);
        scrollToSelected();
        break;
      
      case 'ArrowUp':
        event.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        scrollToSelected();
        break;
      
      case 'Enter':
      case ' ':
        event.preventDefault();
        if (column.items[selectedIndex]) {
          handleItemSelect(column.items[selectedIndex]);
        }
        break;
    }
  }

  /**
   * T108: Scroll to selected item for keyboard navigation
   */
  function scrollToSelected() {
    if (listElement) {
      const selectedElement = listElement.children[selectedIndex] as HTMLElement;
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      }
    }
  }

  /**
   * Get icon for element type
   */
  function getElementIcon(itemType: string | undefined): string {
    if (!itemType) return '';
    
    switch (itemType.toLowerCase()) {
      case 'eventid':
        return '🎯';
      case 'int':
      case 'integer':
        return '123';
      case 'string':
        return 'abc';
      case 'float':
        return '∞';
      case 'action':
        return '▶';
      case 'blob':
        return '📦';
      default:
        return '';
    }
  }

  /**
   * Handle item selection
   * 
   * T093: Supports backward navigation - clicking an item in any column
   * removes all subsequent columns and re-selects the clicked item
   */
  async function handleItemSelect(item: ColumnItem) {
    try {
      // T099: Set column-specific loading state
      isLoadingColumn = true;
      millerColumnsStore.setLoading(true);
      
      const currentNodeId = $millerColumnsStore.selectedNode?.nodeId;
      if (!currentNodeId) {
        throw new Error('No node selected');
      }

      // Build the current path using pathId from metadata (not the UUID in item.id)
      const pathId = item.metadata?.pathId || item.id;
      const currentPath = [...column.parentPath, pathId];
      
      // Update breadcrumb (T083: include instance number for replicated groups)
      let breadcrumbLabel = item.name;
      if (item.metadata?.replicated && item.metadata?.instanceNumber) {
        // For replicated instances, use format "GroupName #N"
        const baseName = item.name.replace(/ \d+$/, ''); // Remove trailing number if present
        breadcrumbLabel = `${baseName} #${item.metadata.instanceNumber}`;
      }
      
      millerColumnsStore.updateBreadcrumb({
        depth: column.depth,
        itemId: item.id,
        itemType: column.type === 'segments' ? 'segment' : column.type === 'elements' ? 'element' : 'group',
        label: breadcrumbLabel,
      });

      // T093: Remove columns after current depth (enables backward navigation)
      millerColumnsStore.removeColumnsAfter(column.depth);

      // If item has children, load next column
      if (item.hasChildren) {
        const nextColumnItems = await getColumnItems(
          currentNodeId,
          currentPath,
          column.depth + 1
        );

        millerColumnsStore.addColumn({
          depth: column.depth + 1,
          type: nextColumnItems.columnType as 'segments' | 'groups' | 'elements',
          items: nextColumnItems.items.map(colItem => ({
            id: colItem.id,
            name: colItem.name,
            fullName: colItem.fullName,
            type: colItem.type,
            hasChildren: colItem.hasChildren,
            metadata: colItem.metadata,
          })),
          parentPath: currentPath,
        });
      }

      // If it's an element, load details
      if (column.type === 'elements') {
        const details = await getElementDetails(currentNodeId, currentPath);
        
        // Store element details in the store
        millerColumnsStore.setElementDetails({
          name: details.name,
          description: details.description,
          dataType: details.dataType,
          fullPath: details.fullPath,
          elementPath: details.elementPath,
          constraints: details.constraints,
          defaultValue: details.defaultValue,
          memoryAddress: details.memoryAddress,
        });
      } else {
        // Clear element details if not selecting an element
        millerColumnsStore.setElementDetails(null);
      }

      millerColumnsStore.setLoading(false);
      isLoadingColumn = false;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      
      // T098: Check if it's a parsing issue
      if (errorMsg.includes('parse') || errorMsg.includes('XML') || errorMsg.includes('Invalid')) {
        hasParsingIssue = true;
        parsingIssueMessage = errorMsg;
      }
      
      millerColumnsStore.setError(`Failed to load column items: ${errorMsg}`);
      console.error('[NavigationColumn] Failed to handle item selection:', err);
      isLoadingColumn = false;
    }
  }

  /**
   * Get column title based on type
   */
  function getColumnTitle(type: string): string {
    switch (type) {
      case 'segments':
        return 'Segments';
      case 'groups':
        return 'Groups';
      case 'elements':
        return 'Elements';
      default:
        return 'Items';
    }
  }
</script>

<div class="navigation-column" role="navigation" aria-label="{getColumnTitle(column.type)} column">
  <div class="column-header">
    <h3>{getColumnTitle(column.type)}</h3>
    {#if column.items.length > 0}
      <span class="count" aria-label="{column.items.length} items">{column.items.length}</span>
    {/if}
  </div>

  <div class="column-content" on:keydown={handleKeyDown} role="group" aria-label="{getColumnTitle(column.type)} items">
    {#if hasParsingIssue}
      <!-- T098: Parsing issue indicator -->
      <div class="parsing-issue" role="alert">
        <div class="issue-icon">⚠️</div>
        <h4>Parsing Issue</h4>
        <p>{parsingIssueMessage}</p>
      </div>
    {:else if column.items.length === 0}
      <div class="empty">
        <p>No items</p>
      </div>
    {:else}
      <ul class="items-list" bind:this={listElement} role="listbox" aria-label="{getColumnTitle(column.type)}">
        {#each column.items as item, index (item.id)}
          <li
            class="item"
            class:selected={selectedItemId === item.id}
            class:keyboard-selected={index === selectedIndex}
            on:click={() => {
              selectedIndex = index;
              handleItemSelect(item);
            }}
            on:keydown={(e) => e.key === 'Enter' && handleItemSelect(item)}
            role="option"
            aria-selected={selectedItemId === item.id}
            tabindex={index === selectedIndex ? 0 : -1}
            title={item.fullName || item.name}
          >
            <span class="item-content">
              {#if column.type === 'elements' && item.type}
                <span class="element-icon" title={item.type}>
                  {getElementIcon(item.type)}
                </span>
              {/if}
              <span class="item-name" title={item.fullName || item.name}>
                {item.name}
              </span>
              {#if item.metadata?.replicated && item.metadata?.instanceNumber}
                <span class="instance-badge" title="Instance {item.metadata.instanceNumber} of {item.metadata.totalInstances}">
                  #{item.metadata.instanceNumber}
                </span>
              {/if}
            </span>
            {#if item.hasChildren}
              <span class="chevron">›</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .navigation-column {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 200px;
    max-width: 300px;
    border-right: 1px solid var(--border-color, #ddd);
    background-color: var(--column-bg, #fafafa);
  }

  .column-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color, #ddd);
    background-color: var(--header-bg, #fff);
  }

  .column-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #333);
  }

  .count {
    font-size: 12px;
    color: var(--text-secondary, #666);
    background-color: var(--badge-bg, #e0e0e0);
    padding: 2px 8px;
    border-radius: 10px;
  }

  .column-content {
    flex: 1;
    overflow-y: auto;
  }

  .empty {
    padding: 24px 16px;
    text-align: center;
    color: var(--text-secondary, #666);
  }

  .items-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    cursor: pointer;
    border-bottom: 1px solid var(--item-border, #efefef);
    transition: background-color 0.15s;
  }

  .item:hover {
    background-color: var(--item-hover, #f0f0f0);
  }

  .item.selected {
    background-color: var(--item-selected, #e3f2fd);
    border-left: 3px solid var(--primary-color, #1976d2);
    padding-left: 13px;
  }

  /* T106: Keyboard navigation highlight */
  .item.keyboard-selected {
    outline: 2px solid var(--primary-color, #1976d2);
    outline-offset: -2px;
  }

  .item-content {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    overflow: hidden;
  }

  .element-icon {
    font-size: 16px;
    flex-shrink: 0;
  }

  .item-name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .instance-badge {
    margin-left: 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--badge-text, #1976d2);
    background-color: var(--badge-bg-light, #e3f2fd);
    padding: 2px 6px;
    border-radius: 8px;
    flex-shrink: 0;
  }

  .chevron {
    margin-left: 8px;
    font-size: 18px;
    color: var(--text-secondary, #666);
    flex-shrink: 0;
  }

  /* T098: Parsing issue indicator */
  .parsing-issue {
    text-align: center;
    padding: 24px 16px;
  }

  .parsing-issue .issue-icon {
    font-size: 32px;
    margin-bottom: 12px;
  }

  .parsing-issue h4 {
    margin: 0 0 8px 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--error-color, #d32f2f);
  }

  .parsing-issue p {
    margin: 0;
    font-size: 12px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
  }
</style>
