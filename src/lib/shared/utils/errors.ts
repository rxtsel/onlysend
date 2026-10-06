/** Extracts a user-presentable message from a command rejection.
 *  Rust errors carry "[ERROR] <real message>" — strip the prefix so the
 *  actual cause (e.g. auth expired) reaches the toast. */
export function errorMessage(err: unknown, fallback: string): string {
  const cleaned = String(err ?? "")
    .replace(/^\[ERROR\]\s*/, "")
    .trim();
  return cleaned || fallback;
}
