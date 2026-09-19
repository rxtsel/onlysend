import { beforeEach, describe, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { emailCache } from "@/lib/features/sending/email-cache.svelte";
import { getSentEmail, listSentEmails } from "./sent";
import type { SentEmail } from "../types/sent.type";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mail = { id: "same-id", subject: "Account A" } as SentEmail;

beforeEach(() => { vi.mocked(invoke).mockReset(); emailCache.clear(); });

describe("sent cache account boundary", () => {
  test("switch invalidates a populated list", async () => {
    emailCache.setList([mail]);
    emailCache.clear();
    vi.mocked(invoke).mockResolvedValue([]);
    expect(await listSentEmails(16, 0)).toEqual([]);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  test("late list response cannot repopulate another account's cache", async () => {
    let resolve!: (value: SentEmail[]) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((done) => { resolve = done; }) as any);
    const pending = listSentEmails(16, 0);
    emailCache.clear();
    emailCache.setList([]);
    resolve([mail]);
    await pending;
    expect(emailCache.getList()).toEqual([]);
  });

  test("late detail response cannot poison the next account's cache", async () => {
    let resolve!: (value: SentEmail) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((done) => { resolve = done; }) as any);
    const pending = getSentEmail(mail.id);
    emailCache.clear();
    resolve(mail);
    await pending;
    expect(emailCache.get(mail.id)).toBeUndefined();
  });
});
