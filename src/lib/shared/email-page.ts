/** One Resend cursor page. Continuation is explicit, not inferred from length. */
export interface EmailPage<T> {
  items: T[];
  hasMore: boolean;
  nextCursor: string | null;
}
