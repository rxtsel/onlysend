export type Mailbox = "inbox" | "sent" | "composer";

export function mailUrl(accountId: string, mailbox: Mailbox, emailId?: string): string {
  if (!accountId) throw new Error("Account ID is required");
  const base = `/mail/${encodeURIComponent(accountId)}/${mailbox}`;
  return emailId ? `${base}/${encodeURIComponent(emailId)}` : base;
}

export function mailboxFromPath(pathname: string): Mailbox {
  const parts = pathname.split("/");
  const mailbox = parts.length >= 4 ? parts[3] : parts[2];
  return mailbox === "inbox" ? "inbox" : mailbox === "composer" ? "composer" : "sent";
}
