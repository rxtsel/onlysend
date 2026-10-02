import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import { disconnectResend } from "$lib/shared/api/auth";
import { emailCache } from "$lib/features/sending/email-cache.svelte";
import { clearInboundStatus, getInboundStatus } from "$lib/shared/inbound-status.svelte";
import { accountSwitch, switchMailAccount, logoutMailAccount } from "./account-switch.svelte";

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$lib/shared/api/auth", () => ({ disconnectResend: vi.fn() }));

beforeEach(() => {
  vi.mocked(goto).mockReset().mockResolvedValue(undefined);
  vi.mocked(disconnectResend).mockReset().mockResolvedValue(undefined);
  accountSwitch.busy = false;
  accountSwitch.generation = 0;
  for (const id of ["a", "b"]) {
    emailCache.clear(id);
    clearInboundStatus(id);
    getInboundStatus(id).ready = true;
    emailCache.setList(id, []);
  }
});

test("logout names the account and preserves other account caches", async () => {
  await logoutMailAccount("a");
  expect(disconnectResend).toHaveBeenCalledWith("a");
  expect(goto).toHaveBeenCalledWith("/");
  expect(accountSwitch.busy).toBe(false);
  expect(emailCache.getList("a")).toBeNull();
  expect(getInboundStatus("a").ready).toBe(false);
  expect(emailCache.getList("b")).toEqual([]);
  expect(getInboundStatus("b").ready).toBe(true);
});

test("failed logout restores the view without navigating or deleting cache", async () => {
  vi.mocked(disconnectResend).mockRejectedValue(new Error("store failure"));
  await expect(logoutMailAccount("a")).rejects.toThrow("store failure");
  expect(goto).not.toHaveBeenCalled();
  expect(accountSwitch.busy).toBe(false);
  expect(emailCache.getList("a")).toEqual([]);
});

test("switch navigates to explicit account mailbox without the previous email ID", async () => {
  await switchMailAccount("b", "/mail/a/inbox/email-from-a");
  expect(goto).toHaveBeenCalledWith("/mail/b/inbox");
  expect(accountSwitch.busy).toBe(false);
  expect(accountSwitch.generation).toBe(1);
  expect(emailCache.getList("a")).toEqual([]);
});

test("failed navigation releases the loading boundary", async () => {
  vi.mocked(goto).mockRejectedValue(new Error("navigation failed"));
  await expect(switchMailAccount("missing", "/mail/a/sent/old-id")).rejects.toThrow("navigation failed");
  expect(accountSwitch.busy).toBe(false);
});

test("concurrent switches cannot race navigation", async () => {
  let finish!: () => void;
  vi.mocked(goto).mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
  const first = switchMailAccount("b", "/mail/a/sent");
  await vi.waitFor(() => expect(goto).toHaveBeenCalledTimes(1));
  await switchMailAccount("c", "/mail/a/inbox");
  expect(goto).toHaveBeenCalledTimes(1);
  finish();
  await first;
  expect(accountSwitch.busy).toBe(false);
});
