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

  let generation = 0;

  async function load(append = false, silent = false) {
    if (silent && (isRefreshing || isLoadingMore)) return;
    const request = ++generation;

    try {
      if (append) {
        isLoadingMore = true;
      } else {
        isRefreshing = true;
        currentPage = 0;
      }

      const page = append ? currentPage + 1 : 0;
      const offset = page * pageSize;

      const newItems = await fetchPage(pageSize, offset);
      if (request !== generation) return;

      if (append) {
        items = [...items, ...newItems];
        currentPage = page;
      } else {
        items = newItems;
        currentPage = 0;
      }

      hasMore = newItems.length === pageSize;
    } catch (err) {
      if (request !== generation) return;
      // Automatic (silent) reloads never surface errors to the user.
      if (silent) {
        console.error("Silent email list refresh failed:", err);
        return;
      }
      throw err;
    } finally {
      if (request === generation) {
        isRefreshing = false;
        isLoadingMore = false;
        if (!append) isLoading = false;
      }
    }
  }

  async function refresh() {
    await load(false);
  }

  async function refreshSilent() {
    await load(false, true);
  }

  async function loadMore() {
    if (!isLoadingMore && !isRefreshing && hasMore) {
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
    clearItems() {
      generation += 1;
      isLoading = true;
      isRefreshing = false;
      isLoadingMore = false;
      items = [];
      hasMore = true;
      currentPage = 0;
    },
  };
}
