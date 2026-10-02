import { invoke } from "@tauri-apps/api/core";

export interface InboundEmail {
  id: string;
  from: string;
  to: string[];
  subject: string;
  createdAt: string;
  /** Recipient domain, derived from the addresses. */
  domain: string;
}

export interface InboundAttachment {
  id: string;
  filename: string | null;
  contentType: string;
  size: number | null;
}

export interface InboundEmailDetail extends InboundEmail {
  /** Sanitized HTML body, rendered inside a sandboxed iframe. */
  html: string | null;
  text: string | null;
  attachments: InboundAttachment[];
}

export async function listInboundEmails(
  accountId: string,
  limit?: number,
  offset?: number,
): Promise<InboundEmail[]> {
  return await invoke<InboundEmail[]>("list_inbound_emails", { accountId, limit, offset });
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