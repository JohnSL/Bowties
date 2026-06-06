/**
 * RFC 4122 v4 UUID generation using `crypto.getRandomValues`.
 *
 * `crypto.randomUUID()` would be simpler but requires Safari 15.4+ /
 * WebKit 615+ (macOS 12+, Ubuntu 22.04+). `getRandomValues` is available
 * everywhere Tauri runs (Safari 11+, WebKit 606+), so we hand-roll v4.
 */
export function generateUuidV4(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
  bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 1 (RFC 4122)
  return [...bytes]
    .map((b, i) => ([4, 6, 8, 10].includes(i) ? '-' : '') + b.toString(16).padStart(2, '0'))
    .join('');
}
