import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { refreshInbox } from "./inbound";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue(undefined); });

test("inbox refresh delegates detection and scheduling to Rust for the explicit account", async () => {
  await refreshInbox("a");
  expect(invoke).toHaveBeenCalledExactlyOnceWith("refresh_inbox", { accountId: "a", force: false });
});

test("manual refresh explicitly requests an archive retry", async () => {
  await refreshInbox("b", true);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("refresh_inbox", { accountId: "b", force: true });
});

test("refresh failures propagate instead of claiming success", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("offline"));
  await expect(refreshInbox("a", true)).rejects.toThrow("offline");
});
