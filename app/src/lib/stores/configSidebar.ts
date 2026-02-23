/**
 * Configuration Sidebar State Management
 *
 * Manages sidebar navigation state (expanded nodes, selected segment)
 * and the element card deck state for the selected segment.
 *
 * Feature: 005-config-sidebar-view
 */

import { writable } from 'svelte/store';

// ---------------------------------------------------------------------------
// Backend response types (mirror get_card_elements contract)
// ---------------------------------------------------------------------------

export interface CardField {
  /** Full CDI path from root; used as cache key for readConfigValue */
  elementPath: string[];
  /** CDI element name; display label */
  name: string;
  /** CDI description text; null if absent */
  description: string | null;
  /** Element type — determines FieldRow vs EventSlotRow */
  dataType: 'int' | 'string' | 'eventid' | 'float' | 'action' | 'blob';
  /** Absolute memory address within the node's address space */
  memoryAddress: number;
  /** Value size in bytes */
  sizeBytes: number;
  /** CDI default value as string; null if not specified */
  defaultValue: string | null;
  /** LCC memory space number (matches parent segment's space) */
  addressSpace: number;
}

export interface CardSubGroup {
  name: string;
  description: string | null;
  /** Full path to this sub-group from CDI root */
  groupPath: string[];
  fields: CardField[];
  subGroups: CardSubGroup[]; // Recursive — arbitrary CDI depth
  /** CDI replication count. > 1: one instance of a replicated group → accordion.
   *  === 1: non-replicated group → render inline (always visible). */
  replication: number;
}

export interface CardElementTree {
  groupName: string | null;
  groupDescription: string | null;
  /** Leaf elements directly in this group, in CDI document order */
  fields: CardField[];
  /** Child sub-groups, rendered inline and fully expanded (FR-011) */
  subGroups: CardSubGroup[];
}

// ---------------------------------------------------------------------------
// Segment info (derived from get_cdi_structure response)
// ---------------------------------------------------------------------------

export interface SegmentInfo {
  segmentId: string;
  /** CDI navigation path used by get_column_items, e.g. "seg:0" */
  segmentPath: string;
  segmentName: string;
  description: string | null;
  space: number;
}

// ---------------------------------------------------------------------------
// Card deck state
// ---------------------------------------------------------------------------

export interface CardData {
  /** Serialized group path — unique within card deck */
  cardId: string;
  /** Full CDI navigation path from segment root to this group */
  groupPath: string[];
  /** Raw CDI group name; shown in parentheses per FR-007 */
  cdGroupName: string;
  /** True when group.replication > 1 */
  isReplicated: boolean;
  /** 1-based instance number; null for non-replicated groups */
  instanceIndex: number | null;
  /** Computed per FR-007 naming algorithm (see resolveCardTitle utility) */
  cardTitle: string;
  /** Null until card is first expanded (lazy load via get_card_elements) */
  elements: CardElementTree | null;
  /** True while get_card_elements is in flight */
  isLoading: boolean;
  /** Error message if element load failed; null on success */
  loadError: string | null;
}

export interface ConfigSidebarCardDeck {
  nodeId: string;
  segmentId: string;
  segmentName: string;
  /** All top-level group cards for this segment */
  cards: CardData[];
  /** Card IDs currently expanded (default: none — FR-008) */
  expandedCardIds: string[];
  /** True while top-level groups are being fetched */
  isLoading: boolean;
  /** Error loading the card deck; null on success */
  error: string | null;
}

// ---------------------------------------------------------------------------
// Store state
// ---------------------------------------------------------------------------

export interface ConfigSidebarState {
  /**
   * Node IDs whose segment list is currently expanded.
   * FR-015: Preserved across segment selections within a session.
   */
  expandedNodeIds: string[];

  /** The currently selected segment (one at a time globally) */
  selectedSegment: { nodeId: string; segmentId: string; segmentPath: string } | null;

  /** Card deck for the selected segment; null when no segment is selected */
  cardDeck: ConfigSidebarCardDeck | null;

  /**
   * Segment loading state per node.
   * 'loading' while getCdiStructure is in flight after first node expansion.
   */
  nodeLoadingStates: Record<string, 'idle' | 'loading' | 'error'>;

  /** Per-node error messages; null = no error */
  nodeErrors: Record<string, string | null>;
}

// ---------------------------------------------------------------------------
// Initial state
// ---------------------------------------------------------------------------

const initialState: ConfigSidebarState = {
  expandedNodeIds: [],
  selectedSegment: null,
  cardDeck: null,
  nodeLoadingStates: {},
  nodeErrors: {},
};

// ---------------------------------------------------------------------------
// Store factory
// ---------------------------------------------------------------------------

function createConfigSidebarStore() {
  const { subscribe, set, update } = writable<ConfigSidebarState>(initialState);

  return {
    subscribe,

    /**
     * Toggle expansion of a node entry in the sidebar.
     * FR-015: Expanding does NOT collapse previously expanded nodes.
     */
    toggleNodeExpanded(nodeId: string): void {
      update(state => {
        const isExpanded = state.expandedNodeIds.includes(nodeId);
        return {
          ...state,
          expandedNodeIds: isExpanded
            ? state.expandedNodeIds.filter(id => id !== nodeId)
            : [...state.expandedNodeIds, nodeId],
        };
      });
    },

    /** Record that a node's segments have loaded successfully */
    setNodeSegments(_nodeId: string, _segments: SegmentInfo[]): void {
      // Segments are stored on the CDI structure response via getCdiStructure,
      // not in this store; this action just clears the loading state.
      update(state => ({
        ...state,
        nodeLoadingStates: {
          ...state.nodeLoadingStates,
          [_nodeId]: 'idle',
        },
        nodeErrors: {
          ...state.nodeErrors,
          [_nodeId]: null,
        },
      }));
    },

    /** Mark a node's segment load as in-progress or failed */
    setNodeLoading(nodeId: string, status: 'idle' | 'loading' | 'error', error?: string): void {
      update(state => ({
        ...state,
        nodeLoadingStates: {
          ...state.nodeLoadingStates,
          [nodeId]: status,
        },
        nodeErrors: {
          ...state.nodeErrors,
          [nodeId]: error ?? null,
        },
      }));
    },

    /**
     * Select a segment — triggers card deck load.
     * FR-005: Replaces any previously shown card deck.
     */
    selectSegment(nodeId: string, segmentId: string, segmentName: string, segmentPath: string): void {
      update(state => ({
        ...state,
        selectedSegment: { nodeId, segmentId, segmentPath },
        cardDeck: {
          nodeId,
          segmentId,
          segmentName,
          cards: [],
          expandedCardIds: [],
          isLoading: true,
          error: null,
        },
      }));
    },

    /** Populate the card deck after top-level group data is fetched */
    setCards(nodeId: string, segmentId: string, cards: CardData[]): void {
      update(state => {
        if (
          !state.cardDeck ||
          state.cardDeck.nodeId !== nodeId ||
          state.cardDeck.segmentId !== segmentId
        ) {
          return state;
        }
        return {
          ...state,
          cardDeck: {
            ...state.cardDeck,
            cards,
            isLoading: false,
            error: null,
          },
        };
      });
    },

    /** Set card deck loading state and optional error */
    setCardDeckLoading(loading: boolean, error?: string | null): void {
      update(state => {
        if (!state.cardDeck) return state;
        return {
          ...state,
          cardDeck: {
            ...state.cardDeck,
            isLoading: loading,
            error: error ?? null,
          },
        };
      });
    },

    /**
     * Toggle a card's expanded state.
     */
    toggleCardExpanded(cardId: string): void {
      update(state => {
        if (!state.cardDeck) return state;
        const isExpanded = state.cardDeck.expandedCardIds.includes(cardId);
        return {
          ...state,
          cardDeck: {
            ...state.cardDeck,
            expandedCardIds: isExpanded
              ? state.cardDeck.expandedCardIds.filter(id => id !== cardId)
              : [...state.cardDeck.expandedCardIds, cardId],
          },
        };
      });
    },

    /** Store fetched elements for a card (called when get_card_elements resolves) */
    updateCard(cardId: string, patch: Partial<Pick<CardData, 'elements' | 'isLoading' | 'loadError'>>): void {
      update(state => {
        if (!state.cardDeck) return state;
        return {
          ...state,
          cardDeck: {
            ...state.cardDeck,
            cards: state.cardDeck.cards.map(card =>
              card.cardId === cardId ? { ...card, ...patch } : card
            ),
          },
        };
      });
    },

    /** Store fetched elements for a card (called when get_card_elements resolves) */
    setCardElements(cardId: string, elements: CardElementTree): void {
      update(state => {
        if (!state.cardDeck) return state;
        return {
          ...state,
          cardDeck: {
            ...state.cardDeck,
            cards: state.cardDeck.cards.map(card =>
              card.cardId === cardId
                ? { ...card, elements, isLoading: false, loadError: null }
                : card
            ),
          },
        };
      });
    },

    /** Mark a specific card element load as failed */
    setCardElementsError(cardId: string, error: string): void {
      update(state => {
        if (!state.cardDeck) return state;
        return {
          ...state,
          cardDeck: {
            ...state.cardDeck,
            cards: state.cardDeck.cards.map(card =>
              card.cardId === cardId
                ? { ...card, isLoading: false, loadError: error }
                : card
            ),
          },
        };
      });
    },

    /**
     * Clear all state.
     * FR-018: Called when Discover/Refresh Nodes is triggered.
     */
    reset(): void {
      set(initialState);
    },
  };
}

/** Singleton configSidebarStore instance */
export const configSidebarStore = createConfigSidebarStore();
