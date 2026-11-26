/** Return a reordered copy when an item can move to its neighbour. */
export function moveItem<T>(items: T[], index: number, delta: -1 | 1): T[] {
  const target = index + delta;
  if (!Number.isInteger(index) || index < 0 || index >= items.length || target < 0 || target >= items.length) return items;
  const result = [...items];
  [result[index], result[target]] = [result[target], result[index]];
  return result;
}

/** Return a reordered copy when an item moves to a different valid index. */
export function moveItemTo<T>(items: T[], from: number, to: number): T[] {
  if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || from >= items.length || to < 0 || to >= items.length || from === to) return items;
  const result = [...items];
  const [item] = result.splice(from, 1);
  result.splice(to, 0, item);
  return result;
}

/** Return the insertion slot at or below the given row midpoints. */
export function dropSlot(midpoints: number[], y: number): number {
  return midpoints.filter((midpoint) => y >= midpoint).length;
}
