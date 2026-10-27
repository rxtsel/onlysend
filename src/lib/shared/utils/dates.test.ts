import { afterEach, expect, test, vi } from "vitest";
import { formatEmailDate, formatFullEmailDate } from "./dates";

afterEach(() => { vi.useRealTimers(); });

test("same-day timestamps render as local time", () => {
  vi.useFakeTimers();
  const now = new Date(2026, 3, 4, 14, 32);
  vi.setSystemTime(now);
  expect(formatEmailDate(now.toISOString())).toBe(new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" }).format(now));
});

test("microsecond timestamps retain their date and year", () => {
  expect(formatEmailDate("2023-04-03T22:13:42.674981+00:00")).toBe(
    new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" }).format(new Date("2023-04-03T22:13:42.674Z")),
  );
});

test("yesterday follows the local calendar, not a rolling 24-hour window", () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 3, 4, 0, 5));
  expect(formatEmailDate(new Date(2026, 3, 3, 23, 55).toISOString())).toBe("Yesterday");
});

test("documented Resend PostgreSQL timestamps format like ISO without reparsing per locale", () => {
  const iso = "2026-04-03T22:13:42.674Z";
  for (const value of ["2026-04-03 22:13:42.674981+00", "2026-04-03 22:13:42.674981 +00:00"]) {
    expect(formatEmailDate(value)).toBe(formatEmailDate(iso));
    expect(formatFullEmailDate(value)).toBe(formatFullEmailDate(iso));
  }
});

test("future and invalid dates never render negative days ago or throw", () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 3, 4));
  expect(formatEmailDate("2026-04-10T12:00:00Z")).not.toContain("ago");
  expect(formatEmailDate("invalid")).toBe("invalid");
  expect(formatFullEmailDate("invalid")).toBe("invalid");
});
