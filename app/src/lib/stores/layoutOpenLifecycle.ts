import { derived, writable } from 'svelte/store';

export type LayoutOpenPhase =
  | 'idle'
  | 'opening_file'
  | 'hydrating_snapshots'
  | 'replaying_offline_changes'
  | 'ready'
  | 'error';

export const layoutOpenPhase = writable<LayoutOpenPhase>('idle');

export function setLayoutOpenPhase(phase: LayoutOpenPhase): void {
  layoutOpenPhase.set(phase);
}

export const layoutOpenInProgress = derived(layoutOpenPhase, (phase) => (
  phase !== 'idle' && phase !== 'ready' && phase !== 'error'
));

export const layoutOpenStatusText = derived(layoutOpenPhase, (phase) => {
  switch (phase) {
    case 'opening_file':
      return 'Opening layout file...';
    case 'hydrating_snapshots':
      return 'Hydrating captured nodes...';
    case 'replaying_offline_changes':
      return 'Replaying offline edits...';
    default:
      return '';
  }
});
