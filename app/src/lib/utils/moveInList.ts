/** Return a reordered copy when an item can move to its neighbour. */
export function moveItem<T>(items: T[], index: number, delta: -1 | 1): T[] {
  const target = index + delta;
  if (!Number.isInteger(index) || index < 0 || index >= items.length || target < 0 || target >= items.length) return items;
  const result = [...items];
  [result[index], result[target]] = [result[target], result[index]];
  return result;
}
