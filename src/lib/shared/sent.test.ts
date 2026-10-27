import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { getSentEmail } from "./sent";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset(); });

test("sent details use the native account-scoped cache, never a shared JS cache", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ id: "same", subject: "A" }).mockResolvedValueOnce({ id: "same", subject: "B" });
  expect((await getSentEmail("a", "same")).subject).toBe("A");
  expect((await getSentEmail("b", "same")).subject).toBe("B");
  expect(invoke).toHaveBeenNthCalledWith(1, "get_sent_email", { accountId: "a", emailId: "same" });
  expect(invoke).toHaveBeenNthCalledWith(2, "get_sent_email", { accountId: "b", emailId: "same" });
});
