import { invoke } from '@tauri-apps/api/core';
import type { PanelRecord } from '$lib/api/facilities';
import type { RemovalEdit } from '$lib/api/signaling';

/** Who holds a board pin, tagged by `kind`. */
export type PinOwner =
  | { kind: 'free' }
  | { kind: 'panel'; facilityId: string; name: string; row: number }
  | { kind: 'facility'; facilityId: string; name: string }
  | { kind: 'outside'; row: number | null; description: string };

export interface PanelWiringPin {
  output: number;
  /** "#9 H3-G". */
  label: string;
  /** The connector group the pin belongs to, e.g. "H3". */
  group: string;
  owner: PinOwner;
}

export interface PanelWiringBoard {
  nodeKey: string;
  name: string;
  pins: PanelWiringPin[];
}

export interface PanelWiringFacts {
  boards: PanelWiringBoard[];
  /** Boards whose pins could not be read. */
  issues: { nodeKey: string; detail: string }[];
}

/** One lamp to wire: the entry it lights for and the pin it sits on. */
export interface PanelLampRequest {
  entryId: string;
  nodeKey: string;
  output: number;
  /** The Direct Lamp row the lamp already uses, when re-composing a saved panel. */
  currentRow?: number;
  name: string;
}

export interface ComposePanelRequest {
  facilityId: string;
  record: PanelRecord;
  lamps: PanelLampRequest[];
}

export interface PanelRowAssignment {
  entryId: string;
  nodeKey: string;
  row: number;
}

export type ComposePanelIssueCode =
  | 'pinOwnedElsewhere'
  | 'pinOutsideNotReleased'
  | 'duplicatePin'
  | 'blockEventsUnresolved'
  | 'boardUnavailable'
  | 'noFreeLampRow';

export interface ComposePanelIssue {
  code: ComposePanelIssueCode;
  entryId: string;
  nodeKey: string;
  output: number;
  detail: string;
}

/** With any issue, `edits` and `assignments` are empty. */
export interface ComposePanelResponse {
  edits: RemovalEdit[];
  assignments: PanelRowAssignment[];
  issues: ComposePanelIssue[];
}

/** Read-only: every pin on the named boards (all boards when omitted) with its owner. */
export async function panelWiringFacts(nodeKeys?: string[]): Promise<PanelWiringFacts> {
  const request = nodeKeys ? { nodeKeys } : {};
  return invoke<PanelWiringFacts>('panel_wiring_facts', { request });
}

/** Read-only: generates the edits for the caller to stage; writes nothing. */
export async function composePanel(request: ComposePanelRequest): Promise<ComposePanelResponse> {
  return invoke<ComposePanelResponse>('compose_panel', { request });
}
