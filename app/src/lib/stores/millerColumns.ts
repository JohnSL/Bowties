/**
 * Miller Columns State Management
 * 
 * Manages the state for the Miller Columns navigation interface,
 * including node selection, column management, and navigation breadcrumb.
 */

import { writable } from 'svelte/store';
import type { ConfigValueMap, ReadProgressState } from '$lib/api/types';

// T101: Debounce timeout for navigation clicks
let debounceTimeout: ReturnType<typeof setTimeout> | null = null;
const DEBOUNCE_DELAY = 50; // ms

/**
 * Navigation step in the breadcrumb
 */
export interface NavigationStep {
    /** Hierarchy depth (0 = nodes, 1 = segments, 2+ = groups/elements) */
    depth: number;
    
    /** Unique identifier for selected item at this depth */
    itemId: string;
    
    /** Type of item (determines next column type) */
    itemType: 'node' | 'segment' | 'group' | 'element';
    
    /** User-visible label for breadcrumb */
    label: string;
}

/**
 * Content for a single column
 */
export interface ColumnData {
    /** Column position (0 = nodes, 1 = segments, 2+ = groups/elements) */
    depth: number;
    
    /** Column type (determines rendering behavior) */
    type: 'nodes' | 'segments' | 'groups' | 'elements';
    
    /** Items to display in this column */
    items: ColumnItem[];
    
    /** Path to parent (for context and caching) */
    parentPath: string[];
}

/**
 * Selectable item within a column
 */
export interface ColumnItem {
    /** Unique identifier (used for selection and caching) */
    id: string;
    
    /** Display name (may be truncated for long names) */
    name: string;
    
    /** Full name for tooltips */
    fullName?: string;
    
    /** Data type for elements (e.g., "eventid", "int", "string") */
    type?: string;
    
    /** Whether this item has children (determines if clicking adds column) */
    hasChildren: boolean;
    
    /** Additional metadata (instance number, constraints, etc.) */
    metadata?: Record<string, unknown>;
}

/**
 * Miller Columns state interface
 */
export interface MillerColumnsState {
    /** Currently selected node (null if no node selected) */
    selectedNode: {
        nodeId: string;
        nodeName: string;
    } | null;
    
    /** Active columns (dynamic array, grows/shrinks with navigation) */
    columns: ColumnData[];
    
    /** Navigation breadcrumb (path from root to current selection) */
    breadcrumb: NavigationStep[];
    
    /** Selected element details (null if no element selected) */
    selectedElementDetails: ElementDetails | null;
    
    /** Loading state */
    isLoading: boolean;
    
    /** Error message (null if no error) */
    error: string | null;
    
    /** Config values cache (T020) - Map<cacheKey, value> where cacheKey = nodeId:elementPath */
    configValues: ConfigValueMap;
    
    /** Read progress state (T020) - null when not reading */
    readProgress: ReadProgressState | null;
    
    /** Cancelling state (T020) - true when cancellation requested */
    isCancelling: boolean;
}

/**
 * Element details for the Details Panel
 */
export interface ElementDetails {
    name: string;
    description: string | null;
    dataType: string;
    fullPath: string;
    elementPath: string[];
    constraints: Constraint[];
    defaultValue: string | null;
    memoryAddress: number;
}

/**
 * Constraint information
 */
export interface Constraint {
    type: 'range' | 'map' | 'length';
    description: string;
    value: {
        min?: number;
        max?: number;
        entries?: Array<{ value: number; label: string }>;
        maxLength?: number;
    };
}

/**
 * Initial state
 */
const initialState: MillerColumnsState = {
    selectedNode: null,
    columns: [],
    breadcrumb: [],
    selectedElementDetails: null,
    isLoading: false,
    error: null,
    configValues: new Map(),  // T021: Initialize empty cache
    readProgress: null,       // T022: No progress initially
    isCancelling: false,      // T023: Not cancelling initially
};

/**
 * Create the Miller Columns store
 */
function createMillerColumnsStore() {
    const { subscribe, set, update } = writable<MillerColumnsState>(initialState);
    
    return {
        subscribe,
        
        /**
         * Select a node (reset columns, trigger segment load)
         * 
         * @param nodeId - Node ID in dotted hex format
         * @param nodeName - User-visible node name
         */
        selectNode: (nodeId: string, nodeName: string) => {
            update(state => ({
                ...state,
                selectedNode: { nodeId, nodeName },
                columns: [],
                breadcrumb: [{
                    depth: 0,
                    itemId: nodeId,
                    itemType: 'node',
                    label: nodeName,
                }],
                error: null,
            }));
        },
        
        /**
         * Add a new column to the navigation
         * T101: Debounced to prevent rapid navigation clicks
         * 
         * @param column - Column data to add
         */
        addColumn: (column: ColumnData) => {
            // Clear any pending debounce
            if (debounceTimeout) {
                clearTimeout(debounceTimeout);
            }
            
            // Debounce the column addition
            debounceTimeout = setTimeout(() => {
                update(state => ({
                    ...state,
                    columns: [...state.columns, column],
                }));
                debounceTimeout = null;
            }, DEBOUNCE_DELAY);
        },
        
        /**
         * Remove all columns after a specific depth (navigation back support)
         * 
         * @param depth - Depth to keep (removes all columns after this depth)
         */
        removeColumnsAfter: (depth: number) => {
            update(state => ({
                ...state,
                columns: state.columns.filter(col => col.depth <= depth),
                breadcrumb: state.breadcrumb.filter(step => step.depth <= depth),
            }));
        },
        
        /**
         * Update the breadcrumb with a new navigation step
         * 
         * @param step - Navigation step to add
         */
        updateBreadcrumb: (step: NavigationStep) => {
            update(state => {
                // Remove any steps at or after this depth
                const newBreadcrumb = state.breadcrumb.filter(s => s.depth < step.depth);
                
                // T083: Enhance label with instance number for replicated groups
                let enhancedStep = { ...step };
                
                return {
                    ...state,
                    breadcrumb: [...newBreadcrumb, enhancedStep],
                };
            });
        },
        
        /**
         * Set loading state
         * 
         * @param isLoading - Loading state
         */
        setLoading: (isLoading: boolean) => {
            update(state => ({
                ...state,
                isLoading,
            }));
        },
        
        /**
         * Set error state
         * 
         * @param error - Error message (null to clear error)
         */
        setError: (error: string | null) => {
            update(state => ({
                ...state,
                error,
                isLoading: false,
            }));
        },
        
        /**
         * Set selected element details
         * 
         * @param details - Element details (null to clear)
         */
        setElementDetails: (details: ElementDetails | null) => {
            update(state => ({
                ...state,
                selectedElementDetails: details,
            }));
        },
        
        /**
         * Set a single config value in cache (T040)
         * 
         * @param nodeId - Node ID
         * @param elementPath - Element path array
         * @param value - Config value with metadata
         */
        setConfigValue: (nodeId: string, elementPath: string[], value: any) => {
            update(state => {
                const cacheKey = `${nodeId}:${elementPath.join('/')}`;
                const newCache = new Map(state.configValues);
                newCache.set(cacheKey, value);
                return {
                    ...state,
                    configValues: newCache,
                };
            });
        },
        
        /**
         * Get a config value from cache (T041)
         * 
         * @param nodeId - Node ID
         * @param elementPath - Element path array
         * @returns Config value or null if not cached
         */
        getConfigValue: (nodeId: string, elementPath: string[]) => {
            const cacheKey = `${nodeId}:${elementPath.join('/')}`;
            const state = writable<MillerColumnsState>(initialState);
            let currentState: MillerColumnsState;
            const unsubscribe = state.subscribe(s => currentState = s);
            unsubscribe();
            return currentState!.configValues.get(cacheKey) ?? null;
        },
        
        /**
         * Set multiple config values in batch (T059)
         * 
         * @param values - Record of cache keys to values
         */
        setConfigValues: (values: Record<string, any>) => {
            update(state => {
                const newCache = new Map(state.configValues);
                Object.entries(values).forEach(([key, value]) => {
                    newCache.set(key, value);
                });
                return {
                    ...state,
                    configValues: newCache,
                };
            });
        },
        
        /**
         * Set read progress state (T060)
         * 
         * @param progress - Progress state (null to clear)
         */
        setReadProgress: (progress: ReadProgressState | null) => {
            update(state => ({
                ...state,
                readProgress: progress,
            }));
        },
        
        /**
         * Clear all config values from cache (T061)
         */
        clearConfigValues: () => {
            update(state => ({
                ...state,
                configValues: new Map(),
            }));
        },
        
        /**
         * Set cancelling state (T062)
         * 
         * @param isCancelling - Whether cancellation is in progress
         */
        setCancelling: (isCancelling: boolean) => {
            update(state => ({
                ...state,
                isCancelling,
            }));
        },
        
        /**
         * Reset the store to initial state
         */
        reset: () => {
            set(initialState);
        },
    };
}

/**
 * Miller Columns store instance
 */
export const millerColumnsStore = createMillerColumnsStore();
