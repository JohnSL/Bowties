import { expect, it, vi } from 'vitest';
import * as facilities from './facilityOrchestrator';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

it('retires the Block Indicator public workflows and keeps Panel / ABS / shared removal', () => {
  for (const name of ['selectChannelForSlot', 'addChannelForSlot', 'removeFromSlot', 'composeBowtiesIfWired', 'tearDownFacilityBowties']) {
    expect(Object.hasOwn(facilities, name), `${name} must be retired`).toBe(false);
  }
  for (const name of ['createPanelFacility', 'updatePanelFacility', 'createBlockSignalsFacility', 'updateBlockSignalsFacility', 'removeFacility']) {
    expect(Object.hasOwn(facilities, name), `${name} must remain`).toBe(true);
  }
});
