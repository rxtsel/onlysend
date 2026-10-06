import { errorMessage } from "./utils/errors";
import type { EmailPage } from "./email-page";

/** Account/mailbox-scoped cursor state; stale responses never advance it. */
export function createEmailList<T extends { id: string }>(
  fetchPage: (limit: number, after: string | null) => Promise<EmailPage<T>>,
  pageSize = 16,
  fetchCached?: (limit: number, after: string | null) => Promise<EmailPage<T> | null>,
) {
  let items = $state<T[]>([]);
  let isLoading = $state(true);
  let isRefreshing = $state(false);
  let isLoadingMore = $state(false);
  let hasMore = $state(false);
  let nextCursor: string | null = null;
  let error = $state<string | null>(null);
  let cacheInfo = $state<EmailPage<T>["cache"] | null>(null);
  let generation = 0;
  let seenCursors = new Set<string>();

  function unique(values: T[]): T[] {
    const seen = new Set<string>();
    return values.filter((item) => {
      if (seen.has(item.id)) return false;
      seen.add(item.id);
      return true;
    });
  }

  async function load(append = false, silent = false, diskOnly = false) {
    if (silent && (isRefreshing || isLoadingMore)) return;
    const request = ++generation;
    const after = append ? nextCursor : null;
    try {
      if (append) isLoadingMore = true;
      else {
        isRefreshing = true;
        // Explicit refresh supersedes an in-flight load-more request.
        isLoadingMore = false;
      }
      // Show the correct account's disk snapshot immediately, then revalidate.
      if (!append && !silent && !diskOnly && !items.length && fetchCached) {
        const cached = await fetchCached(pageSize, null).catch(() => null);
        if (request !== generation) return;
        if (cached && (!cached.hasMore || (cached.items.length && cached.nextCursor))) {
          items = unique(cached.items);
          hasMore = cached.hasMore;
          nextCursor = cached.nextCursor;
          cacheInfo = cached.cache ?? null;
          isLoading = false;
        }
      }
      const page = diskOnly && fetchCached ? await fetchCached(pageSize, after) : await fetchPage(pageSize, after);
      if (request !== generation || !page) return;
      if (page.hasMore && (!page.items.length || !page.nextCursor ||
          page.nextCursor === after || (append && seenCursors.has(page.nextCursor)))) {
        throw new Error("Email pagination did not advance. Retry or refresh the list.");
      }

      // Polling updates the head without discarding older loaded pages. If a
      // burst of new messages removes all overlap, restart from the new head
      // cursor instead of skipping the gap between new and old history.
      const oldIds = new Set(items.map((item) => item.id));
      const preserveHistory = silent && page.hasMore && page.items.some((item) => oldIds.has(item.id));
      if (append) {
        items = unique([...items, ...page.items]);
      } else if (preserveHistory) {
        items = unique([...page.items, ...items]);
      } else {
        items = unique(page.items);
        seenCursors = new Set();
      }
      if (!preserveHistory) {
        hasMore = page.hasMore;
        nextCursor = page.hasMore ? page.nextCursor : null;
        if (nextCursor) seenCursors.add(nextCursor);
      }
      cacheInfo = page.cache ?? (append ? cacheInfo : null);
      error = cacheInfo?.error ?? null;
    } catch (err) {
      if (request !== generation) return;
      error = errorMessage(err, "Failed to load emails");
      if (silent) return;
      throw err;
    } finally {
      if (request === generation) {
        isRefreshing = false;
        isLoadingMore = false;
        if (!append) isLoading = false;
      }
    }
  }

  return {
    get items() { return items; },
    get error() { return error; },
    get cacheInfo() { return cacheInfo; },
    get isLoading() { return isLoading; },
    get isRefreshing() { return isRefreshing; },
    get isLoadingMore() { return isLoadingMore; },
    get hasMore() { return hasMore; },
    async refresh() { await load(); },
    async refreshSilent() { await load(false, true); },
    async refreshCached() {
      if (fetchCached && !isRefreshing && !isLoadingMore) await load(false, false, true);
    },
    async loadMore() {
      if (!isLoadingMore && !isRefreshing && hasMore && nextCursor) await load(true);
    },
    markLoaded() { isLoading = false; },
    clearItems() {
      generation += 1;
      isLoading = true;
      isRefreshing = false;
      isLoadingMore = false;
      items = [];
      error = null;
      cacheInfo = null;
      hasMore = false;
      nextCursor = null;
      seenCursors = new Set();
    },
  };
}
