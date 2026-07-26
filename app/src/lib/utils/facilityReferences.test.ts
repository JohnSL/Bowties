import { describe, it, expect } from 'vitest';
import { findUpstreamReferrers } from './facilityReferences';
import type { FacilityRecord } from '$lib/api/facilities';

function makeFacility(id: string, template: string, bindings: Record<string, string[]> = {}): FacilityRecord {
  return {
    facilityId: id,
    templateId: template,
    name: `Facility ${id}`,
    slotBindings: bindings,
    logicAllocation: null,
  };
}

describe('findUpstreamReferrers', () => {
  it('returns empty array when no facilities exist', () => {
    const facilities: FacilityRecord[] = [];
    expect(findUpstreamReferrers('f-target', facilities)).toEqual([]);
  });

  it('returns empty array when target facility does not exist', () => {
    const facilities = [
      makeFacility('f1', 'abs-3-aspect-signal', {
        output: ['ch-1'],
        'downstream-signal': [],
      }),
    ];
    expect(findUpstreamReferrers('f-nonexistent', facilities)).toEqual([]);
  });

  it('returns empty array when target facility has no output channels', () => {
    const facilities = [
      makeFacility('f-target', 'abs-3-aspect-signal', { output: [] }),
      makeFacility('f1', 'abs-3-aspect-signal', {
        output: ['ch-2'],
        'downstream-signal': ['ch-1'],
      }),
    ];
    expect(findUpstreamReferrers('f-target', facilities)).toEqual([]);
  });

  it('returns empty array when no upstream facilities reference target', () => {
    const facilities = [
      makeFacility('f-target', 'abs-3-aspect-signal', { output: ['ch-1'], 'downstream-signal': [] }),
      makeFacility('f1', 'abs-3-aspect-signal', {
        output: ['ch-2'],
        'downstream-signal': ['ch-3'],
      }),
    ];
    expect(findUpstreamReferrers('f-target', facilities)).toEqual([]);
  });

  it('returns single upstream facility when it references target via downstream-signal', () => {
    const target = makeFacility('f-target', 'abs-3-aspect-signal', { output: ['ch-1'], 'downstream-signal': [] });
    const upstream = makeFacility('f-upstream', 'abs-3-aspect-signal', {
      output: ['ch-2'],
      'downstream-signal': ['ch-1'],
    });
    const facilities = [target, upstream];
    const result = findUpstreamReferrers('f-target', facilities);
    expect(result).toHaveLength(1);
    expect(result[0].facilityId).toBe('f-upstream');
  });

  it('returns multiple upstream facilities when they reference target', () => {
    const target = makeFacility('f-target', 'abs-3-aspect-signal', { output: ['ch-1'], 'downstream-signal': [] });
    const upstream1 = makeFacility('f-upstream-1', 'abs-3-aspect-signal', {
      output: ['ch-2'],
      'downstream-signal': ['ch-1'],
    });
    const upstream2 = makeFacility('f-upstream-2', 'abs-3-aspect-signal', {
      output: ['ch-3'],
      'downstream-signal': ['ch-1'],
    });
    const facilities = [target, upstream1, upstream2];
    const result = findUpstreamReferrers('f-target', facilities);
    expect(result).toHaveLength(2);
    expect(result.map((f) => f.facilityId).sort()).toEqual(['f-upstream-1', 'f-upstream-2']);
  });

  it('excludes self-references (facility referencing itself)', () => {
    const self = makeFacility('f-self', 'abs-3-aspect-signal', {
      output: ['ch-1'],
      'downstream-signal': ['ch-1'],
    });
    const facilities = [self];
    const result = findUpstreamReferrers('f-self', facilities);
    expect(result).toEqual([]);
  });

  it('handles multiple output channels on target facility', () => {
    const target = makeFacility('f-target', 'abs-3-aspect-signal', {
      output: ['ch-1', 'ch-2'],
      'downstream-signal': [],
    });
    const upstream1 = makeFacility('f-upstream-1', 'abs-3-aspect-signal', {
      output: ['ch-3'],
      'downstream-signal': ['ch-1'],
    });
    const upstream2 = makeFacility('f-upstream-2', 'abs-3-aspect-signal', {
      output: ['ch-4'],
      'downstream-signal': ['ch-2'],
    });
    const facilities = [target, upstream1, upstream2];
    const result = findUpstreamReferrers('f-target', facilities);
    expect(result).toHaveLength(2);
    expect(result.map((f) => f.facilityId).sort()).toEqual(['f-upstream-1', 'f-upstream-2']);
  });

  it('returns each upstream facility only once even if it binds multiple downstream channels', () => {
    const target = makeFacility('f-target', 'abs-3-aspect-signal', {
      output: ['ch-1', 'ch-2'],
      'downstream-signal': [],
    });
    // Upstream binds only ch-1 (not both).
    const upstream = makeFacility('f-upstream', 'abs-3-aspect-signal', {
      output: ['ch-3'],
      'downstream-signal': ['ch-1'],
    });
    const facilities = [target, upstream];
    const result = findUpstreamReferrers('f-target', facilities);
    expect(result).toHaveLength(1);
    expect(result[0].facilityId).toBe('f-upstream');
  });

  it('handles empty slot bindings gracefully', () => {
    const target = makeFacility('f-target', 'abs-3-aspect-signal', {
      output: ['ch-1'],
      'downstream-signal': [],
    });
    const upstream = makeFacility('f-upstream', 'abs-3-aspect-signal', {
      output: ['ch-2'],
    }); // No downstream-signal slot binding.
    const facilities = [target, upstream];
    const result = findUpstreamReferrers('f-target', facilities);
    expect(result).toEqual([]);
  });
});
