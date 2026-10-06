import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listInboundEmails, type InboundEmail } from "./inbound";
import { createEmailList } from "./email-list.svelte";
import type { EmailPage } from "./email-page";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockReset());
const mail = { id: "received-one", domains: ["a.example", "b.example"] } as InboundEmail;
const head: EmailPage<InboundEmail> = { items: [mail], hasMore: true, nextCursor: mail.id };
const end: EmailPage<InboundEmail> = { items: [{ ...mail, id: "received-two" }], hasMore: false, nextCursor: null };

test("inbox wrappers preserve the API continuation, including a short page", async () => {
  vi.mocked(invoke).mockResolvedValue(head);
  expect(await listInboundEmails("a", 16)).toEqual(head);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("list_inbound_emails", { accountId: "a", limit: 16, after: null });
});

test("inbox load-more passes the previous page's cursor and preserves all receiving domains", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(head).mockResolvedValueOnce(end);
  const list = createEmailList((limit, after) => listInboundEmails("a", limit, after));
  await list.refresh();
  await list.loadMore();
  expect(invoke).toHaveBeenNthCalledWith(2, "list_inbound_emails", { accountId: "a", limit: 16, after: mail.id });
  expect(list.items.map((item) => item.domains)).toEqual([mail.domains, mail.domains]);
  expect(list.hasMore).toBe(false);
});

test("slow page from account A cannot advance account B's inbox continuation", async () => {
  let resolveA!: (page: EmailPage<InboundEmail>) => void;
  vi.mocked(invoke).mockResolvedValueOnce(head)
    .mockImplementationOnce(() => new Promise((done) => { resolveA = done; }))
    .mockResolvedValueOnce({ ...head, nextCursor: "b-cursor" }).mockResolvedValueOnce(end);
  const a = createEmailList((limit, after) => listInboundEmails("a", limit, after));
  const b = createEmailList((limit, after) => listInboundEmails("b", limit, after));
  await a.refresh();
  const pending = a.loadMore();
  a.clearItems();
  await b.refresh();
  resolveA({ ...end, hasMore: true, nextCursor: "obsolete-a-cursor" });
  await pending;
  expect(a.items).toEqual([]);
  await b.loadMore();
  expect(invoke).toHaveBeenLastCalledWith("list_inbound_emails", { accountId: "b", limit: 16, after: "b-cursor" });
});
