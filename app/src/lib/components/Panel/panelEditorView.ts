import type { SelectedDetectedBlock } from '$lib/api/signaling';
import type { panelEditorStore } from '$lib/stores/panelEditor.svelte';

/** What the Panel editor components read: the store's public getters. */
export type PanelEditorView = typeof panelEditorStore;

/** A detected block, with the detector board and signals facility when known. */
export type PanelDetectedBlock = SelectedDetectedBlock & { board?: string; facility?: string };
