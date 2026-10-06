import type { SentEmail } from "@/lib/types";
import type { EmailPage } from "$lib/shared/email-page";

type AccountCache = {
  emails: Map<string, SentEmail>;
  lists: Map<number, { page: EmailPage<SentEmail>; timestamp: number }>;
};

/** Separate account buckets; generations stop late logout responses repopulating them. */
class EmailCache {
  private accounts = new Map<string, AccountCache>();
  private epochs = new Map<string, number>();
  private readonly ttl = 5 * 60 * 1000;

  generation(accountId: string): number { return this.epochs.get(accountId) ?? 0; }

  private bucket(accountId: string): AccountCache {
    if (!accountId) throw new Error("Account ID is required");
    let bucket = this.accounts.get(accountId);
    if (!bucket) {
      bucket = { emails: new Map(), lists: new Map() };
      this.accounts.set(accountId, bucket);
    }
    return bucket;
  }

  get(accountId: string, id: string): SentEmail | undefined {
    return this.bucket(accountId).emails.get(id);
  }
  set(accountId: string, id: string, email: SentEmail): void {
    this.bucket(accountId).emails.set(id, email);
  }
  getList(accountId: string, limit = 12): EmailPage<SentEmail> | null {
    const list = this.bucket(accountId).lists.get(limit);
    return list && Date.now() - list.timestamp < this.ttl ? list.page : null;
  }
  setList(accountId: string, page: EmailPage<SentEmail>, limit = 12): void {
    this.bucket(accountId).lists.set(limit, { page, timestamp: Date.now() });
  }
  invalidateList(accountId: string): void {
    this.bucket(accountId).lists.clear();
    this.epochs.set(accountId, this.generation(accountId) + 1);
  }
  clear(accountId: string): void {
    this.accounts.delete(accountId);
    this.epochs.set(accountId, this.generation(accountId) + 1);
  }
}

export const emailCache = new EmailCache();
