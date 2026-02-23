/**
 * T008: Vitest unit tests for resolveCardTitle()
 * TDD — written before implementation; must FAIL until cardTitle.ts exists.
 *
 * Covers:
 * - replicated + named: "Yard Button (Line 3)"
 * - replicated + unnamed: "Line 3 (unnamed)"
 * - non-replicated + named: "Yard Button (Port I/O)"
 * - non-replicated + unnamed: CDI group name only
 * - null-byte user name treated as unnamed (RQ-002)
 * - whitespace-only user name treated as unnamed (RQ-002)
 */

import { describe, it, expect } from 'vitest';
import { resolveCardTitle } from '$lib/utils/cardTitle';
import type { CardField } from '$lib/stores/configSidebar';
import type { ConfigValueWithMetadata } from '$lib/api/types';
import { getCacheKey } from '$lib/api/types';

const NODE_ID = '02.01.57.00.00.01';

/** Build a minimal ConfigValueMap with a user name string at the given path */
function makeConfigValues(
  nodeId: string,
  elementPath: string[],
  value: string,
): Map<string, ConfigValueWithMetadata> {
  const map = new Map<string, ConfigValueWithMetadata>();
  map.set(getCacheKey(nodeId, elementPath), {
    value: { type: 'String', value, size_bytes: 16 },
    memory_address: 100,
    address_space: 253,
    element_path: elementPath,
    timestamp: new Date().toISOString(),
  });
  return map;
}

/** Minimal CardField stub for a User Name string element */
function makeUserNameField(path: string[]): CardField {
  return {
    elementPath: path,
    name: 'User Name',
    description: null,
    dataType: 'string',
    memoryAddress: 100,
    sizeBytes: 16,
    defaultValue: null,
    addressSpace: 253,
  };
}

describe('resolveCardTitle()', () => {
  it('replicated + named: returns "Yard Button (Line 3)" (FR-007)', () => {
    const userNamePath = ['Port I/O', 'elem:2#3', 'elem:0'];
    const fields: CardField[] = [makeUserNameField(userNamePath)];
    const configValues = makeConfigValues(NODE_ID, userNamePath, 'Yard Button');

    const title = resolveCardTitle(
      { cdGroupName: 'Line', isReplicated: true, instanceIndex: 3, fields },
      NODE_ID,
      configValues,
    );
    expect(title).toBe('Yard Button (Line 3)');
  });

  it('replicated + unnamed: returns "Line 3 (unnamed)" (FR-007)', () => {
    const fields: CardField[] = [];
    const configValues = new Map<string, ConfigValueWithMetadata>();

    const title = resolveCardTitle(
      { cdGroupName: 'Line', isReplicated: true, instanceIndex: 3, fields },
      NODE_ID,
      configValues,
    );
    expect(title).toBe('Line 3 (unnamed)');
  });

  it('non-replicated + named: returns "Yard Button (Port I/O)" (FR-007)', () => {
    const userNamePath = ['Port I/O', 'elem:0'];
    const fields: CardField[] = [makeUserNameField(userNamePath)];
    const configValues = makeConfigValues(NODE_ID, userNamePath, 'Yard Button');

    const title = resolveCardTitle(
      { cdGroupName: 'Port I/O', isReplicated: false, instanceIndex: null, fields },
      NODE_ID,
      configValues,
    );
    expect(title).toBe('Yard Button (Port I/O)');
  });

  it('non-replicated + unnamed: returns CDI group name only (FR-007)', () => {
    const fields: CardField[] = [];
    const configValues = new Map<string, ConfigValueWithMetadata>();

    const title = resolveCardTitle(
      { cdGroupName: 'Port I/O', isReplicated: false, instanceIndex: null, fields },
      NODE_ID,
      configValues,
    );
    expect(title).toBe('Port I/O');
  });

  it('null-byte user name treated as unnamed (RQ-002)', () => {
    const userNamePath = ['Port I/O', 'elem:2#3', 'elem:0'];
    const fields: CardField[] = [makeUserNameField(userNamePath)];
    // Null-byte only string
    const configValues = makeConfigValues(NODE_ID, userNamePath, '\x00\x00\x00');

    const title = resolveCardTitle(
      { cdGroupName: 'Line', isReplicated: true, instanceIndex: 3, fields },
      NODE_ID,
      configValues,
    );
    expect(title).toBe('Line 3 (unnamed)');
  });

  it('whitespace-only user name treated as unnamed (RQ-002)', () => {
    const userNamePath = ['Port I/O', 'elem:2#3', 'elem:0'];
    const fields: CardField[] = [makeUserNameField(userNamePath)];
    const configValues = makeConfigValues(NODE_ID, userNamePath, '   ');

    const title = resolveCardTitle(
      { cdGroupName: 'Line', isReplicated: true, instanceIndex: 3, fields },
      NODE_ID,
      configValues,
    );
    expect(title).toBe('Line 3 (unnamed)');
  });

  it('searches for "name" (case-insensitive) as fallback field name (RQ-002)', () => {
    const namePath = ['Identification', 'elem:0'];
    const fields: CardField[] = [{
      elementPath: namePath,
      name: 'Name',
      description: null,
      dataType: 'string',
      memoryAddress: 0,
      sizeBytes: 32,
      defaultValue: null,
      addressSpace: 253,
    }];
    const configValues = makeConfigValues(NODE_ID, namePath, 'My Device');

    const title = resolveCardTitle(
      { cdGroupName: 'Identification', isReplicated: false, instanceIndex: null, fields },
      NODE_ID,
      configValues,
    );
    expect(title).toBe('My Device (Identification)');
  });
});
