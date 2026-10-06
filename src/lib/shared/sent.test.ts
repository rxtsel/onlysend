import { beforeEach, describe, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { emailCache } from "@/lib/features/sending/email-cache.svelte";
import { getSentEmail, listSentEmails } from "./sent";
import type { SentEmail } from "../types/sent.type";
import type { EmailPage } from "./email-page";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mail = { id: "same-id", subject: "Account A" } as SentEmail;
const empty: EmailPage<SentEmail> = { items: [], hasMore: false, nextCursor: null };
const page: EmailPage<SentEmail> = { items: [mail], hasMore: true, nextCursor: mail.id };

beforeEach(() => { vi.mocked(invoke).mockReset(); emailCache.clear("a"); emailCache.clear("b"); });

describe("sent cache account boundary", () => {
  test("same message ID in two accounts has separate cache entries", async () => {
    emailCache.set("a", mail.id, mail);
    const other = { ...mail, subject: "Account B" };
    vi.mocked(invoke).mockResolvedValue(other);
    expect(await getSentEmail("b", mail.id)).toEqual(other);
    expect(invoke).toHaveBeenCalledWith("get_sent_email", { accountId: "b", emailId: mail.id });
    expect(await getSentEmail("a", mail.id)).toEqual(mail);
    expect(emailCache.get("b", mail.id)).toEqual(other);
  });

  test("sent pagination sends an explicit account and after cursor without an offset", async () => {
    vi.mocked(invoke).mockResolvedValue(empty);
    await listSentEmails("a", 16, "previous-message");
    expect(invoke).toHaveBeenCalledExactlyOnceWith("list_sent_emails", { accountId: "a", limit: 16, after: "previous-message" });
  });

  test("first-page cache retains hasMore and cursor, scoped by account and page size", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(page).mockResolvedValue(empty);
    expect(await listSentEmails("a", 16)).toEqual(page);
    expect(await listSentEmails("a", 16)).toEqual(page);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(await listSentEmails("a", 32)).toEqual(empty);
    expect(await listSentEmails("b", 16)).toEqual(empty);
    expect(invoke).toHaveBeenCalledTimes(3);
  });

  test("load more never reuses or overwrites the first-page cache", async () => {
    emailCache.setList("a", page, 16);
    vi.mocked(invoke).mockResolvedValue(empty);
    expect(await listSentEmails("a", 16, mail.id)).toEqual(empty);
    expect(emailCache.getList("a", 16)).toEqual(page);
    expect(await listSentEmails("a", 16)).toEqual(page);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  test("force refresh bypasses and replaces the cached first page", async () => {
    emailCache.setList("a", page, 16);
    vi.mocked(invoke).mockResolvedValue(empty);
    expect(await listSentEmails("a", 16, null, true)).toEqual(empty);
    expect(emailCache.getList("a", 16)).toEqual(empty);
  });

  test("logout invalidates a populated list including continuation metadata", async () => {
    emailCache.setList("a", page, 16);
    emailCache.clear("a");
    vi.mocked(invoke).mockResolvedValue(empty);
    expect(await listSentEmails("a", 16)).toEqual(empty);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  test("late list response cannot repopulate a logged-out account's cache", async () => {
    let resolve!: (value: EmailPage<SentEmail>) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
    const pending = listSentEmails("a", 16);
    emailCache.clear("a");
    emailCache.setList("b", empty, 16);
    resolve(page);
    await pending;
    expect(emailCache.getList("a", 16)).toBeNull();
    expect(emailCache.getList("b", 16)).toEqual(empty);
  });

  test("late detail response cannot poison the next account's cache", async () => {
    let resolve!: (value: SentEmail) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
    const pending = getSentEmail("a", mail.id);
    emailCache.clear("a");
    resolve(mail);
    await pending;
    expect(emailCache.get("a", mail.id)).toBeUndefined();
  });
});
