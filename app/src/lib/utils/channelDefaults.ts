/**
 * Generate a default display name for an auto-created channel.
 *
 * Format: "{nodeName} — {slotLabel} — Input {ordinal}"
 */
export function generateDefaultChannelName(
  nodeName: string,
  slotLabel: string,
  inputOrdinal: number,
): string {
  return `${nodeName} — ${slotLabel} — Input ${inputOrdinal}`;
}
