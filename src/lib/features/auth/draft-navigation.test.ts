import { expect, test, vi } from "vitest";
import { registerDraftGuard, approveAccountDeparture, allowAccountNavigation } from "./draft-navigation";

test("draft guards are isolated by account and apply to direct/history navigation", () => {
  const confirmA = vi.fn(() => false);
  const cleanup = registerDraftGuard("a", confirmA);
  try {
    expect(allowAccountNavigation("b")).toBe(true);
    expect(allowAccountNavigation("a")).toBe(false);
    expect(allowAccountNavigation("a")).toBe(false);
    expect(confirmA).toHaveBeenCalledTimes(2);
  } finally { cleanup(); }
});

test("cleanup from an older composer cannot remove a newer guard", () => {
  const old = registerDraftGuard("a", () => true);
  const current = registerDraftGuard("a", () => false);
  old();
  expect(approveAccountDeparture("a")).toBe(false);
  current();
  expect(allowAccountNavigation("a")).toBe(true);
});
