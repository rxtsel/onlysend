// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { goto } from "$app/navigation";
import { invoke } from "@tauri-apps/api/core";
import Composer from "./__tests__/composer-harness.svelte";
import { prepareReply } from "$lib/shared/reply";
import { listFromEmails } from "$lib/shared/from-emails";
import { registerDraftGuard } from "$lib/features/auth/draft-navigation";

const { pageState } = vi.hoisted(() => ({ pageState: { url: new URL("https://app.test/mail/account-a/composer") } }));
vi.mock("$app/state", () => ({ page: pageState }));
vi.mock("$app/navigation", () => ({ goto: vi.fn(), beforeNavigate: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("$lib/shared/reply", () => ({ prepareReply: vi.fn() }));
vi.mock("$lib/shared/from-emails", async (original) => ({ ...await original<typeof import("$lib/shared/from-emails")>(), listFromEmails: vi.fn() }));
vi.mock("$lib/features/auth/draft-navigation", () => ({ registerDraftGuard: vi.fn(() => vi.fn()), allowAccountNavigation: vi.fn(() => true) }));
vi.mock("@/lib/components/editor", async () => ({ Editor: (await import("./__tests__/composer-editor-probe.svelte")).default }));
vi.mock("svelte-sonner", () => ({ toast: { loading: vi.fn(), dismiss: vi.fn(), success: vi.fn(), error: vi.fn() } }));

const emailId = "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1";
const context = { to: ["reply@example.com"], subject: "Re: Invoice", fromCandidates: ["ME@example.com"] };
let target: HTMLElement;
let instance: ReturnType<typeof mount> | null;
beforeEach(() => {
  pageState.url = new URL(`https://app.test/mail/account-a/composer?reply=${emailId}`);
  vi.mocked(listFromEmails).mockReset().mockResolvedValue([
    { id: "default", label: "Default", address: "default@example.com", isDefault: true },
    { id: "me", label: "Me", address: "me@example.com", isDefault: false },
  ]);
  vi.mocked(prepareReply).mockReset().mockResolvedValue(context);
  vi.mocked(invoke).mockReset().mockResolvedValue(undefined);
  vi.mocked(registerDraftGuard).mockClear();
  vi.mocked(goto).mockClear();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(async () => {
  if (instance) await unmount(instance);
  instance = null;
  document.body.replaceChildren();
  vi.restoreAllMocks();
});
function open() { instance = mount(Composer, { target }); flushSync(); }
function input(selector: string) { return target.querySelector<HTMLInputElement>(selector)!; }
async function ready() {
  await vi.waitFor(() => expect(target.querySelector("[data-composer-ready]")?.textContent).toBe("true"));
}
function body(value: string) {
  const editor = target.querySelector<HTMLTextAreaElement>("textarea")!;
  flushSync(() => { editor.value = value; editor.dispatchEvent(new Event("input", { bubbles: true })); });
}

test("reply reuses the composer, locks its subject and chooses the matching sender", async () => {
  open();
  expect(target.querySelector("[data-composer-ready]")?.textContent).toBe("false");
  await ready();
  expect(prepareReply).toHaveBeenCalledExactlyOnceWith("account-a", emailId);
  expect(input("#subject").value).toBe("Re: Invoice");
  expect(input("#subject").readOnly).toBe(true);
  expect(input('input[type="hidden"][name="to"]').value).toBe("reply@example.com");
  expect(target.querySelector("#from")?.textContent).toContain("Me <me@example.com>");
  expect(document.activeElement).toBe(target.querySelector("textarea"));
  expect(target.querySelector<HTMLTextAreaElement>("textarea")?.value).toBe("");
  const guard = vi.mocked(registerDraftGuard).mock.calls[0][1];
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  expect(guard()).toBe(true); // Prefilled fields are not unsaved edits.
  expect(confirm).not.toHaveBeenCalled();
  body("<p>My reply</p>");
  expect(guard()).toBe(false);
  flushSync(() => target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", { accountId: "account-a", data: {
    replyEmailId: emailId, subject: "Re: Invoice", to: ["reply@example.com"], from: "Me <me@example.com>", content: "<p>My reply</p>", attachments: [], cc: [], bcc: [],
  } }));
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/mail/account-a/sent"));
});

test("ordinary messages keep an editable subject and the default sender", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  expect(prepareReply).not.toHaveBeenCalled();
  expect(input("#subject").readOnly).toBe(false);
  expect(input("#subject").value).toBe("");
  expect(target.querySelector('[name="replyTo"]')).toBeNull();
  expect(target.querySelector('[name="messageId"]')).toBeNull();
  expect(target.textContent).not.toContain("Add message ID");
  expect(target.querySelector("#from")?.textContent).toContain("Default <default@example.com>");
});

test.each([false, true])("composer fields follow From, To, Subject without nested cards (reply: %s)", async (isReply) => {
  if (!isReply) pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  const labels = () => Array.from(target.querySelectorAll("form label")).map((label) => label.textContent?.trim());
  expect(labels()).toEqual(["From", "To", "Subject", "Content"]);
  expect(target.querySelector("form")?.classList.contains("overflow-hidden")).toBe(false);
  const group = target.querySelector('form [data-slot="field-group"]')!;
  expect(group.classList.contains("border")).toBe(false);
  expect(group.classList.contains("rounded-xl")).toBe(false);
  const from = target.querySelector("#from")!;
  expect(from.classList.contains("border")).toBe(true);
  expect(from.classList.contains("w-[180px]")).toBe(true);
  expect(from.closest('[data-slot="field"]')?.getAttribute("data-orientation")).toBe("vertical");
  const toggleGroup = target.querySelector('[data-slot="toggle-group"]')!;
  expect(toggleGroup.getAttribute("data-variant")).toBe("outline");
  expect(from.closest('[data-slot="field"]')?.parentElement?.contains(toggleGroup)).toBe(true);
  expect(input("#subject").classList.contains("border")).toBe(true);
  expect(input("#subject").classList.contains("h-9")).toBe(true);
  expect(input("#subject").classList.contains("h-8")).toBe(false);
  const to = input("#to");
  expect(to.parentElement?.classList.contains("border")).toBe(true);
  const toggles = Array.from(target.querySelectorAll<HTMLButtonElement>('[data-slot="toggle-group-item"]'));
  flushSync(() => { toggles[0].click(); toggles[1].click(); });
  expect(labels()).toEqual(["From", "To", "Cc", "Bcc", "Subject", "Content"]);
  expect(input("#cc").parentElement?.classList.contains("border")).toBe(true);
  expect(input("#bcc").parentElement?.classList.contains("border")).toBe(true);
  expect(to.closest('[data-slot="field"]')?.parentElement).toBe(input("#cc").closest('[data-slot="field"]')?.parentElement);
});

test("Tab from Subject goes to the writer; Shift-Tab keeps native backwards navigation", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  const subject = input("#subject");
  subject.focus();
  const backwards = new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, bubbles: true, cancelable: true });
  subject.dispatchEvent(backwards);
  expect(backwards.defaultPrevented).toBe(false);
  const forwards = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
  subject.dispatchEvent(forwards);
  expect(forwards.defaultPrevented).toBe(true);
  expect(document.activeElement).toBe(target.querySelector("textarea"));
});

test("composer fields still show validation errors and do not submit invalid drafts", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  flushSync(() => target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  expect(input("#to").getAttribute("aria-invalid")).toBe("true");
  expect(input("#subject").getAttribute("aria-invalid")).toBe("true");
  expect(target.querySelectorAll('[data-slot="field-error"]').length).toBeGreaterThanOrEqual(2);
  expect(invoke).not.toHaveBeenCalled();
});

test("missing reply details block sending and remain retryable without losing the source", async () => {
  vi.mocked(prepareReply).mockRejectedValueOnce("Reply metadata is unavailable offline");
  open();
  await vi.waitFor(() => expect(target.querySelector('[role="alert"]')?.textContent).toContain("unavailable offline"));
  expect(target.querySelector("form")).toBeNull();
  expect(target.querySelector("[data-composer-ready]")?.textContent).toBe("false");
  expect(invoke).not.toHaveBeenCalled();
  flushSync(() => target.querySelector<HTMLButtonElement>("button")!.click());
  await ready();
  expect(input("#subject").readOnly).toBe(true);
});

test("a reply cannot initialize a composer after it has been closed", async () => {
  let resolve!: (value: typeof context) => void;
  vi.mocked(prepareReply).mockReturnValueOnce(new Promise((done) => { resolve = done; }));
  open();
  await unmount(instance!);
  instance = null;
  resolve(context);
  await Promise.resolve();
  flushSync();
  expect(target.querySelector("form")).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});
