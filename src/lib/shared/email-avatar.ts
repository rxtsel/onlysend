/** Ignore display names, casing and surrounding whitespace, not dots or aliases. */
export function emailAvatarSeed(value: string): string {
  const start = value.lastIndexOf("<");
  const end = value.indexOf(">", start);
  const address = start >= 0 && end > start ? value.slice(start + 1, end) : value;
  return address.trim().toLowerCase() || "unknown";
}
