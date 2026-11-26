import { dropSlot } from '$lib/utils/moveInList';

export type DropSide = 'before' | 'after';

/**
 * Drag-to-reorder state for one list, driven by pointer events on a grip
 * (the desktop webview intercepts native HTML5 drag and drop). Rows carry
 * `data-reorder-id`; the drop index comes from the row midpoints
 * (`dropSlot`), the same rule for every list.
 */
export class DragReorder {
  draggingId = $state<string | null>(null);
  marker = $state<{ id: string; side: DropSide } | null>(null);

  constructor(
    private readonly container: () => HTMLElement | null | undefined,
    private readonly onDrop: (id: string, toIndex: number) => void,
  ) {}

  /** The drop line to draw on a row, if any. */
  sideFor(id: string): DropSide | undefined {
    return this.marker?.id === id ? this.marker.side : undefined;
  }

  /** Begin a drag from the grip; the grip captures the pointer until release. */
  start(id: string, event: PointerEvent): void {
    if (event.button !== 0) return;
    event.preventDefault();
    const grip = event.currentTarget as HTMLElement | null;
    grip?.setPointerCapture?.(event.pointerId);
    this.draggingId = id;
    this.marker = null;
  }

  move(event: PointerEvent): void {
    if (!this.draggingId) return;
    const rows = this.rows();
    if (rows.length === 0) return;
    const slot = this.slotAt(rows, event.clientY);
    this.marker = slot >= rows.length
      ? { id: this.idOf(rows[rows.length - 1]), side: 'after' }
      : { id: this.idOf(rows[slot]), side: 'before' };
  }

  finish(event: PointerEvent): void {
    const id = this.draggingId;
    if (!id) return;
    const moved = this.marker !== null;
    const rows = this.rows();
    const from = rows.findIndex((row) => this.idOf(row) === id);
    const slot = this.slotAt(rows, event.clientY);
    this.cancel();
    if (!moved || from < 0) return;
    const to = slot > from ? slot - 1 : slot;
    if (to !== from) this.onDrop(id, to);
  }

  cancel(): void {
    this.draggingId = null;
    this.marker = null;
  }

  private rows(): HTMLElement[] {
    return Array.from(this.container()?.querySelectorAll<HTMLElement>('[data-reorder-id]') ?? []);
  }

  private idOf(row: HTMLElement): string {
    return row.dataset.reorderId ?? '';
  }

  private slotAt(rows: HTMLElement[], y: number): number {
    return dropSlot(rows.map((row) => {
      const box = row.getBoundingClientRect();
      return box.top + box.height / 2;
    }), y);
  }
}
