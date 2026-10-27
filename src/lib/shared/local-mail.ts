import { invoke } from "@tauri-apps/api/core";
import type { EmailPage } from "./email-page";

export type Mailbox = "inbox" | "sent";
export interface MailSyncStatus { startedAt: number; metadataCompletedAt: number | null; completedAt: number | null; lastError: string | null }
export interface MailListItem {
  id: string;
  from: string;
  to: string[];
  subject: string;
  createdAt: string;
  domains?: string[];
  domain?: string;
  preview: string;
  isRead: boolean;
}

export interface MailListRequest {
  mailbox: Mailbox;
  limit: number;
  after: string | null;
  domain: string | null;
  search: string | null;
  refresh: boolean;
}

export function loadMailList(accountId: string, request: MailListRequest): Promise<EmailPage<MailListItem> & { domains: string[] }> {
  return invoke("load_mail_list", { accountId, request });
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

/** Rust refreshes the head and retries the background archive download. */
export function refreshMailbox(accountId: string, mailbox: Mailbox): Promise<void> {
  return invoke("refresh_mailbox", { accountId, mailbox });
}
