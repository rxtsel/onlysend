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
  limit?: number,
  offset?: number,
): Promise<InboundEmail[]> {
  return await invoke<InboundEmail[]>("list_inbound_emails", { limit, offset });
}

export async function getInboundEmail(
  emailId: string,
): Promise<InboundEmailDetail> {
  return await invoke<InboundEmailDetail>("get_inbound_email", { emailId });
}

export async function getReadInboundIds(): Promise<string[]> {
  return await invoke<string[]>("get_read_inbound_ids");
}

export async function markInboundRead(emailId: string): Promise<void> {
  await invoke("mark_inbound_read", { emailId });
}