/**
 * Frontend TypeScript types for the Bowties Tab — Discover Existing Connections feature.
 *
 * These are the canonical types used in:
 *   - app/src/lib/api/tauri.ts       (Tauri command wrappers)
 *   - app/src/lib/stores/bowties.ts  (Svelte stores)
 *   - app/src/lib/components/Bowtie/ (UI components)
 *
 * Mirror of Rust types in app/src-tauri/src/commands/bowties.rs
 */

// ─── Core entities ──────────────────────────────────────────────────────────

/** The role of an event slot as determined by the classification pipeline */
export type EventRole = 'Producer' | 'Consumer' | 'Ambiguous';

// Note: 'Ambiguous' entries are only present in BowtieCard.ambiguous_entries.
// Entries in BowtieCard.producers always have role 'Producer';
// entries in BowtieCard.consumers always have role 'Consumer'.

/**
 * A single classified event ID field from one node.
 * Ambiguous slots are never included — they are filtered during catalog build.
 */
export interface EventSlotEntry {
  /** Node identifier in dotted-hex format, e.g. "02.01.57.00.00.01" */
  node_id: string;

  /** Human-readable node name (priority: SNIP user_name → "{mfg} — {model}" → node_id) */
  node_name: string;

  /**
   * Full CDI path from segment root to this element.
   * e.g. ["Segment 1", "Producers", "Track 1", "Output Active"]
   */
  element_path: string[];

  /**
   * Display label shown in the element entry card.
   * Priority (per CDI spec + research.md RQ-12):
   *   1. CDI element `<name>` (non-empty)
   *   2. First sentence of CDI element `<description>` (non-empty)
   *   3. Slash-joined element_path as fallback
   *
   * This is conceptually equivalent to JMRI's "Also Known As" column, but
   * sourced directly from CDI rather than from JMRI-managed turnout/sensor objects.
   */
  element_label: string;

  /** The 8-byte event ID value currently stored in this slot */
  event_id: number[];   // always length 8

  /** Classified role */
  role: EventRole;
}

/**
 * A bowtie card — one shared event ID with ≥1 confirmed producer and ≥1 confirmed consumer.
 *
 * Invariants:
 *   - producers.length >= 1
 *   - consumers.length >= 1
 *   - event_id_bytes.length === 8
 *   - ambiguous_entries may be empty (most cards will have none)
 */
export interface BowtieCard {
  /** Event ID in dotted-hex notation, e.g. "05.02.01.02.03.00.00.01" — used as unique key */
  event_id_hex: string;

  /** Raw 8-byte event ID (for sorting / lookups) */
  event_id_bytes: number[];

  /** All confirmed producer slots (≥1) — role determined by Identify Events protocol reply */
  producers: EventSlotEntry[];

  /** All confirmed consumer slots (≥1) — role determined by Identify Events protocol reply */
  consumers: EventSlotEntry[];

  /**
   * Slots whose role could not be determined.
   * These are from nodes that replied BOTH Producer Identified AND Consumer Identified
   * for this event, AND whose CDI heuristic was also inconclusive.
   * Shown in the card as "Unknown role — needs clarification".
   * Future phase: user can assign a role and the decision will be persisted.
   */
  ambiguous_entries: EventSlotEntry[];

  /**
   * User-assigned name, or null if not yet named.
   * Out of scope in this phase — always null from backend.
   * Card header shows event_id_hex when null (FR-014).
   */
  name: string | null;
}

/** Derived display name for a BowtieCard (FR-014) */
export function bowtieName(card: BowtieCard): string {
  return card.name ?? card.event_id_hex;
}

/**
 * The complete bowtie catalog for the current session.
 * Rebuilt atomically after each full configuration refresh.
 */
export interface BowtieCatalog {
  /** All bowties sorted by event_id_bytes (lexicographic) */
  bowties: BowtieCard[];

  /** ISO 8601 timestamp of last rebuild */
  built_at: string;

  /** Number of nodes whose data was included */
  source_node_count: number;

  /** Total event slots scanned (including excluded ambiguous / unmatched) */
  total_slots_scanned: number;
}

// ─── Tauri event payloads ────────────────────────────────────────────────────

/**
 * Payload of the `cdi-read-complete` Tauri event.
 * Emitted when all CDI reads complete and the catalog has been (re)built.
 */
export interface CdiReadCompletePayload {
  catalog: BowtieCatalog;
  node_count: number;
}

// ─── Navigation / cross-reference ───────────────────────────────────────────

/**
 * Passed as an optional prop to EventSlotRow (FR-008).
 * When present, the slot shows a "Used in: [name]" navigable link.
 */
export interface UsedInRef {
  /** Card header label (bowtieName(card)) */
  label: string;

  /** event_id_hex — used as the `highlight` query parameter when navigating */
  eventIdHex: string;
}
