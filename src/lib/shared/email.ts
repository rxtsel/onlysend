/**
 * Splits a raw string at the first "@": users often paste a full email
 * where only the local part is expected. Returns both halves so callers
 * can keep the local part and inspect the typed domain.
 */
export function stripAt(raw: string): { local: string; typedDomain: string } {
  const at = raw.indexOf("@");
  if (at === -1) return { local: raw, typedDomain: "" };
  return {
    local: raw.slice(0, at),
    typedDomain: raw.slice(at + 1).trim().toLowerCase(),
  };
}
