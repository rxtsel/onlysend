import { beforeEach, describe, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { emailCache } from "@/lib/features/sending/email-cache.svelte";
import { getSentEmail, listSentEmails } from "./sent";
import type { SentEmail } from "../types/sent.type";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mail = { id: "same-id", subject: "Account A" } as SentEmail;

beforeEach(() => { vi.mocked(invoke).mockReset(); emailCache.clear("a"); });

describe("sent cache account boundary", () => {
  test("same message ID in two accounts has separate cache entries", async () => {
    emailCache.clear("b");
    emailCache.set("a", mail.id, mail);
    const other = { ...mail, subject: "Account B" };
    vi.mocked(invoke).mockResolvedValue(other);
    expect(await getSentEmail("b", mail.id)).toEqual(other);
    expect(invoke).toHaveBeenCalledWith("get_sent_email", { accountId: "b", emailId: mail.id });
    expect(await getSentEmail("a", mail.id)).toEqual(mail);
    expect(emailCache.get("b", mail.id)).toEqual(other);
  });

  test("sent pagination includes the captured account ID", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    await listSentEmails("a", 16, 16);
    expect(invoke).toHaveBeenCalledWith("list_sent_emails", { accountId: "a", limit: 16, offset: 16 });
  });

  test("switch invalidates a populated list", async () => {
    emailCache.setList("a", [mail], 16);
    emailCache.clear("a");
    vi.mocked(invoke).mockResolvedValue([]);
    expect(await listSentEmails("a", 16, 0)).toEqual([]);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  test("late list response cannot repopulate another account's cache", async () => {
    let resolve!: (value: SentEmail[]) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((done) => { resolve = done; }) as any);
    const pending = listSentEmails("a", 16, 0);
    emailCache.clear("a");
    emailCache.setList("a", [], 16);
    resolve([mail]);
    await pending;
    expect(emailCache.getList("a", 16)).toEqual([]);
  });

  test("late detail response cannot poison the next account's cache", async () => {
    let resolve!: (value: SentEmail) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((done) => { resolve = done; }) as any);
    const pending = getSentEmail("a", mail.id);
    emailCache.clear("a");
    resolve(mail);
    await pending;
    expect(emailCache.get("a", mail.id)).toBeUndefined();
  });
});
