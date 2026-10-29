// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { mount, unmount } from "svelte";
import Reader from "./__tests__/mail-reader-harness.svelte";
import { getInboundEmail } from "$lib/shared/inbound";
import { getSentEmail } from "$lib/shared/sent";
import { listEmailAttachments } from "$lib/shared/attachments";
import { getMailThread } from "$lib/shared/local-mail";
import { deleteLocalMail } from "$lib/shared/services/delete-mail";
import { goto } from "$app/navigation";
import { listen } from "@tauri-apps/api/event";

vi.mock("$app/navigation", () => ({ goto: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("$lib/shared/services/delete-mail", () => ({ deleteLocalMail: vi.fn().mockResolvedValue(true) }));
vi.mock("$lib/shared/inbound", () => ({ getInboundEmail: vi.fn() }));
vi.mock("$lib/shared/sent", () => ({ getSentEmail: vi.fn() }));
vi.mock("$lib/shared/local-mail", () => ({ getMailThread: vi.fn().mockResolvedValue([]) }));
vi.mock("$lib/shared/attachments", async (original) => ({ ...await original<typeof import("$lib/shared/attachments")>(), listEmailAttachments: vi.fn().mockResolvedValue([]) }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn() } }));

let target: HTMLElement;
let instance: ReturnType<typeof mount>;
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getMailThread).mockReset().mockResolvedValue([]);
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });

test("shows the list item's cached preview while the full message is loading", async () => {
  vi.mocked(getInboundEmail).mockReturnValueOnce(new Promise(() => {}));
  instance = mount(Reader, { target, props: { preview: {
    accountId: "account-a", mailbox: "inbox", mail: {
      id: "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1", from: "sender@example.test", to: ["me@example.test"],
      subject: "Cached subject", createdAt: "2026-01-01T00:00:00Z", preview: "Cached preview snippet", isRead: true, threadCount: 2,
    },
  } } });
  await vi.waitFor(() => expect(target.querySelector('[aria-label="Loading full message"]')).not.toBeNull());
  expect(target.textContent).toContain("Cached preview snippet");
  expect(target.textContent).toContain("sender");
  expect(target.querySelectorAll('[data-slot="skeleton"]')).toHaveLength(3);
  expect(target.querySelector('[data-slot="collapsible-trigger"] > span')?.getAttribute("style")).toContain("width: 32px");
  expect(target.querySelector("time")?.classList.contains("whitespace-nowrap")).toBe(true);
});

test.each([
  ["inbox", false], ["sent", false], ["inbox", true], ["sent", true],
] as const)("%s loading preview shares the rendered message layout (threaded: %s)", async (mailbox, threaded) => {
  const mail = {
    id: "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1", from: "Sender <sender@example.test>", to: ["me@example.test"],
    subject: "Subject", createdAt: "2026-01-01T00:00:00Z", preview: "Cached snippet", isRead: true, threadCount: threaded ? 2 : 1,
  };
  const full = { ...mail, html: null, text: "First line\nSecond line\nThird line", cc: [], bcc: [], replyTo: [], attachments: [] };
  let resolve!: (value: typeof full) => void;
  const promise = new Promise<typeof full>((done) => { resolve = done; });
  const fetchDetail = mailbox === "inbox" ? vi.mocked(getInboundEmail) : vi.mocked(getSentEmail);
  if (mailbox === "inbox") vi.mocked(getInboundEmail).mockReturnValueOnce(promise);
  else vi.mocked(getSentEmail).mockReturnValueOnce(promise.then((value) => ({ ...value, replyTo: null })));
  vi.mocked(getMailThread).mockResolvedValue(threaded ? [
    { ...mail, mailbox, id: "older", createdAt: "2025-12-31T00:00:00Z" },
    { ...mail, mailbox },
  ] : []);
  instance = mount(Reader, { target, props: { mailbox, preview: { accountId: "account-a", mailbox, mail } } });
  await vi.waitFor(() => expect(target.querySelector('[aria-label="Loading full message"]')).not.toBeNull());
  expect(fetchDetail).toHaveBeenCalledExactlyOnceWith("account-a", mail.id);
  expect(target.textContent?.split("Cached snippet").length).toBe(threaded ? 2 : 1);
  const headerSelector = threaded ? '[data-slot="collapsible-trigger"]' : "header";
  const layout = () => {
    const header = Array.from(target.querySelectorAll<HTMLElement>(headerSelector)).at(-1)!;
    return [header.className, header.parentElement?.className, header.firstElementChild?.getAttribute("style"), header.querySelector("time")?.className, header.querySelector("summary")?.className];
  };
  const loadingLayout = layout();
  const skeleton = target.querySelector('[aria-label="Loading full message"]')!;
  const bodyClass = skeleton.parentElement!.className;
  for (const name of ["py-4", "text-sm", "leading-7"]) expect(skeleton.classList.contains(name)).toBe(true);
  expect(Array.from(skeleton.children).every((line) => line.classList.contains("h-7"))).toBe(true);
  resolve(full);
  await vi.waitFor(() => expect(target.querySelector("pre")?.textContent).toBe(full.text));
  expect(layout()).toEqual(loadingLayout);
  const body = target.querySelector("pre")!;
  expect(body.parentElement!.className).toBe(bodyClass);
  for (const name of ["py-4", "text-sm", "leading-7"]) expect(body.classList.contains(name)).toBe(true);
  expect(target.querySelector('[aria-label="Loading full message"]')).toBeNull();
});

test("received attachments sit below the content and leave the subject at full width", async () => {
  const attachment = { id: "file", filename: "invoice.pdf", contentType: "application/pdf", size: 100 };
  vi.mocked(listEmailAttachments).mockResolvedValueOnce([attachment]);
  vi.mocked(getMailThread).mockResolvedValue([]);
  vi.mocked(getInboundEmail).mockResolvedValue({
    id: "message", from: "sender@example.com", to: ["me@example.com"],
    subject: "Private subject", createdAt: "2026-01-01T00:00:00Z",
    html: null, text: "Private body", attachments: [attachment], replyTo: ["reply@example.com"],
  });
  instance = mount(Reader, { target });
  await vi.waitFor(() => expect(target.querySelector('[aria-label="Attachments"]')).not.toBeNull());
  const header = target.querySelector("header")!;
  const attachments = target.querySelector('[aria-label="Attachments"]')!;
  expect(target.querySelector("[data-slot=collapsible-trigger]")).toBeNull();
  expect(header.contains(attachments)).toBe(false);
  expect(attachments.classList.contains("@container/attachments")).toBe(true);
  expect(attachments.querySelector("ul")?.classList.contains("@xl/attachments:grid-cols-2")).toBe(true);
  expect(target.querySelector("pre")?.textContent).toBe("Private body");
  expect(target.querySelector("article ol")?.contains(attachments)).toBe(true);
  expect(target.textContent).toContain("reply@example.com");
});

test("a thread's Reply action targets the exact message card", async () => {
  const emailId = "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1";
  vi.mocked(getInboundEmail).mockResolvedValue({
    id: emailId, from: "sender@example.test", to: ["me@example.test"],
    subject: "Re: Topic", createdAt: "2026-01-02T00:00:00Z", html: null, text: "Latest reply", attachments: [],
  });
  vi.mocked(getMailThread).mockResolvedValue([
    { id: "root", mailbox: "inbox", from: "author@example.test", to: ["me@example.test"], subject: "Topic", createdAt: "2026-01-01T00:00:00Z", preview: "Original" },
    { id: emailId, mailbox: "inbox", from: "sender@example.test", to: ["me@example.test"], subject: "Re: Topic", createdAt: "2026-01-02T00:00:00Z", preview: "Latest reply" },
  ]);
  instance = mount(Reader, { target });
  await vi.waitFor(() => expect(target.querySelector('[aria-label="Reply to message from sender@example.test"]')).not.toBeNull());
  expect(target.querySelector('[aria-label="Reply to message from sender@example.test"]')?.getAttribute("href"))
    .toBe(`/mail/account-a/composer?reply=${emailId}`);
});

test("thread view keeps the subject above chronological, expandable message cards", async () => {
  vi.mocked(getInboundEmail).mockResolvedValue({
    id: "current", from: "sender@example.test", to: ["me@example.test"],
    subject: "Re: Topic", createdAt: "2026-01-02T00:00:00Z", html: null, text: "Latest reply", attachments: [],
  });
  vi.mocked(getMailThread).mockResolvedValue([
    { id: "root", mailbox: "inbox", from: "author@example.test", to: ["me@example.test"], subject: "Topic", createdAt: "2026-01-01T00:00:00Z", preview: "Original" },
    { id: "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1", mailbox: "inbox", from: "sender@example.test", to: ["me@example.test"], subject: "Re: Topic", createdAt: "2026-01-02T00:00:00Z", preview: "Latest reply" },
    { id: "newest", mailbox: "inbox", from: "newest@example.test", to: ["me@example.test"], subject: "Re: Topic", createdAt: "2026-01-03T00:00:00Z", preview: "Newest reply" },
  ]);
  instance = mount(Reader, { target });
  await vi.waitFor(() => expect(target.querySelector('[aria-label="Conversation messages"]')).not.toBeNull());
  expect(getMailThread).toHaveBeenCalledExactlyOnceWith("account-a", "inbox", "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1");
  const cards = target.querySelectorAll<HTMLButtonElement>('[data-slot="collapsible-trigger"]');
  expect(cards).toHaveLength(3);
  expect(cards[0].textContent).toContain("author");
  expect(cards[0].textContent).toContain("Original");
  expect(cards[0].querySelector("p")?.classList.contains("line-clamp-1")).toBe(true);
  expect(cards[1].textContent).toContain("sender");
  expect(cards[1].textContent).toContain("To me@example.test");
  expect(target.textContent).toContain("Latest reply");
  expect(cards[0].getAttribute("aria-expanded")).toBe("false");
  expect(cards[1].getAttribute("aria-expanded")).toBe("false");
  expect(cards[2].getAttribute("aria-expanded")).toBe("true");
  expect(target.textContent).not.toContain("messages in this conversation");
  const deletes = target.querySelectorAll<HTMLButtonElement>('[aria-label="Delete from this device"]');
  expect(deletes).toHaveLength(3);
  expect(cards[0].contains(deletes[0])).toBe(false);
  expect(cards[0].classList.contains("w-full")).toBe(true);
  expect(cards[0].querySelector("time")).not.toBeNull();
  expect(deletes[0].tagName).toBe("BUTTON");
  expect(deletes[0].classList.contains("right-4")).toBe(true);
  expect(deletes[0].classList.contains("size-6")).toBe(true);
  expect(cards[0].querySelector("svg.absolute")?.classList.contains("right-5")).toBe(true);
  deletes[0].click();
  expect(cards[0].getAttribute("aria-expanded")).toBe("false");
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  expect(deleteLocalMail).not.toHaveBeenCalled();
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')!.click();
  await vi.waitFor(() => expect(deleteLocalMail).toHaveBeenCalledWith("account-a", "inbox", "root"));
  await vi.waitFor(() => expect(target.querySelectorAll('[data-slot="collapsible-trigger"]')).toHaveLength(2));
  expect(goto).not.toHaveBeenCalled();
});

test("deleting the selected email from the list closes its reader only for the owning account and mailbox", async () => {
  const emailId = "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1";
  vi.mocked(getInboundEmail).mockResolvedValue({
    id: emailId, from: "sender@example.test", to: ["me@example.test"],
    subject: "Subject", createdAt: "2026-01-01T00:00:00Z", html: null, text: "Body", attachments: [],
  });
  instance = mount(Reader, { target });
  await vi.waitFor(() => expect(target.querySelector("pre")?.textContent).toBe("Body"));
  const handler = vi.mocked(listen).mock.calls[0][1];
  const event = { event: "mail-deletion-updated", id: 0, payload: { accountId: "other-account", mailbox: "inbox", emailId, deleted: true } };
  handler(event);
  handler({ ...event, payload: { ...event.payload, accountId: "account-a", mailbox: "sent" } });
  expect(goto).not.toHaveBeenCalled();
  const ownedEvent = { ...event, payload: { ...event.payload, accountId: "account-a" } };
  handler(ownedEvent);
  handler(ownedEvent);
  expect(goto).toHaveBeenCalledExactlyOnceWith("/mail/account-a/inbox");
});

test("Sent shows mixed inbox/sent thread messages and never replies to the user's own From address", async () => {
  vi.mocked(getSentEmail).mockResolvedValue({
    id: "message", from: "me@example.com", to: ["recipient@example.com"],
    subject: "Sent subject", createdAt: "2026-01-02T00:00:00Z", html: null,
    text: "Sent body", cc: [], bcc: [], replyTo: null,
  });
  vi.mocked(getMailThread).mockResolvedValue([
    { id: "received", mailbox: "inbox", from: "recipient@example.test", to: ["me@example.test"], subject: "Topic", createdAt: "2026-01-01T00:00:00Z", preview: "Original" },
    { id: "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1", mailbox: "sent", from: "me@example.test", to: ["recipient@example.test"], subject: "Re: Topic", createdAt: "2026-01-02T00:00:00Z", preview: "Reply" },
  ]);
  instance = mount(Reader, { target, props: { mailbox: "sent" } });
  await vi.waitFor(() => expect(target.querySelectorAll('[data-slot="collapsible-trigger"]')).toHaveLength(2));
  expect(getMailThread).toHaveBeenCalledExactlyOnceWith("account-a", "sent", "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1");
  expect(target.querySelector("article")?.textContent).toContain("Sent body");
  expect(target.querySelector('[aria-label="Reply to this message"]')).toBeNull();
});
