import { invoke } from "@tauri-apps/api/core";
import type { EmailPage } from "./email-page";

export interface InboundEmail {
  id: string;
  from: string;
  to: string[];
  subject: string;
  createdAt: string;
  /** First envelope recipient domain, kept for compatibility. */
  domain: string;
  /** All envelope recipient domains; never derived from the sender. */
  domains: string[];
}

export interface InboundAttachment {
  id: string;
  filename: string | null;
  contentType: string;
  size: number | null;
}

export interface InboundEmailDetail extends Omit<InboundEmail, "domain" | "domains"> {
  /** Sanitized HTML body, rendered inside a sandboxed iframe. */
  html: string | null;
  text: string | null;
  attachments: InboundAttachment[];
}

export async function listInboundEmails(
  accountId: string,
  limit?: number,
  after: string | null = null,
): Promise<EmailPage<InboundEmail>> {
  return await invoke<EmailPage<InboundEmail>>("list_inbound_emails", { accountId, limit, after });
}

export async function getInboundEmail(
  accountId: string,
  emailId: string,
): Promise<InboundEmailDetail> {
  return await invoke<InboundEmailDetail>("get_inbound_email", { accountId, emailId });
}

export async function getReadInboundIds(accountId: string): Promise<string[]> {
  return await invoke<string[]>("get_read_inbound_ids", { accountId });
}

export async function markInboundRead(accountId: string, emailId: string): Promise<void> {
  await invoke("mark_inbound_read", { accountId, emailId });
}