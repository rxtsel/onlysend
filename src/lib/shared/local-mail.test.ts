import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { getCachedMailPage, getLocalArchivePage, loadArchivePage, getMailArchiveStatus, queryLocalMail, syncMailbox } from "./local-mail";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockReset().mockResolvedValue(null));

test("disk snapshot requests explicitly carry account, mailbox and remote cursor", async () => {
  await getCachedMailPage("account-b", "inbox", 16, "remote-id");
  expect(invoke).toHaveBeenCalledWith("get_cached_mail_page", { accountId: "account-b", mailbox: "inbox", limit: 16, after: "remote-id" });
});

test("local query uses a separate keyset and structured filters, never SQL", async () => {
  const query = { mailbox: "sent" as const, sort: "subjectAsc" as const, search: "invoice", domain: "example.com", after: { value: "invoice", emailId: "id", queryKey: "query" } };
  await queryLocalMail("account-b", query);
  expect(invoke).toHaveBeenCalledWith("query_local_mail", { accountId: "account-b", query });
});

test("background retry and progress are account/mailbox scoped", async () => {
  await syncMailbox("account-b", "inbox", true);
  expect(invoke).toHaveBeenCalledWith("sync_mailbox", { accountId: "account-b", mailbox: "inbox", force: true, limit: 16 });
  await getMailArchiveStatus("account-b", "sent");
  expect(invoke).toHaveBeenLastCalledWith("get_mail_archive_status", { accountId: "account-b", mailbox: "sent" });
});

const local = {
  items: [{ id: "retained-copy" }], hasMore: true,
  nextCursor: { value: "123", emailId: "retained-copy", queryKey: "account-query" },
  sync: null, isPartial: true, downloadedAt: 123,
};

test("visible head comes from SQLite and preserves mail absent from the remote head", async () => {
  vi.mocked(invoke).mockResolvedValue(local);
  const refresh = vi.fn(async () => ({ items: [{ id: "remote-head" }], hasMore: false, nextCursor: null }));
  const page = await loadArchivePage("a", "inbox", 16, null, refresh);
  expect(page.items).toEqual(local.items);
  expect(page.nextCursor).toBe(JSON.stringify(local.nextCursor));
  expect(refresh).toHaveBeenCalledWith(16);
});

test("local continuation is never sent to Resend", async () => {
  vi.mocked(invoke).mockResolvedValue(local);
  const refresh = vi.fn();
  await loadArchivePage("a", "inbox", 16, JSON.stringify(local.nextCursor), refresh);
  expect(refresh).not.toHaveBeenCalled();
  expect(invoke).toHaveBeenCalledWith("query_local_mail", { accountId: "a", query: { mailbox: "inbox", limit: 16, after: local.nextCursor } });
});

test("remote failure displays the retained archive but an uninitialized archive is an error", async () => {
  vi.mocked(invoke).mockResolvedValue(local);
  const refresh = vi.fn(async () => { throw new Error("offline"); });
  const page = await loadArchivePage("a", "inbox", 16, null, refresh);
  expect(page.cache?.error).toContain("offline");
  expect(page.items).toEqual(local.items);
  vi.mocked(invoke).mockResolvedValue({ ...local, items: [], downloadedAt: null });
  await expect(loadArchivePage("a", "inbox", 16, null, refresh)).rejects.toThrow("offline");
});

test("empty schema is not hydrated as known empty mail history", async () => {
  vi.mocked(invoke).mockResolvedValue({ ...local, items: [], downloadedAt: null });
  expect(await getLocalArchivePage("a", "sent", 16)).toBeNull();
  vi.mocked(invoke).mockResolvedValue({ ...local, items: [], downloadedAt: 123 });
  expect((await getLocalArchivePage("a", "sent", 16))?.cache?.downloadedAt).toBe(123);
});
