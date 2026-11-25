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

describe('issueText Logic placement issues', () => {
  const logicIssue = (code: ComposeIssue['code'], logic?: number[]): ComposeIssue => ({
    code, signalId: null, nodeKey: 'N', detail: 'backend detail', ...(logic ? { logic } : {}),
  });

  it('names one Logic or several for a Function value the board does not define', () => {
    expect(issueText(logicIssue('logicUndefinedFunction', [3]), OUTPUTS)).toBe(
      'Logic 3 has a Function value this board does not define. Fix it on the board, or release it, before Bowties places signals here.',
    );
    expect(issueText(logicIssue('logicUndefinedFunction', [3, 5, 7]), OUTPUTS)).toBe(
      'Logic 3, 5 and 7 have a Function value this board does not define. Fix it on the board, or release it, before Bowties places signals here.',
    );
  });

  it('words a group with no end, and an unread Logic', () => {
    expect(issueText(logicIssue('logicUnterminatedGroup', [4, 5]), OUTPUTS)).toBe(
      'Logic 4 and 5 start a group that never ends with a Last (Single) Logic. Fix it on the board, or release it, before Bowties places signals here.',
    );
    expect(issueText(logicIssue('logicUnread', [2]), OUTPUTS)).toBe(
      'Logic 2 could not be read from this board. Check that the node is online, then try again.',
    );
  });

  it('falls back to the backend detail when the issue names no Logic', () => {
    expect(issueText(logicIssue('logicUnread'), OUTPUTS)).toBe('backend detail');
  });
});

describe('issueText two-block issues', () => {
  const issue = (code: ComposeIssue['code'], detail = 'backend detail'): ComposeIssue => ({
    code, signalId: 'sig-1', nodeKey: 'N', detail,
  });

  it('words signals on more than one board and a board with no free Track Circuit', () => {
    expect(issueText(issue('unsupportedBoards'), OUTPUTS)).toBe('This release builds signals on one Signal LCC only. Choose the same Signal LCC for every signal.');
    expect(issueText(issue('noFreeTrackCircuit'), OUTPUTS)).toBe('This Signal LCC has no free Track Circuit left for these signals. Choose another Signal LCC.');
  });

  it('keeps the backend detail for an unread Link Address, which names the next signal', () => {
    const detail = "The Link Address of Next signal's mast has not been read from the board.";
    expect(issueText(issue('linkAddressMissing', detail), OUTPUTS)).toBe(detail);
  });
});
