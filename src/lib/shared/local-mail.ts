import { invoke } from "@tauri-apps/api/core";
import type { EmailPage } from "./email-page";
import { errorMessage } from "./utils/errors";

export type Mailbox = "inbox" | "sent";
export type MailSort = "newest" | "oldest" | "subjectAsc" | "subjectDesc" | "senderAsc" | "senderDesc";
export interface LocalCursor { value: string; emailId: string; queryKey: string }
export interface MailSyncStatus { startedAt: number; metadataCompletedAt: number | null; completedAt: number | null; lastError: string | null }
export interface LocalMailQuery {
  mailbox: Mailbox;
  limit?: number;
  sort?: MailSort;
  domain?: string;
  recipient?: string;
  search?: string;
  unreadOnly?: boolean;
  after?: LocalCursor | null;
}
export interface LocalMailPage<T> {
  items: T[];
  hasMore: boolean;
  nextCursor: LocalCursor | null;
  sync: MailSyncStatus | null;
  isPartial: boolean;
  downloadedAt: number | null;
}

/** Exact downloaded remote page; its cursor is NOT a local query keyset. */
export function getCachedMailPage<T>(accountId: string, mailbox: Mailbox, limit = 16, after: string | null = null): Promise<EmailPage<T> | null> {
  return invoke("get_cached_mail_page", { accountId, mailbox, limit, after });
}

function asListPage<T>(local: LocalMailPage<T>, cache?: EmailPage<T>["cache"]): EmailPage<T> {
  return { items: local.items, hasMore: local.hasMore, nextCursor: local.nextCursor ? JSON.stringify(local.nextCursor) : null, ...(cache ? { cache } : {}) };
}

/** Disk-only list hydration. Empty initialized databases are not empty history. */
export async function getLocalArchivePage<T>(accountId: string, mailbox: Mailbox, limit: number): Promise<EmailPage<T> | null> {
  const local = await queryLocalMail<T>(accountId, { mailbox, limit });
  if (local.downloadedAt === null) return null;
  return asListPage(local, { downloadedAt: local.downloadedAt, error: null });
}

/** Visible list is the retained local archive, never a remote-page slice. */
export async function loadArchivePage<T>(
  accountId: string, mailbox: Mailbox, limit: number, after: string | null,
  refreshRemote: (limit: number) => Promise<EmailPage<T>>,
): Promise<EmailPage<T>> {
  let cache: EmailPage<T>["cache"];
  let remoteError: string | null = null;
  if (after === null) {
    try { cache = (await refreshRemote(limit)).cache; }
    catch (error) { remoteError = errorMessage(error, "Remote refresh failed"); }
  }
  const local = await queryLocalMail<T>(accountId, { mailbox, limit, after: after ? JSON.parse(after) : null });
  if (remoteError) {
    if (local.downloadedAt === null) throw new Error(remoteError);
    cache = { downloadedAt: local.downloadedAt, error: remoteError };
  }
  return asListPage(local, cache);
}

/** Search/sort/filter the entire downloaded archive, not only visible pages. */
export function queryLocalMail<T>(accountId: string, query: LocalMailQuery): Promise<LocalMailPage<T>> {
  return invoke("query_local_mail", { accountId, query });
}

export interface MailArchiveStatus {
  downloadedMessages: number;
  downloadedBodies: number;
  running: boolean;
  sync: MailSyncStatus | null;
}
export function getMailArchiveStatus(accountId: string, mailbox: Mailbox): Promise<MailArchiveStatus> {
  return invoke("get_mail_archive_status", { accountId, mailbox });
}

/** Sequential background metadata/body download. Never deletes remote mail. */
export function syncMailbox(accountId: string, mailbox: Mailbox, force = false, limit = 16): Promise<boolean> {
  return invoke("sync_mailbox", { accountId, mailbox, force, limit });
}
