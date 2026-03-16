/**
 * TypeScript types for editable bowties (Feature 009).
 *
 * These types mirror the Rust layout types and define the frontend
 * state model for bowtie creation, editing, and persistence.
 */

// ── Layout File Types (YAML persistence) ─────────────────────────────────────

/** Root structure matching YAML layout file */
export interface LayoutFile {
  schemaVersion: string;
  bowties: Record<string, BowtieMetadata>;
  roleClassifications: Record<string, RoleClassification>;
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
  elementPath: string[];
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
}
