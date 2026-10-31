// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Harness from "./__tests__/mail-list-harness.svelte";
import { loadMailList, type MailListItem, type Mailbox } from "$lib/shared/local-mail";
import { deleteLocalMails } from "$lib/shared/services/delete-mail";
import { goto } from "$app/navigation";

const { page, handlers } = vi.hoisted(() => ({
  page: { params: { id: undefined as string | undefined } },
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
}));
vi.mock("$app/state", () => ({ page }));
vi.mock("$app/navigation", () => ({ goto: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async (name, handler) => {
  handlers.set(name, handler);
  return () => handlers.delete(name);
}) }));
vi.mock("$lib/components/ui/sidebar", async () => ({ Trigger: (await import("./__tests__/layout-frame-probe.svelte")).default }));
vi.mock("$lib/shared/local-mail", () => ({ loadMailList: vi.fn(), getMailArchiveStatus: vi.fn().mockResolvedValue({ downloadedMessages: 9, downloadedBodies: 9 }), refreshMailbox: vi.fn() }));
vi.mock("$lib/shared/services/delete-mail", () => ({ deleteLocalMail: vi.fn().mockResolvedValue(true), deleteLocalMails: vi.fn().mockResolvedValue(true) }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

let instance: ReturnType<typeof mount>;
let target: HTMLElement;
let controls = $state<{ mailbox: Mailbox; domain: string | null; active: boolean }>({ mailbox: "inbox", domain: null, active: true });
let items: MailListItem[];
const onSelectMail = vi.fn();
const rows = () => Array.from(target.querySelectorAll<HTMLButtonElement>("[data-mail-id]"));
const selected = () => rows().filter((row) => row.getAttribute("aria-pressed") === "true").map((row) => row.dataset.mailId);
const click = (index: number, modifiers: MouseEventInit = {}) => {
  rows()[index].dispatchEvent(new MouseEvent("click", { bubbles: true, ...modifiers }));
  flushSync();
};
const key = (target: HTMLElement, key: string, shiftKey = true) => {
  const event = new KeyboardEvent("keydown", { key, shiftKey, bubbles: true, cancelable: true });
  target.dispatchEvent(event);
  flushSync();
  return event;
};
const confirm = () => document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')!.click();

beforeEach(async () => {
  vi.clearAllMocks();
  handlers.clear();
  page.params.id = undefined;
  controls = { mailbox: "inbox", domain: null, active: true };
  items = Array.from({ length: 9 }, (_, i) => ({
    id: `m${i}`, from: "sender@example.com", to: ["recipient@example.com"], subject: `Subject ${i}`,
    createdAt: "2026-01-01T00:00:00Z", preview: "Body", isRead: false,
  }));
  vi.mocked(loadMailList).mockImplementation(async () => ({ items: [...items], domains: [], hasMore: false, nextCursor: null, cache: { downloadedAt: 123, error: null } }));
  vi.mocked(deleteLocalMails).mockResolvedValue(true);
  target = document.createElement("div");
  document.body.append(target);
  instance = mount(Harness, { target, props: { controls, onSelectMail } });
  await vi.waitFor(() => expect(rows()).toHaveLength(9));
});
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });

test("shift and ctrl/cmd clicks select rows without opening or marking mail read", () => {
  click(1, { shiftKey: true });
  click(8, { shiftKey: true });
  expect(selected()).toEqual(items.slice(1).map((mail) => mail.id));
  click(4, { shiftKey: true });
  expect(selected()).toEqual(["m1", "m2", "m3", "m4"]);
  key(rows()[4], "Escape", false);
  click(0, { ctrlKey: true });
  click(6, { metaKey: true });
  expect(selected()).toEqual(["m0", "m6"]);
  click(0, { ctrlKey: true });
  expect(selected()).toEqual(["m6"]);
  expect(goto).not.toHaveBeenCalled();
  expect(onSelectMail).not.toHaveBeenCalled();
  expect(items.every((item) => !item.isRead)).toBe(true);
  expect(target.textContent).toContain("1 Selected");
  expect(target.querySelector('[aria-label="Refresh mail"]')).toBeNull();
});

test("a normal click opens the preview and anchors shift arrows with focus and contraction", () => {
  click(2);
  expect(goto).toHaveBeenCalledExactlyOnceWith("/mail/account-a/inbox/m2");
  expect(onSelectMail).toHaveBeenCalledTimes(1);
  expect(key(rows()[2], "ArrowDown").defaultPrevented).toBe(true);
  expect(selected()).toEqual(["m2", "m3"]);
  expect(document.activeElement).toBe(rows()[3]);
  key(rows()[3], "ArrowDown");
  expect(selected()).toEqual(["m2", "m3", "m4"]);
  key(rows()[4], "ArrowUp");
  expect(selected()).toEqual(["m2", "m3"]);
  key(rows()[3], "Escape", false);
  expect(selected()).toEqual([]);
  expect(target.querySelector('[aria-label="Refresh mail"]')).not.toBeNull();
  expect(goto).toHaveBeenCalledTimes(1);
});

test("keyboard selection does not steal keys from search, menus or the preview", () => {
  const input = target.querySelector<HTMLInputElement>("input")!;
  expect(key(input, "ArrowDown").defaultPrevented).toBe(false);
  const preview = document.createElement("button");
  document.body.append(preview);
  expect(key(preview, "ArrowDown").defaultPrevented).toBe(false);
  const actions = target.querySelector<HTMLButtonElement>('[aria-label="Actions for Subject 0"]')!;
  key(actions, "ArrowUp");
  expect(selected()).toEqual([]);
});

test("cancel keeps selection; confirmation deletes only the selected representatives", async () => {
  items[0].threadId = "thread";
  items[0].threadCount = 7;
  click(0, { ctrlKey: true });
  click(6, { ctrlKey: true });
  target.querySelector<HTMLButtonElement>('[aria-label="Delete selected emails"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Other messages in their conversations are not affected");
  expect(deleteLocalMails).not.toHaveBeenCalled();
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).toBeNull());
  expect(selected()).toEqual(["m0", "m6"]);
  target.querySelector<HTMLButtonElement>('[aria-label="Delete selected emails"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  confirm();
  await vi.waitFor(() => expect(deleteLocalMails).toHaveBeenCalledExactlyOnceWith("account-a", "inbox", ["m0", "m6"]));
  await vi.waitFor(() => expect(selected()).toEqual([]));
});

test("failed deletion preserves the selection for retry and blocks duplicate submissions", async () => {
  let resolve!: (success: boolean) => void;
  vi.mocked(deleteLocalMails).mockReturnValueOnce(new Promise((done) => { resolve = done; }));
  click(1, { ctrlKey: true });
  target.querySelector<HTMLButtonElement>('[aria-label="Delete selected emails"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  confirm();
  await vi.waitFor(() => expect(rows()[1].disabled).toBe(true));
  key(document.body, "ArrowDown");
  expect(deleteLocalMails).toHaveBeenCalledTimes(1);
  resolve(false);
  await vi.waitFor(() => expect(rows()[1].disabled).toBe(false));
  expect(selected()).toEqual(["m1"]);
});

test.each(["mailbox", "domain", "active"] as const)("changing %s clears the selection and any pending confirmation", async (field) => {
  click(1, { ctrlKey: true });
  target.querySelector<HTMLButtonElement>('[aria-label="Delete selected emails"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  flushSync(() => {
    if (field === "mailbox") controls.mailbox = "sent";
    else if (field === "domain") controls.domain = "example.com";
    else controls.active = false;
  });
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).toBeNull());
  expect(selected()).toEqual([]);
  expect(deleteLocalMails).not.toHaveBeenCalled();
});

test("an in-flight deletion stays scoped to its original mailbox after navigation", async () => {
  let resolve!: (success: boolean) => void;
  vi.mocked(deleteLocalMails).mockReturnValueOnce(new Promise((done) => { resolve = done; }));
  click(1, { ctrlKey: true });
  target.querySelector<HTMLButtonElement>('[aria-label="Delete selected emails"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  confirm();
  await vi.waitFor(() => expect(deleteLocalMails).toHaveBeenCalled());
  flushSync(() => { controls.mailbox = "sent"; });
  resolve(true);
  await vi.waitFor(() => expect(rows()[0].disabled).toBe(false));
  expect(deleteLocalMails).toHaveBeenCalledExactlyOnceWith("account-a", "inbox", ["m1"]);
  expect(goto).not.toHaveBeenCalled();
  expect(selected()).toEqual([]);
});

test("confirmation snapshots IDs even if new rows arrive before confirmation", async () => {
  click(1, { ctrlKey: true });
  click(6, { ctrlKey: true });
  target.querySelector<HTMLButtonElement>('[aria-label="Delete selected emails"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  items = [{ ...items[0], id: "new" }, ...items];
  handlers.get("inbox-updated")!({ payload: { accountId: "account-a" } });
  await vi.waitFor(() => expect(rows()).toHaveLength(10));
  confirm();
  await vi.waitFor(() => expect(deleteLocalMails).toHaveBeenCalledExactlyOnceWith("account-a", "inbox", ["m1", "m6"]));
});

test("search clears selection and background updates prune vanished IDs", async () => {
  click(1, { ctrlKey: true });
  click(6, { ctrlKey: true });
  items = items.filter((item) => item.id !== "m1");
  handlers.get("inbox-updated")!({ payload: { accountId: "account-a" } });
  await vi.waitFor(() => expect(selected()).toEqual(["m6"]));
  const input = target.querySelector<HTMLInputElement>("input")!;
  input.value = "query";
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await vi.waitFor(() => expect(selected()).toEqual([]));
});
