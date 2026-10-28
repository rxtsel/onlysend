import type { EmailAttachment } from "$lib/shared/attachments";

export interface SentEmail {
  id: string;
  from: string;
  to: string[];
  subject: string;
  html: string | null;
  text?: string | null;
  attachments?: EmailAttachment[];
  createdAt: string;
  cc: string[];
  bcc: string[];
  replyTo: string | null;
  lastEvent?: string | null;
}
