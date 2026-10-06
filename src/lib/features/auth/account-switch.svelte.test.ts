import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import { disconnectResend } from "$lib/shared/api/auth";
import { emailCache } from "$lib/features/sending/email-cache.svelte";
import { clearInboundStatus, getInboundStatus } from "$lib/shared/inbound-status.svelte";
import { accountSwitch, switchMailAccount, logoutMailAccount } from "./account-switch.svelte";
import { registerDraftGuard, allowAccountNavigation } from "./draft-navigation";

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$lib/shared/api/auth", () => ({ disconnectResend: vi.fn() }));

beforeEach(() => {
  vi.mocked(goto).mockReset().mockResolvedValue(undefined);
  vi.mocked(disconnectResend).mockReset().mockResolvedValue(undefined);
  accountSwitch.busy = false;
  for (const id of ["a", "b"]) {
    emailCache.clear(id);
    clearInboundStatus(id);
    getInboundStatus(id).ready = true;
    emailCache.setList(id, { items: [], hasMore: false, nextCursor: null });
  }
});

test("logout names the account and preserves other account caches", async () => {
  await logoutMailAccount("a");
  expect(disconnectResend).toHaveBeenCalledWith("a");
  expect(goto).toHaveBeenCalledWith("/");
  expect(accountSwitch.busy).toBe(false);
  expect(emailCache.getList("a")).toBeNull();
  expect(getInboundStatus("a").ready).toBe(false);
  expect(emailCache.getList("b")).toEqual({ items: [], hasMore: false, nextCursor: null });
  expect(getInboundStatus("b").ready).toBe(true);
});

test("failed logout restores the view without navigating or deleting cache", async () => {
  vi.mocked(disconnectResend).mockRejectedValue(new Error("store failure"));
  await expect(logoutMailAccount("a")).rejects.toThrow("store failure");
  expect(goto).not.toHaveBeenCalled();
  expect(accountSwitch.busy).toBe(false);
  expect(emailCache.getList("a")).toEqual({ items: [], hasMore: false, nextCursor: null });
});

test("switch navigates to explicit account mailbox without the previous email ID", async () => {
  await switchMailAccount("b", "/mail/a/inbox/email-from-a");
  expect(goto).toHaveBeenCalledWith("/mail/b/inbox");
  expect(accountSwitch.busy).toBe(false);
  expect(emailCache.getList("a")).toEqual({ items: [], hasMore: false, nextCursor: null });
});

test("failed navigation releases the loading boundary", async () => {
  vi.mocked(goto).mockRejectedValue(new Error("navigation failed"));
  await expect(switchMailAccount("missing", "/mail/a/sent/old-id")).rejects.toThrow("navigation failed");
  expect(accountSwitch.busy).toBe(false);
});

test("cancelled draft departure does not navigate or remove credentials", async () => {
  const confirm = vi.fn(() => false);
  const cleanup = registerDraftGuard("a", confirm);
  try {
    await switchMailAccount("b", "/mail/a/composer");
    expect(await logoutMailAccount("a")).toBe(false);
    expect(goto).not.toHaveBeenCalled();
    expect(disconnectResend).not.toHaveBeenCalled();
    expect(accountSwitch.busy).toBe(false);
    expect(emailCache.getList("a")).toEqual({ items: [], hasMore: false, nextCursor: null });
  } finally { cleanup(); }
});

test("approved draft departure prompts once and keeps its guard mounted during navigation", async () => {
  const confirm = vi.fn(() => true);
  const cleanup = registerDraftGuard("a", confirm);
  vi.mocked(goto).mockImplementationOnce(async () => {
    expect(accountSwitch.busy).toBe(true);
    expect(allowAccountNavigation("a")).toBe(true);
    expect(confirm).toHaveBeenCalledTimes(1);
  });
  try {
    await switchMailAccount("b", "/mail/a/composer");
    expect(confirm).toHaveBeenCalledTimes(1);
  } finally { cleanup(); }
});

test("failed operation resets draft approval for the next navigation", async () => {
  const confirm = vi.fn().mockReturnValueOnce(true).mockReturnValueOnce(false);
  const cleanup = registerDraftGuard("a", confirm);
  vi.mocked(disconnectResend).mockRejectedValue(new Error("store failure"));
  try {
    await expect(logoutMailAccount("a")).rejects.toThrow("store failure");
    expect(allowAccountNavigation("a")).toBe(false);
    expect(confirm).toHaveBeenCalledTimes(2);
  } finally { cleanup(); }
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
