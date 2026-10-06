import { describe, expect, it, vi } from "vitest";
import { registerDraftGuard, confirmAppDeparture, allowAccountNavigation } from "./draft-navigation";

describe("updater draft protection", () => {
  it("allows an app restart without a mounted draft", () => { expect(confirmAppDeparture()).toBe(true); });
  it("cancels installation when the current draft declines departure", () => {
    const confirm = vi.fn(() => false);
    const cleanup = registerDraftGuard("update-cancel", confirm);
    try { expect(confirmAppDeparture()).toBe(false); expect(confirm).toHaveBeenCalledOnce(); }
    finally { cleanup(); }
  });
  it("never leaves account-navigation approval behind after an update attempt", () => {
    const confirm = vi.fn().mockReturnValueOnce(true).mockReturnValueOnce(false);
    const cleanup = registerDraftGuard("update-failure", confirm);
    try {
      expect(confirmAppDeparture()).toBe(true);
      expect(allowAccountNavigation("update-failure")).toBe(false);
      expect(confirm).toHaveBeenCalledTimes(2);
    } finally { cleanup(); }
  });
});
