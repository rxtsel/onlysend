import { invoke } from "@tauri-apps/api/core";

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

export async function getInboundEmail(
  accountId: string,
  emailId: string,
): Promise<InboundEmailDetail> {
  return await invoke<InboundEmailDetail>("open_inbound_email", { accountId, emailId });
}
