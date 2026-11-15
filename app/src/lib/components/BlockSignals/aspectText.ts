/** Product wording for the Automatic Block Signals rules, ends and signals. */
import type {
  EntranceEnd,
  ExitEnd,
  ListedBlock,
  RuleCondition,
  SignalIndication,
  SignalPreview,
} from '$lib/api/signaling';

export const INDICATION_LABEL: Record<SignalIndication, string> = {
  stop: 'Stop',
  restricting: 'Restricting',
  approach: 'Approach',
  advanceApproach: 'Advance Approach',
  clear: 'Clear',
};

/** CSS tone for an indication cell. */
export function indicationTone(indication: SignalIndication): 'stop' | 'approach' | 'clear' {
  if (indication === 'clear') return 'clear';
  if (indication === 'approach' || indication === 'advanceApproach') return 'approach';
  return 'stop';
}

export function turnoutName(blocks: ListedBlock[], turnoutId: string): string {
  for (const block of blocks) {
    const turnout = block.turnouts.find((t) => t.turnoutId === turnoutId);
    if (turnout) return turnout.name;
  }
  return turnoutId;
}

export function conditionText(condition: RuleCondition, blocks: ListedBlock[], protectedCount: number): string {
  const occupied = protectedCount > 1 ? 'A protected block occupied' : 'Protected block occupied';
  const clear = protectedCount > 1 ? 'Protected blocks clear' : 'Protected block clear';
  switch (condition.kind) {
    case 'blockOccupied': return occupied;
    case 'turnoutNotLined':
      return `${turnoutName(blocks, condition.turnoutId)} not lined for the main, or position unknown`;
    case 'blockClear': return clear;
    case 'clearNothingBeyondTreatedAsStop': return `${clear}; nothing beyond, treated as Stop`;
    case 'clearNextAtStop': return `${clear}; next signal at Stop`;
    case 'clearNextAtApproach': return `${clear}; next signal at Approach`;
    case 'clearNextPermissive': return `${clear}; next signal permissive`;
  }
}

/** What the signal reads, for the context line under the name. */
export function nextSignalText(signal: SignalPreview, signals: SignalPreview[]): string {
  switch (signal.next.kind) {
    case 'notUsed': return 'occupancy only';
    case 'nothingBeyond': return 'nothing beyond · Stop / Clear only';
    case 'treatedAsStop': return 'nothing beyond · treated as Stop';
    case 'plantEntrance': return 'Plant entrance';
    case 'signal': {
      const { signalId } = signal.next;
      return signals.find((s) => s.signalId === signalId)?.name ?? signalId;
    }
  }
}

/** A note under the rules table explaining a fallback, or null. */
export function rulesNote(signal: SignalPreview): string | null {
  if (signal.next.kind === 'treatedAsStop') {
    return 'Nothing beyond the last block is treated as Stop, so this signal shows Approach at best.';
  }
  if (signal.next.kind === 'nothingBeyond') {
    return 'Nothing beyond the last block, so this signal shows Stop / Clear only.';
  }
  if (signal.next.kind === 'plantEntrance') {
    return 'The Plant entrance supplies Stop or a permissive restriction; its route indications fold into Approach here.';
  }
  if (signal.settings.aspects >= 4 && !signal.rules.some((rule) => rule.indication === 'advanceApproach')) {
    return 'Advance Approach needs a next signal that itself shows Approach; none is available here.';
  }
  return null;
}

/** The label on an end chip, or null when nothing is set ("+ Plant"). */
export function endChipLabel(end: 'start' | 'end', entrance: EntranceEnd, exit: ExitEnd): string | null {
  const value = end === 'start' ? entrance : exit;
  if (value.kind === 'plant') return 'Plant';
  if (value.kind === 'treatAsStop') return 'End: Stop';
  return null;
}

/** What lies beyond the last block, in a sentence fragment. */
export function exitEndText(exit: ExitEnd): string {
  switch (exit.kind) {
    case 'nothingBeyond': return 'nothing beyond these blocks';
    case 'treatAsStop': return 'nothing beyond, treated as Stop';
    case 'plant': return 'a Plant entrance';
  }
}
