/**
 * Shared pagination state for email lists (sent / inbox).
 *
 * The factory owns the loading lifecycle so list consumers only provide
 * the page fetcher. State is reactive ($state) and read through getters.
 */
export function createEmailList<T>(
  fetchPage: (limit: number, offset: number) => Promise<T[]>,
  pageSize = 16,
) {
  let items = $state<T[]>([]);
  let isLoading = $state(true);
  let isRefreshing = $state(false);
  let isLoadingMore = $state(false);
  let hasMore = $state(true);
  let currentPage = $state(0);

  async function load(append = false, silent = false) {
    if (silent && isRefreshing) return;

    try {
      if (append) {
        isLoadingMore = true;
      } else {
        if (!silent) isRefreshing = true;
        currentPage = 0;
      }

      const page = append ? currentPage + 1 : 0;
      const offset = page * pageSize;

      const newItems = await fetchPage(pageSize, offset);

      if (append) {
        items = [...items, ...newItems];
        currentPage = page;
      } else {
        items = newItems;
        currentPage = 0;
      }

      hasMore = newItems.length === pageSize;
    } catch (err) {
      // Automatic (silent) reloads never surface errors to the user.
      if (silent) {
        console.error("Silent email list refresh failed:", err);
        return;
      }
      throw err;
    } finally {
      isRefreshing = false;
      isLoadingMore = false;
      if (!append) isLoading = false;
    }
  }

  async function refresh() {
    await load(false);
  }

  async function refreshSilent() {
    await load(false, true);
  }

  async function loadMore() {
    if (!isLoadingMore && hasMore) {
      await load(true);
    }
  }

  return {
    get items() {
      return items;
    },
    get isLoading() {
      return isLoading;
    },
    get isRefreshing() {
      return isRefreshing;
    },
    get isLoadingMore() {
      return isLoadingMore;
    },
    get hasMore() {
      return hasMore;
    },
    refresh,
    refreshSilent,
    loadMore,
    /** Marks the list as initialized without fetching (initial paint). */
    markLoaded() {
      isLoading = false;
    },
  };
}
