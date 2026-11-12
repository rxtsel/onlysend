import { afterEach, expect, test, vi } from "vitest";
import { emailComposerSchema } from "./email-composer.schema";

const email = { from: "me@example.com", to: ["you@example.com"], subject: "Hello", content: "Hi" };
afterEach(() => vi.useRealTimers());

test("scheduling is optional and accepts only future ISO times within 30 days", () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-07-01T12:00:00Z"));
  expect(emailComposerSchema.safeParse(email).success).toBe(true);
  for (const scheduledAt of ["2026-07-01T13:00:00Z", "2026-07-31T12:00:00Z", "2026-07-01T15:00:00+02:00"]) {
    expect(emailComposerSchema.safeParse({ ...email, scheduledAt }).success).toBe(true);
  }
  for (const scheduledAt of ["invalid", "", "2026-07-01T13:00", "2026-07-01T12:00:00Z", "2026-06-30T12:00:00Z", "2026-07-31T12:00:01Z"]) {
    expect(emailComposerSchema.safeParse({ ...email, scheduledAt }).success).toBe(false);
  }
});
