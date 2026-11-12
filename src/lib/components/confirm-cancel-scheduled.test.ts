// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import { goto } from "$app/navigation";
import { toast } from "svelte-sonner";
import ConfirmCancelScheduled from "./confirm-cancel-scheduled.svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("$app/navigation", () => ({ goto: vi.fn().mockResolvedValue(undefined) }));
vi.mock("$lib/features/auth/account-context", () => ({ useAccountId: () => "account-a" }));
vi.mock("svelte-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
let instance: ReturnType<typeof mount>;
const emailId = "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1";
beforeEach(() => {
  vi.clearAllMocks();
  const target = document.createElement("div");
  document.body.append(target);
  instance = mount(ConfirmCancelScheduled, { target, props: { open: true, emailId, subject: "Scheduled message" } });
  flushSync();
});
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });
function confirm() {
  flushSync(() => document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')!.click());
}

test("does not cancel without confirmation", () => {
  flushSync(() => document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')!.click());
  expect(invoke).not.toHaveBeenCalled();
  expect(goto).not.toHaveBeenCalled();
});

test("waits for remote cancellation and draft restoration before opening the draft", async () => {
  let finish!: (value: string) => void;
  vi.mocked(invoke).mockReturnValueOnce(new Promise<string>((resolve) => { finish = resolve; }));
  confirm();
  expect(invoke).toHaveBeenCalledExactlyOnceWith("cancel_scheduled_email", { accountId: "account-a", emailId });
  expect(document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')?.disabled).toBe(true);
  expect(goto).not.toHaveBeenCalled();
  finish(emailId);
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(`/mail/account-a/composer?draft=${emailId}`));
  expect(toast.success).toHaveBeenCalled();
});

test("an already-sent or failed cancellation leaves the message untouched and allows retry", async () => {
  vi.mocked(invoke).mockRejectedValueOnce("This email is no longer scheduled");
  confirm();
  await vi.waitFor(() => expect(toast.error).toHaveBeenCalledWith("This email is no longer scheduled"));
  expect(goto).not.toHaveBeenCalled();
  expect(document.querySelector('[role="alertdialog"]')).not.toBeNull();
  vi.mocked(invoke).mockResolvedValueOnce(emailId);
  confirm();
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(`/mail/account-a/composer?draft=${emailId}`));
});
