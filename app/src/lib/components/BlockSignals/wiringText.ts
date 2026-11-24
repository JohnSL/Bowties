/** Product wording for the problems found when checking the signals' wiring. */
import type { ComposeIssue, ComposeIssueCode } from '$lib/api/signaling';

const ISSUE_TEXT: Record<ComposeIssueCode, string> = {
  wiringMissing: 'Choose a Signal LCC and a red and a green pin for this signal.',
  bindingInvalid: 'The chosen pins cannot drive this signal. Choose different pins.',
  hardwareUnavailable: 'The Signal LCC settings could not be read. Check that the node is online.',
  styleUnsupported: 'This Signal LCC cannot drive a red and green searchlight.',
  outputDuplicate: 'The red and green lamps need different pins.',
  outputInUse: 'A chosen pin is already in use by another signal. Choose a free pin.',
  outputCapacity: 'This Signal LCC does not have enough free pins.',
  noFreeMast: 'This Signal LCC has no free mast. Choose another Signal LCC.',
  noFreeLogic: 'This Signal LCC has no free logic left for these signals. Choose another Signal LCC.',
  claimedSlotUnavailable:
    'The mast and logic this signal already uses are no longer available on this Signal LCC. Choose the original Signal LCC.',
  occupancyEventsMissing: 'Occupancy events are missing for a protected block. Set up its detector first.',
  aspectEventsMissing: 'The aspect events for this signal are missing on the Signal LCC.',
  unsupportedBlockCount: 'This release cannot build signals for this number of blocks.',
  unsupportedMovements: 'This release builds signals for one direction only.',
  unsupportedEnds: 'This release does not build signals for these track ends.',
  unsupportedAspects: 'This release builds three-aspect signals only.',
  unsupportedApproachLighting: 'This release builds always-lit signals only.',
  releaseInvalid: 'A released mast or lamp can no longer be released.',
};

export function issueText(issue: ComposeIssue): string {
  return ISSUE_TEXT[issue.code] ?? issue.detail;
}
