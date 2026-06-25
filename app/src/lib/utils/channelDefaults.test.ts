import { describe, expect, it } from 'vitest';

import { generateDefaultChannelName } from './channelDefaults';

describe('generateDefaultChannelName', () => {
  it('produces the expected format with node name, slot label, and input ordinal', () => {
    expect(generateDefaultChannelName('West Yard', 'Connector A', 1)).toBe(
      'West Yard — Connector A — Input 1',
    );
  });

  it('handles multi-digit input ordinals', () => {
    expect(generateDefaultChannelName('East Staging', 'Connector B', 8)).toBe(
      'East Staging — Connector B — Input 8',
    );
  });

  it('uses the node name as-is (no truncation or normalization)', () => {
    const longName = 'My Very Long Node Name With Lots Of Words';
    expect(generateDefaultChannelName(longName, 'Connector A', 3)).toBe(
      `${longName} — Connector A — Input 3`,
    );
  });
});
