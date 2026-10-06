import { describe, expect, test } from "vitest";
import { formatEmailDate } from "./dates.js";

describe("formatEmailDate", () => {
  test("same-day timestamps render as time", () => {
    const now = new Date();
    // Locale appends an AM/PM marker; the time part is what matters.
    expect(formatEmailDate(now.toISOString())).toMatch(/^\d{1,2}:\d{2}/);
  });

  test("invalid strings fall back to the raw input", () => {
    expect(formatEmailDate("not-a-date")).toBe("not-a-date");
  });

  test("microsecond timestamps parse correctly", () => {
    // Resend format: microseconds suffix
    const result = formatEmailDate("2023-04-03T22:13:42.674981+00:00");
    expect(result).toBe("Apr 3, 2023");
  });
});
