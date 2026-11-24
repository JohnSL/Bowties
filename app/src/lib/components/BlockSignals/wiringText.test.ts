import { describe, expect, it } from 'vitest';
import type { ComposeIssue, SignalOutputHost } from '$lib/api/signaling';
import { issueText } from './wiringText';

const OUTPUTS: SignalOutputHost['outputs'] = [
  { ordinal: 10, label: '#10 H3-Y', owner: 'Mast 1' },
  { ordinal: 11, label: '#11 H4-R', owner: 'Mast 2', facility: { facilityId: 'fac-1', name: 'Mill Signals' } },
];

const conflict = (output: number, owner: string, withDetail = true): ComposeIssue => ({
  code: 'outputInUse', signalId: 'sig-1', nodeKey: 'N', detail: 'x',
  ...(withDetail ? { bindingIssue: { kind: 'signalOutputConflict', signalId: 'sig-1', nodeKey: 'N', output, owner } } : {}),
}) as ComposeIssue;

describe('issueText outputInUse', () => {
  it('names an owner configured outside Bowties', () => {
    expect(issueText(conflict(10, 'Mast 1'), OUTPUTS)).toBe('#10 H3-Y is used by Mast 1, configured outside Bowties.');
  });

  it('names the Bowties facility that owns the pin', () => {
    expect(issueText(conflict(11, 'Mast 2'), OUTPUTS)).toBe('#11 H4-R is used by Mill Signals.');
  });

  it('falls back to the pin number when the host does not list the pin', () => {
    expect(issueText(conflict(12, 'Mast 4'), OUTPUTS)).toBe('#12 is used by Mast 4, configured outside Bowties.');
  });

  it('keeps the generic text without a binding issue', () => {
    expect(issueText(conflict(10, 'Mast 1', false), OUTPUTS)).toBe('A chosen pin is already in use by another signal. Choose a free pin.');
  });
});
