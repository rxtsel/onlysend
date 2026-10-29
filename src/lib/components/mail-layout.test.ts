// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Layout from "./__tests__/mail-layout-harness.svelte";
import { prepareReply, type ReplyContext } from "$lib/shared/reply";
import { deleteLocalMail } from "$lib/shared/services/delete-mail";
import { goto } from "$app/navigation";

const { pageState } = vi.hoisted(() => ({ pageState: {
  url: new URL("https://app.test/mail/account-a/inbox/message"), params: { id: "message" as string | undefined },
} }));
vi.mock("$app/state", () => ({ page: pageState }));
vi.mock("$app/navigation", () => ({ goto: vi.fn().mockResolvedValue(undefined), beforeNavigate: vi.fn() }));
vi.mock("$lib/shared/services/delete-mail", () => ({ deleteLocalMail: vi.fn().mockResolvedValue(true) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("$lib/features/auth/account-switch.svelte", () => ({ accountSwitch: { busy: false } }));
vi.mock("$lib/components/app-sidebar.svelte", async () => ({ default: (await import("./__tests__/layout-frame-probe.svelte")).default }));
vi.mock("$lib/components/mail-list.svelte", async () => ({ default: (await import("./__tests__/layout-frame-probe.svelte")).default }));
vi.mock("$lib/components/mail-status-footer.svelte", async () => ({ default: (await import("./__tests__/layout-frame-probe.svelte")).default }));
vi.mock("$lib/components/ui/sidebar", async () => ({ Provider: (await import("./__tests__/layout-frame-probe.svelte")).default }));
vi.mock("$lib/shared/reply", () => ({ prepareReply: vi.fn() }));
vi.mock("$lib/shared/from-emails", async (original) => ({ ...await original<typeof import("$lib/shared/from-emails")>(), listFromEmails: vi.fn().mockResolvedValue([]) }));
vi.mock("$lib/features/auth/draft-navigation", () => ({ registerDraftGuard: vi.fn(() => vi.fn()), allowAccountNavigation: vi.fn(() => true) }));
vi.mock("@/lib/components/editor", async () => ({ Editor: (await import("./__tests__/composer-editor-probe.svelte")).default }));
vi.mock("svelte-sonner", () => ({ toast: { loading: vi.fn(), dismiss: vi.fn(), success: vi.fn(), error: vi.fn() } }));

const emailId = "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1";
let target: HTMLElement;
let instance: ReturnType<typeof mount>;
beforeEach(() => { vi.clearAllMocks(); target = document.createElement("div"); document.body.append(target); });
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });

test("keeps the subject full-width while thread status is unknown", () => {
  pageState.url = new URL(`https://app.test/mail/account-a/inbox/${emailId}`);
  pageState.params.id = emailId;
  instance = mount(Layout, { target });
  flushSync();
  const header = target.querySelector("main > header")!;
  const subject = header.querySelector("h1")!;
  expect(subject.textContent).toBe("Loading subject");
  expect(subject.querySelector('[data-slot="skeleton"]')).not.toBeNull();
  expect(subject.classList.contains("flex-1")).toBe(true);
  expect(header.querySelector('[aria-label="Reply to this message"]')).toBeNull();
  expect(header.classList.contains("items-start")).toBe(true);
});

test("single-message toolbar places icon-only delete after reply and persists before leaving", async () => {
  pageState.url = new URL(`https://app.test/mail/account-a/inbox/${emailId}`);
  pageState.params.id = emailId;
  instance = mount(Layout, { target, props: { threaded: false } });
  flushSync();
  const header = target.querySelector("main > header")!;
  const reply = header.querySelector('[aria-label="Reply to this message"]')!;
  const remove = header.querySelector<HTMLButtonElement>('[aria-label="Delete from this device"]')!;
  expect(remove.textContent?.trim()).toBe("");
  expect(reply.compareDocumentPosition(remove) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(header.querySelector("h1")?.classList.contains("flex-1")).toBe(true);
  expect(remove.tagName).toBe("BUTTON");
  expect(reply.classList.contains("h-7")).toBe(true);
  expect(remove.classList.contains("size-7")).toBe(true);
  expect(remove.parentElement?.classList.contains("items-center")).toBe(true);
  expect(remove.parentElement?.classList.contains("gap-1")).toBe(true);
  expect(remove.parentElement?.classList.contains("self-start")).toBe(true);
  remove.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  expect(deleteLocalMail).not.toHaveBeenCalled();
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')!.click();
  await vi.waitFor(() => expect(deleteLocalMail).toHaveBeenCalledWith("account-a", "inbox", emailId));
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/mail/account-a/inbox"));
});

test.each(["inbox", "sent"])("%s aligns subject line height with the top-aligned actions", (mailbox) => {
  pageState.url = new URL(`https://app.test/mail/account-a/${mailbox}/${emailId}`);
  pageState.params.id = emailId;
  instance = mount(Layout, { target, props: { threaded: false } });
  flushSync();
  const header = target.querySelector("main > header")!;
  const remove = header.querySelector('[aria-label="Delete from this device"]')!;
  expect(header.querySelector("h1")?.classList.contains("leading-7")).toBe(true);
  expect(remove.classList.contains("size-7")).toBe(true);
  expect(remove.parentElement?.classList.contains("self-start")).toBe(true);
});

test("thread toolbar leaves deletion to the individual message headers", () => {
  pageState.url = new URL(`https://app.test/mail/account-a/inbox/${emailId}`);
  pageState.params.id = emailId;
  instance = mount(Layout, { target, props: { threaded: true } });
  flushSync();
  expect(target.querySelector('main > header [aria-label="Delete from this device"]')).toBeNull();
});

test("the Sent toolbar does not reply to the user's own address", () => {
  pageState.url = new URL(`https://app.test/mail/account-a/sent/${emailId}`);
  pageState.params.id = emailId;
  instance = mount(Layout, { target });
  flushSync();
  expect(target.querySelector('[aria-label="Reply to this message"]')).toBeNull();
});

test("Send is disabled until reply preparation finishes, with a back link to the original", async () => {
  let resolve!: (context: ReplyContext) => void;
  vi.mocked(prepareReply).mockReturnValueOnce(new Promise((done) => { resolve = done; }));
  pageState.url = new URL(`https://app.test/mail/account-a/composer?reply=${emailId}`);
  pageState.params.id = undefined;
  instance = mount(Layout, { target, props: { composer: true } });
  flushSync();
  const send = Array.from(target.querySelectorAll<HTMLButtonElement>("main > header button"))
    .find((button) => button.textContent?.trim() === "Send")!;
  expect(send.disabled).toBe(true);
  expect(target.querySelector('[aria-label="Back to original message"]')?.getAttribute("href")).toBe(`/mail/account-a/inbox/${emailId}`);
  resolve({ to: ["sender@example.com"], subject: "Re: Invoice", fromCandidates: [] });
  await vi.waitFor(() => expect(send.disabled).toBe(false));
  expect(target.querySelector<HTMLInputElement>("#subject")?.readOnly).toBe(true);
});
