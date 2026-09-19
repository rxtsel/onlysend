import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import { disconnectResend, setActiveAccount } from "@/lib/shared/api/auth";
import { emailCache } from "@/lib/features/sending/email-cache.svelte";
import { inboundStatus } from "@/lib/shared/inbound-status.svelte";
import { accountSwitch, switchMailAccount, logoutMailAccount } from "./account-switch.svelte";

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("@/lib/shared/api/auth", () => ({ setActiveAccount: vi.fn(), disconnectResend: vi.fn() }));

beforeEach(() => {
  vi.mocked(goto).mockReset().mockResolvedValue(undefined);
  vi.mocked(setActiveAccount).mockReset().mockResolvedValue(undefined);
  vi.mocked(disconnectResend).mockReset().mockResolvedValue(undefined);
  accountSwitch.busy = false;
  accountSwitch.generation = 0;
  inboundStatus.ready = true;
  emailCache.setList([]);
});

test("logout clears account caches and returns to auth routing", async () => {
  await logoutMailAccount();
  expect(disconnectResend).toHaveBeenCalledTimes(1);
  expect(goto).toHaveBeenCalledWith("/");
  expect(accountSwitch.busy).toBe(false);
  expect(emailCache.getList()).toBeNull();
  expect(inboundStatus.ready).toBe(false);
});

test("failed logout restores the view without navigating", async () => {
  vi.mocked(disconnectResend).mockRejectedValue(new Error("store failure"));
  await expect(logoutMailAccount()).rejects.toThrow("store failure");
  expect(goto).not.toHaveBeenCalled();
  expect(accountSwitch.busy).toBe(false);
});

test("switch returns to mailbox root and resets all shared account state", async () => {
  await switchMailAccount("b", "/mail/inbox/email-from-a");
  expect(goto).toHaveBeenCalledWith("/mail/inbox");
  expect(setActiveAccount).toHaveBeenCalledWith("b");
  expect(accountSwitch.busy).toBe(false);
  expect(accountSwitch.generation).toBe(1);
  expect(inboundStatus.ready).toBe(false);
  expect(emailCache.getList()).toBeNull();
});

test("failed switch releases loading boundary and remounts original account", async () => {
  vi.mocked(setActiveAccount).mockRejectedValue(new Error("unknown account"));
  await expect(switchMailAccount("missing", "/mail/sent/old-id")).rejects.toThrow("unknown account");
  expect(goto).toHaveBeenCalledWith("/mail/sent");
  expect(accountSwitch.busy).toBe(false);
  expect(accountSwitch.generation).toBe(1);
});

test("concurrent switches cannot race active account writes", async () => {
  let finish!: () => void;
  vi.mocked(setActiveAccount).mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
  const first = switchMailAccount("b", "/mail/sent");
  await vi.waitFor(() => expect(setActiveAccount).toHaveBeenCalledTimes(1));
  await switchMailAccount("c", "/mail/inbox");
  expect(setActiveAccount).toHaveBeenCalledTimes(1);
  finish();
  await first;
  expect(accountSwitch.busy).toBe(false);
});
