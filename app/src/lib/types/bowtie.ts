/**
 * TypeScript types for editable bowties (Feature 009).
 *
 * These types mirror the Rust layout types and define the frontend
 * state model for bowtie creation, editing, and persistence.
 */

// ── Layout File Types (YAML persistence) ─────────────────────────────────────

/**
 * Configuration-mode variant selections per node (Spec 014, ADR-0008).
 *
 * Outer key is a `NodeKey` — canonical NodeID (uppercase, no dots) for real
 * nodes or `placeholder:<uuidv4>` for placeholder boards. Inner key is the
 * `ConfigurationMode` id; value is the chosen `VariantId`.
 */
export type LayoutNodeModeSelections = Record<string, Record<string, string>>;

/** Root structure matching YAML layout file */
export interface LayoutFile {
  schemaVersion: string;
  bowties: Record<string, BowtieMetadata>;
  roleClassifications: Record<string, RoleClassification>;
  /**
   * Configuration-mode variant selections (Spec 014 / S6). Replaces the
   * pre-release `connectorSelections` field. Optional because freshly
   * created layouts may omit the field entirely.
   */
  nodeModeSelections?: LayoutNodeModeSelections;
}

/** Metadata for a single bowtie stored in layout YAML */
export interface BowtieMetadata {
  name?: string;
  tags: string[];
}

/** User-provided role for an ambiguous event slot */
export interface RoleClassification {
  role: 'Producer' | 'Consumer';
}

/** Recent layout file reference */
export interface RecentLayout {
  path: string;
  lastOpened: string;
}

// ── Layout Edit Deltas (ADR-0002) ─────────────────────────────────────────────

/**
 * A single edit operation sent to the backend during save.
 *
 * The frontend sends a list of deltas instead of a full LayoutFile.
 * The backend applies them to its disk-authoritative copy (ADR-0002).
 */
export type LayoutEditDelta =
  | { type: 'createBowtie'; eventIdHex: string; name?: string | null }
  | { type: 'deleteBowtie'; eventIdHex: string }
  | { type: 'renameBowtie'; eventIdHex: string; newName: string }
  | { type: 'addTag'; eventIdHex: string; tag: string }
  | { type: 'removeTag'; eventIdHex: string; tag: string }
  | { type: 'classifyRole'; key: string; role: string }
  | { type: 'adoptEventId'; oldEventIdHex: string; newEventIdHex: string }
  /**
   * Promote a node (real or placeholder) into the layout's saved node roster
   * (S8 / S8.11).  `nodeKey` is the canonical NodeID (uppercase hex, no dots)
   * for real nodes or `"placeholder:<uuid>"` for synthesized placeholders.
   */
  | { type: 'addNode'; nodeKey: string }
  /**
   * Remove a previously-persisted node from the layout's saved roster.
   * Symmetric to `addNode` (S8). The backend drops the node from its
   * permitted-write set; the companion `nodes/<key>.yaml` file is then
   * pruned by the normal save flow.
   */
  | { type: 'removeNode'; nodeKey: string }
  /** Upsert a connector slot selection for a node (ADR-0012). */
  | { type: 'setNodeModeSelection'; nodeKey: string; modeId: string; variantId: string }
  /** Clear a connector slot selection (set to "None installed") (ADR-0012). */
  | { type: 'clearNodeModeSelection'; nodeKey: string; modeId: string };

// ── Bowtie State ──────────────────────────────────────────────────────────────

/** State of a bowtie based on its element membership */
export type BowtieState = 'active' | 'incomplete' | 'planning';

// ── Bowtie Edit Types ─────────────────────────────────────────────────────────

/** Discriminated union for bowtie metadata edit operations */
export type BowtieEditKind =
  | { type: 'create'; eventIdHex: string; name?: string }
  | { type: 'delete'; eventIdHex: string }
  | { type: 'rename'; eventIdHex: string; oldName?: string; newName: string }
  | { type: 'addTag'; eventIdHex: string; tag: string }
  | { type: 'removeTag'; eventIdHex: string; tag: string }
  | { type: 'classifyRole'; key: string; role: 'Producer' | 'Consumer' };

/** A tracked bowtie metadata edit with identity and timestamp */
export interface BowtieMetadataEdit {
  id: string;
  kind: BowtieEditKind;
  timestamp: number;
}

// ── Element Selection ─────────────────────────────────────────────────────────

/** A selected element for one side of a connection */
export interface ElementSelection {
  nodeId: string;
  nodeName: string;
  elementPath: string[];
  elementLabel: string;
  address: number;
  space: number;
  currentEventId: string;
}

/** Result of event ID selection rule evaluation */
export interface EventIdResolution {
  eventIdHex: string;
  writeTo: 'producer' | 'consumer' | 'both' | 'none';
  conflictPrompt?: {
    producerBowtie: string;
    consumerBowtie: string;
  };
}

// ── Write Operation Tracking ──────────────────────────────────────────────────

/** Multi-node write operation with rollback support */
export interface WriteOperation {
  id: string;
  steps: WriteStep[];
  status: 'pending' | 'writing' | 'completed' | 'partial-failure' | 'rolled-back' | 'rollback-failed';
}

/** Individual step in a multi-node write */
export interface WriteStep {
  nodeId: string;
  address: number;
  space: number;
  originalValue: number[];
  newValue: number[];
  status: 'pending' | 'writing' | 'success' | 'failed' | 'rolled-back' | 'rollback-failed';
  error?: string;
}

// ── Editable Bowtie Preview ───────────────────────────────────────────────────

/** Derived view merging live catalog + pending edits + metadata */
export interface EditableBowtiePreview {
  bowties: PreviewBowtieCard[];
  hasUnsavedChanges: boolean;
}

/** A bowtie card with dirty-tracking for UI rendering */
export interface PreviewBowtieCard {
  eventIdHex: string;
  eventIdBytes: number[];
  producers: import('$lib/api/tauri').EventSlotEntry[];
  consumers: import('$lib/api/tauri').EventSlotEntry[];
  ambiguousEntries: import('$lib/api/tauri').EventSlotEntry[];
  name?: string;
  tags: string[];
  state: BowtieState;
  isDirty: boolean;
  dirtyFields: Set<string>;
  /** Keys (`${node_id}:${element_path.join('/')}`) of entries added in this session. */
  newEntryKeys: Set<string>;
}
