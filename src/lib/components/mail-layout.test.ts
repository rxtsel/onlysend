// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Layout from "./__tests__/mail-layout-harness.svelte";
import { prepareReply, type ReplyContext } from "$lib/shared/reply";

const { pageState } = vi.hoisted(() => ({ pageState: {
  url: new URL("https://app.test/mail/account-a/inbox/message"), params: { id: "message" as string | undefined },
} }));
vi.mock("$app/state", () => ({ page: pageState }));
vi.mock("$app/navigation", () => ({ goto: vi.fn(), beforeNavigate: vi.fn() }));
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
beforeEach(() => { target = document.createElement("div"); document.body.append(target); });
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });

test("Reply occupies the right of the Message toolbar rather than the subject", () => {
  pageState.url = new URL(`https://app.test/mail/account-a/inbox/${emailId}`);
  pageState.params.id = emailId;
  instance = mount(Layout, { target });
  flushSync();
  const header = target.querySelector("main > header")!;
  const reply = header.querySelector<HTMLAnchorElement>('[aria-label="Reply to this message"]')!;
  expect(reply.getAttribute("href")).toBe(`/mail/account-a/composer?reply=${emailId}`);
  expect(header.lastElementChild).toBe(reply);
  expect(reply.previousElementSibling?.textContent).toBe("Message");
  expect(reply.previousElementSibling?.classList.contains("flex-1")).toBe(true);
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
