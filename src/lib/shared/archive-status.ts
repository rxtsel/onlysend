import type { MailArchiveStatus, Mailbox } from "./local-mail";

export interface ArchiveFooterData {
  status: MailArchiveStatus | null;
  statusError: string | null;
  refreshError: string | null;
  downloadedAt: number | null;
}
export interface ArchiveFooterSnapshot {
  accountId: string;
  mailbox: Mailbox;
  data: ArchiveFooterData;
}
export const EMPTY_ARCHIVE_FOOTER: ArchiveFooterData = {
  status: null, statusError: null, refreshError: null, downloadedAt: null,
};

export function selectArchiveFooter(snapshot: ArchiveFooterSnapshot | null, accountId: string, mailbox: Mailbox): ArchiveFooterData {
  return snapshot?.accountId === accountId && snapshot.mailbox === mailbox ? snapshot.data : EMPTY_ARCHIVE_FOOTER;
}

export function archiveStatusPresentation(data: ArchiveFooterData) {
  const status = data.status;
  if (data.statusError) return { kind: "error", label: "Local copy unavailable" } as const;
  if (data.refreshError) return { kind: "error", label: "Remote refresh failed" } as const;
  if (status?.running) return { kind: "syncing", label: "Downloading local copy" } as const;
  if (status?.sync?.lastError) return { kind: "error", label: "Local download interrupted" } as const;
  if (!status) return { kind: "checking", label: "Checking local copy" } as const;
  if (status.sync?.completedAt != null && status.downloadedBodies >= status.downloadedMessages) {
    return { kind: "ready", label: "Local copy up to date" } as const;
  }
  return { kind: "partial", label: "Local copy incomplete" } as const;
}
