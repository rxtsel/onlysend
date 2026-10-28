// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { mount, unmount } from "svelte";
import Reader from "./__tests__/mail-reader-harness.svelte";
import { getInboundEmail } from "$lib/shared/inbound";
import { getSentEmail } from "$lib/shared/sent";
import { listEmailAttachments } from "$lib/shared/attachments";

vi.mock("$lib/shared/inbound", () => ({ getInboundEmail: vi.fn() }));
vi.mock("$lib/shared/sent", () => ({ getSentEmail: vi.fn() }));
vi.mock("$lib/shared/attachments", async (original) => ({ ...await original<typeof import("$lib/shared/attachments")>(), listEmailAttachments: vi.fn().mockResolvedValue([]) }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn() } }));

let target: HTMLElement;
let instance: ReturnType<typeof mount>;
beforeEach(() => {
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });

test("received attachments sit below the content and leave the subject at full width", async () => {
  const attachment = { id: "file", filename: "invoice.pdf", contentType: "application/pdf", size: 100 };
  vi.mocked(listEmailAttachments).mockResolvedValueOnce([attachment]);
  vi.mocked(getInboundEmail).mockResolvedValue({
    id: "message", from: "sender@example.com", to: ["me@example.com"],
    subject: "Private subject", createdAt: "2026-01-01T00:00:00Z",
    html: null, text: "Private body", attachments: [attachment], replyTo: ["reply@example.com"],
  });
  instance = mount(Reader, { target });
  await vi.waitFor(() => expect(target.querySelector('[aria-label="Attachments"]')).not.toBeNull());
  const header = target.querySelector("header")!;
  const attachments = target.querySelector('[aria-label="Attachments"]')!;
  expect(header.querySelector("h1")?.textContent).toBe("Private subject");
  expect(header.querySelector("h1")?.parentElement).toBe(header);
  expect(header.contains(attachments)).toBe(false);
  expect(attachments.classList.contains("@container/attachments")).toBe(true);
  expect(attachments.querySelector("ul")?.classList.contains("@xl/attachments:grid-cols-2")).toBe(true);
  expect(attachments.previousElementSibling?.textContent).toContain("Private body");
  expect(target.querySelector("article")?.lastElementChild).toBe(attachments);
  expect(target.textContent).toContain("reply@example.com");
});

test("sent mail does not accidentally offer a reply to the user's own From address", async () => {
  vi.mocked(getSentEmail).mockResolvedValue({
    id: "message", from: "me@example.com", to: ["recipient@example.com"],
    subject: "Sent subject", createdAt: "2026-01-01T00:00:00Z", html: null,
    text: "Sent body", cc: [], bcc: [], replyTo: null,
  });
  instance = mount(Reader, { target, props: { mailbox: "sent" } });
  await vi.waitFor(() => expect(target.querySelector("h1")?.textContent).toBe("Sent subject"));
  expect(target.querySelector('[aria-label="Reply to this message"]')).toBeNull();
});
