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

test("shift and ctrl/cmd clicks preserve selection and preview the latest selected row", () => {
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
  expect(goto).toHaveBeenLastCalledWith("/mail/account-a/inbox/m6", { keepFocus: true, noScroll: true });
  expect(onSelectMail).toHaveBeenLastCalledWith(expect.objectContaining({ id: "m6" }), "inbox");
  expect(target.textContent).toContain("1 Selected");
  expect(target.querySelector('[aria-label="Refresh mail"]')).toBeNull();
});

test("a normal click opens the preview and anchors shift arrows with focus and contraction", () => {
  click(2);
  expect(goto).toHaveBeenCalledExactlyOnceWith("/mail/account-a/inbox/m2", { keepFocus: true, noScroll: true });
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
  expect(goto).toHaveBeenCalledTimes(4);
  expect(goto).toHaveBeenLastCalledWith("/mail/account-a/inbox/m3", { keepFocus: true, noScroll: true });
  expect(onSelectMail).toHaveBeenLastCalledWith(expect.objectContaining({ id: "m3" }), "inbox");
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

test.each(["Delete", "Backspace"])("%s opens bulk confirmation without deleting until confirmed", async (deleteKey) => {
  click(0, { ctrlKey: true });
  click(6, { metaKey: true });
  const event = key(document.body, deleteKey, false);
  expect(event.defaultPrevented).toBe(true);
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  expect(deleteLocalMails).not.toHaveBeenCalled();
  key(document.body, deleteKey, false);
  expect(document.querySelectorAll('[role="alertdialog"]')).toHaveLength(1);
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).toBeNull());
  expect(selected()).toEqual(["m0", "m6"]);
  key(rows()[6], deleteKey, false);
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  confirm();
  await vi.waitFor(() => expect(deleteLocalMails).toHaveBeenCalledExactlyOnceWith("account-a", "inbox", ["m0", "m6"]));
});

test("delete shortcuts leave text editing, dialogs, menus and modified keys alone", () => {
  click(0, { ctrlKey: true });
  const input = target.querySelector<HTMLInputElement>("input")!;
  const editable = document.createElement("div");
  editable.setAttribute("contenteditable", "");
  const nested = document.createElement("span");
  editable.append(nested);
  document.body.append(editable);
  const menu = document.createElement("div");
  menu.setAttribute("role", "menu");
  document.body.append(menu);
  const dialog = document.createElement("div");
  dialog.setAttribute("role", "dialog");
  document.body.append(dialog);
  for (const element of [input, nested, menu, dialog]) {
    for (const deleteKey of ["Delete", "Backspace"]) expect(key(element, deleteKey, false).defaultPrevented).toBe(false);
  }
  for (const modifiers of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }, { shiftKey: true }, { repeat: true }]) {
    document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true, cancelable: true, ...modifiers }));
  }
  flushSync();
  expect(document.querySelector('[role="alertdialog"]')).toBeNull();
  expect(deleteLocalMails).not.toHaveBeenCalled();
  expect(selected()).toEqual(["m0"]);
});

test("Delete does nothing without selection or while the list is inactive", () => {
  expect(key(document.body, "Delete", false).defaultPrevented).toBe(false);
  click(0, { ctrlKey: true });
  flushSync(() => { controls.active = false; });
  expect(key(document.body, "Delete", false).defaultPrevented).toBe(false);
  expect(document.querySelector('[role="alertdialog"]')).toBeNull();
});

test.each(["ctrlKey", "metaKey"] as const)("%s click previews only selected messages when toggling off", (modifier) => {
  click(1, { [modifier]: true });
  click(6, { [modifier]: true });
  expect(onSelectMail).toHaveBeenLastCalledWith(expect.objectContaining({ id: "m6" }), "inbox");
  click(6, { [modifier]: true });
  expect(selected()).toEqual(["m1"]);
  expect(onSelectMail).toHaveBeenLastCalledWith(expect.objectContaining({ id: "m1" }), "inbox");
  const previews = onSelectMail.mock.calls.length;
  click(1, { [modifier]: true });
  expect(selected()).toEqual([]);
  expect(onSelectMail).toHaveBeenCalledTimes(previews);
});

test("rapid shift arrows update the preview in order while keeping the list focus", () => {
  click(2);
  for (let i = 2; i < 7; i++) key(rows()[i], "ArrowDown");
  expect(selected()).toEqual(["m2", "m3", "m4", "m5", "m6", "m7"]);
  expect(onSelectMail.mock.calls.map(([mail]) => mail.id)).toEqual(["m2", "m3", "m4", "m5", "m6", "m7"]);
  expect(document.activeElement).toBe(rows()[7]);
  expect(vi.mocked(goto).mock.calls.every(([, options]) => options?.keepFocus && options?.noScroll)).toBe(true);
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
  expect(goto).toHaveBeenCalledExactlyOnceWith("/mail/account-a/inbox/m1", { keepFocus: true, noScroll: true });
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

test("marking an inbox message read removes only its avatar badge", async () => {
  expect(rows().every((row) => row.querySelector('[title="Unread"]'))).toBe(true);
  handlers.get("inbox-read")!({ payload: { accountId: "other-account", emailId: "m0" } });
  flushSync();
  expect(rows()[0].querySelector('[title="Unread"]')).not.toBeNull();
  handlers.get("inbox-read")!({ payload: { accountId: "account-a", emailId: "m0" } });
  await vi.waitFor(() => expect(rows()[0].querySelector('[title="Unread"]')).toBeNull());
  expect(rows()[1].querySelector('[title="Unread"]')).not.toBeNull();
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
