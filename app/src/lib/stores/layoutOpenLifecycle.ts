import { derived, writable } from 'svelte/store';

export type LayoutOpenPhase =
  | 'idle'
  | 'opening_file'
  | 'hydrating_snapshots'
  | 'replaying_offline_changes'
  | 'ready'
  | 'error';

export const layoutOpenPhase = writable<LayoutOpenPhase>('idle');

const allowedTransitions: Record<LayoutOpenPhase, LayoutOpenPhase[]> = {
  idle: ['opening_file'],
  opening_file: ['hydrating_snapshots', 'error', 'idle'],
  hydrating_snapshots: ['replaying_offline_changes', 'error', 'idle'],
  replaying_offline_changes: ['ready', 'error', 'idle'],
  ready: ['opening_file', 'idle'],
  error: ['idle', 'opening_file'],
};

export function setLayoutOpenPhase(phase: LayoutOpenPhase): void {
  layoutOpenPhase.update((current) => {
    if (current !== phase && !allowedTransitions[current].includes(phase)) {
      console.warn(`[layout-open] Unexpected phase transition ${current} -> ${phase}`);
    }
    return phase;
  });
}

export function resetLayoutOpenPhase(): void {
  setLayoutOpenPhase('idle');
}

export function startLayoutOpen(): void {
  setLayoutOpenPhase('opening_file');
}

export function startLayoutHydration(): void {
  setLayoutOpenPhase('hydrating_snapshots');
}

export function finishLayoutHydration(): void {
  setLayoutOpenPhase('replaying_offline_changes');
}

export function startOfflineReplay(): void {
  setLayoutOpenPhase('replaying_offline_changes');
}

export function finishOfflineReplay(): void {
  setLayoutOpenPhase('ready');
}

export function failLayoutOpen(): void {
  setLayoutOpenPhase('error');
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
