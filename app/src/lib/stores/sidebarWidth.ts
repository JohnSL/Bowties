/**
 * Sidebar Width Store
 *
 * Manages the resizable sidebar panel width with localStorage persistence.
 * Width is app-wide (not per-layout) and survives app restarts.
 */
import { writable } from 'svelte/store';

const STORAGE_KEY = 'bowties:sidebarWidth';
const DEFAULT_WIDTH = 240;
const MIN_WIDTH = 160;
const MAX_WIDTH = 600;

function clamp(value: number): number {
  return Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, value));
}

function readFromStorage(): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return DEFAULT_WIDTH;
    const parsed = Number(raw);
    if (Number.isNaN(parsed)) return DEFAULT_WIDTH;
    return clamp(parsed);
  } catch {
    return DEFAULT_WIDTH;
  }
}

function createSidebarWidthStore() {
  const { subscribe, set } = writable<number>(readFromStorage());

  return {
    subscribe,

    setWidth(width: number): void {
      const clamped = clamp(width);
      set(clamped);
      try {
        localStorage.setItem(STORAGE_KEY, String(clamped));
      } catch {
        // Storage full or unavailable — silently degrade
      }
    },

    reset(): void {
      set(DEFAULT_WIDTH);
      try {
        localStorage.removeItem(STORAGE_KEY);
      } catch {
        // Silently degrade
      }
    },
  };
}

export const sidebarWidthStore = createSidebarWidthStore();

/** Exported constants for use in resize handle constraints */
export const SIDEBAR_MIN_WIDTH = MIN_WIDTH;
export const SIDEBAR_MAX_WIDTH = MAX_WIDTH;
