const time = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const shortDate = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const longDate = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" });
const fullDate = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

function parseDate(value: string): Date {
  // Resend also returns PostgreSQL timestamps with microseconds/hour-only offsets.
  const normalized = value.trim()
    .replace(" ", "T")
    .replace(/\s+([+-]\d{2}(?::?\d{2})?)$/, "$1")
    .replace(/(\.\d{3})\d+/, "$1")
    .replace(/([+-]\d{2})$/, "$1:00");
  return new Date(normalized);
}

export function formatEmailDate(value: string): string {
  const date = parseDate(value);
  if (Number.isNaN(date.getTime())) return value;
  const now = new Date();
  const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  const day = Date.UTC(date.getFullYear(), date.getMonth(), date.getDate());
  const days = (today - day) / 86_400_000;
  if (days === 0) return time.format(date);
  if (days === 1) return "Yesterday";
  return (date.getFullYear() === now.getFullYear() ? shortDate : longDate).format(date);
}

export function formatFullEmailDate(value: string): string {
  const date = parseDate(value);
  return Number.isNaN(date.getTime()) ? value : fullDate.format(date);
}
