import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { loadMailList, getMailArchiveStatus, refreshMailbox, getMailThread, setMailsDeleted } from "./local-mail";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue(null); });

test("bulk deletion passes explicit ownership and message IDs to one native call", async () => {
  await setMailsDeleted("a", "inbox", ["first", "second"], true);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("set_mail_deleted_many", {
    accountId: "a", mailbox: "inbox", emailIds: ["first", "second"], deleted: true,
  });
});

test("thread lookup passes explicit account, mailbox and email ownership", async () => {
  await getMailThread("a", "inbox", "message-id");
  expect(invoke).toHaveBeenCalledExactlyOnceWith("get_mail_thread", { accountId: "a", mailbox: "inbox", emailId: "message-id" });
});

test("manual refresh and status are delegated to the native account/mailbox", async () => {
  await refreshMailbox("b", "inbox");
  expect(invoke).toHaveBeenCalledWith("refresh_mailbox", { accountId: "b", mailbox: "inbox" });
  await getMailArchiveStatus("b", "sent");
  expect(invoke).toHaveBeenLastCalledWith("get_mail_archive_status", { accountId: "b", mailbox: "sent" });
});

test("list filtering, offline fallback and cursor parsing are delegated in one native call", async () => {
  const request = { mailbox: "inbox" as const, limit: 50, after: null, domain: "example.com", search: "invoice", refresh: false };
  const result = { items: [], hasMore: false, nextCursor: null, domains: ["example.com"], cache: { downloadedAt: 123, error: null } };
  vi.mocked(invoke).mockResolvedValue(result);
  expect(await loadMailList("b", request)).toEqual(result);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("load_mail_list", { accountId: "b", request });
});

test("local continuation is opaque to Svelte and never parsed or used as a Resend cursor", async () => {
  const request = { mailbox: "sent" as const, limit: 50, after: "opaque-local-cursor", domain: null, search: null, refresh: false };
  await loadMailList("a", request);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("load_mail_list", { accountId: "a", request });
});
