export interface SentEmail {
  id: string;
  from: string;
  to: string[];
  subject: string;
  html: string | null;
  createdAt: string;
  cc: string[];
  bcc: string[];
  replyTo: string | null;
}
