import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "./external";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset(); });

test("external links use the native browser command without changing the URL", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  const url = "https://example.com/?a=1&b=two#section";
  await openUrl(url);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("open_external_url", { url });
});

test("launcher failures reach the caller instead of appearing successful", async () => {
  vi.mocked(invoke).mockRejectedValue("No default browser");
  await expect(openUrl("https://example.com")).rejects.toBe("No default browser");
});
