import { describe, expect, test, vi, beforeEach } from "vitest";
import { mailUrl, mailboxFromPath } from "./mail-routes";
import { getConnectionStatus, setActiveAccount } from "$lib/shared/api/auth";
import { load } from "../../../routes/mail/[accountId]/+layout";

vi.mock("$lib/shared/api/auth", () => ({ getConnectionStatus: vi.fn(), setActiveAccount: vi.fn() }));
beforeEach(() => {
  vi.mocked(getConnectionStatus).mockReset().mockResolvedValue({ method: "oauth" });
  vi.mocked(setActiveAccount).mockReset().mockResolvedValue(undefined);
});

describe("account routes", () => {
  test("routes include explicit account and escaped message IDs", () => {
    expect(mailUrl("a", "inbox", "message/a")).toBe("/mail/a/inbox/message%2Fa");
    expect(mailUrl("b", "composer")).toBe("/mail/b/composer");
    expect(() => mailUrl("", "sent")).toThrow("Account ID");
    expect(mailboxFromPath("/mail/a/inbox/123")).toBe("inbox");
    expect(mailboxFromPath("/mail/b/sent")).toBe("sent");
  });

  test("direct navigation and history use the URL account, not the active pointer", async () => {
    for (const accountId of ["a", "b", "a"]) {
      await expect(load({ params: { accountId } } as never)).resolves.toEqual({ accountId });
      expect(getConnectionStatus).toHaveBeenLastCalledWith(accountId);
      expect(setActiveAccount).toHaveBeenLastCalledWith(accountId);
    }
  });

  test("unknown or removed account errors without selecting a fallback", async () => {
    vi.mocked(getConnectionStatus).mockRejectedValue(new Error("Unknown account"));
    await expect(load({ params: { accountId: "deleted" } } as never)).rejects.toMatchObject({ status: 404 });
    expect(setActiveAccount).not.toHaveBeenCalled();
  });
});
