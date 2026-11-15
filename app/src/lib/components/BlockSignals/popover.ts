/**
 * Placement for small popovers (help bubbles, row menus) attached to a
 * button inside a scrolling column: open below unless that would be clipped
 * and there is room above.
 */

/** The nearest ancestor that clips its content. */
export function clippingAncestor(node: HTMLElement): HTMLElement | null {
  for (let el = node.parentElement; el; el = el.parentElement) {
    const { overflowY } = getComputedStyle(el);
    if (overflowY === 'auto' || overflowY === 'scroll' || overflowY === 'hidden') return el;
  }
  return null;
}

/**
 * True when a popover should open above its anchor: it doesn't fit below,
 * and either it fits above or there is more room above than below.
 */
export function shouldOpenAbove(anchor: HTMLElement, popover: HTMLElement, gap = 6): boolean {
  const clip = clippingAncestor(anchor)?.getBoundingClientRect();
  if (!clip) return false;
  const box = anchor.getBoundingClientRect();
  const height = popover.getBoundingClientRect().height;
  const roomBelow = clip.bottom - box.bottom - gap;
  const roomAbove = box.top - clip.top - gap;
  return height > roomBelow && (height <= roomAbove || roomAbove > roomBelow);
}
