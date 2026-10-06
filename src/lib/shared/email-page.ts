/** One Resend cursor page. Continuation is explicit, not inferred from length. */
export interface EmailPage<T> {
  items: T[];
  hasMore: boolean;
  nextCursor: string | null;
  /** Present only for a downloaded snapshot, never proof of complete history. */
  cache?: { downloadedAt: number; error: string | null };
}
