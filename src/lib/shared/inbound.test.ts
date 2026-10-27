import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { getInboundEmail } from "./inbound";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue(undefined); });

test("opening a message delegates the detail read and read marker to one native command", async () => {
  await getInboundEmail("a", "message");
  expect(invoke).toHaveBeenCalledExactlyOnceWith("open_inbound_email", { accountId: "a", emailId: "message" });
});

test("read failures propagate instead of claiming success", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("offline"));
  await expect(getInboundEmail("a", "message")).rejects.toThrow("offline");
});
