import { describe, expect, test, vi } from "vitest";
import { createEmailList } from "./email-list.svelte.js";
import type { EmailPage } from "./email-page";

type Item = { id: string };
function page(ids: string[], nextCursor: string | null = null): EmailPage<Item> {
  return { items: ids.map((id) => ({ id })), hasMore: nextCursor !== null, nextCursor };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

describe("createEmailList cursor pagination", () => {
  test("reset discards old account items and cursor without clearing new loading state", async () => {
    const a = deferred<EmailPage<Item>>();
    const b = deferred<EmailPage<Item>>();
    const fetcher = vi.fn().mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise).mockResolvedValue(page(["b2"]));
    const list = createEmailList<Item>(fetcher);
    const old = list.refresh();
    list.clearItems();
    const current = list.refresh();
    a.resolve(page(["a1"], "a-cursor"));
    await old;
    expect(list.items).toEqual([]);
    expect(list.isRefreshing).toBe(true);
    expect(list.hasMore).toBe(false);
    b.resolve(page(["b1"], "b-cursor"));
    await current;
    await list.loadMore();
    expect(fetcher).toHaveBeenLastCalledWith(16, "b-cursor");
    expect(list.items).toEqual([{ id: "b1" }, { id: "b2" }]);
  });

  test("silent refreshes are single-flight even for an empty inbox", async () => {
    const result = deferred<EmailPage<Item>>();
    const fetcher = vi.fn(() => result.promise);
    const list = createEmailList(fetcher);
    const first = list.refreshSilent();
    await list.refreshSilent();
    expect(fetcher).toHaveBeenCalledTimes(1);
    result.resolve(page([]));
    await first;
    expect(list.items).toEqual([]);
    expect(list.isLoading).toBe(false);
    expect(list.hasMore).toBe(false);
  });

  test("stale rejection does not surface an error in another account", async () => {
    const result = deferred<EmailPage<Item>>();
    const list = createEmailList(() => result.promise);
    const pending = list.refresh();
    list.clearItems();
    result.reject(new Error("old account unauthorized"));
    await expect(pending).resolves.toBeUndefined();
    expect(list.error).toBeNull();
  });

  test("permissions error is not empty history and retry clears it", async () => {
    const fetcher = vi.fn().mockRejectedValueOnce(new Error("Insufficient permissions")).mockResolvedValue(page([]));
    const list = createEmailList<Item>(fetcher);
    await expect(list.refresh()).rejects.toThrow("Insufficient permissions");
    expect(list.error).toContain("Insufficient permissions");
    expect(list.isLoading).toBe(false);
    await list.refresh();
    expect(list.items).toEqual([]);
    expect(list.error).toBeNull();
  });

  test("load more starts only after a successful first page", async () => {
    const fetcher = vi.fn().mockResolvedValue(page([]));
    const list = createEmailList<Item>(fetcher);
    await list.loadMore();
    expect(fetcher).not.toHaveBeenCalled();
  });

  test("short pages can continue and full pages can end according to the API", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["1"], "1"))
      .mockResolvedValueOnce(page(["2", "3"]));
    const list = createEmailList<Item>(fetcher, 2);
    await list.refresh();
    expect(list.hasMore).toBe(true);
    expect(fetcher).toHaveBeenNthCalledWith(1, 2, null);
    await list.loadMore();
    expect(fetcher).toHaveBeenNthCalledWith(2, 2, "1");
    expect(list.items.map((item) => item.id)).toEqual(["1", "2", "3"]);
    expect(list.hasMore).toBe(false);
    await list.loadMore();
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  test("cursor traversal loads more than 100 and 255 messages with fixed request size", async () => {
    const all = Array.from({ length: 321 }, (_, index) => ({ id: `message-${index}` }));
    const fetcher = vi.fn(async (limit: number, after: string | null): Promise<EmailPage<Item>> => {
      const start = after === null ? 0 : all.findIndex((item) => item.id === after) + 1;
      const items = all.slice(start, start + limit);
      const hasMore = start + items.length < all.length;
      return { items, hasMore, nextCursor: hasMore ? items.at(-1)!.id : null };
    });
    const list = createEmailList(fetcher);
    await list.refresh();
    while (list.hasMore) await list.loadMore();
    expect(list.items).toEqual(all);
    expect(fetcher).toHaveBeenCalledTimes(21);
    expect(fetcher.mock.calls.every(([limit]) => limit === 16)).toBe(true);
    expect(new Set(list.items.map((item) => item.id)).size).toBe(321);
  });

  test("new arrivals between pages do not shift the continuation or duplicate messages", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["a", "b"], "b"))
      .mockResolvedValueOnce(page(["b", "c", "d"], "d"));
    const list = createEmailList<Item>(fetcher, 2);
    await list.refresh();
    await list.loadMore();
    expect(fetcher).toHaveBeenLastCalledWith(2, "b");
    expect(list.items.map((item) => item.id)).toEqual(["a", "b", "c", "d"]);
  });

  test("failed load more retries the same cursor without losing history", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["1"], "cursor-one"))
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce(page(["2"]));
    const list = createEmailList<Item>(fetcher);
    await list.refresh();
    await expect(list.loadMore()).rejects.toThrow("offline");
    expect(list.items).toEqual([{ id: "1" }]);
    expect(list.hasMore).toBe(true);
    expect(list.isLoadingMore).toBe(false);
    await list.loadMore();
    expect(fetcher).toHaveBeenNthCalledWith(2, 16, "cursor-one");
    expect(fetcher).toHaveBeenNthCalledWith(3, 16, "cursor-one");
    expect(list.items).toEqual([{ id: "1" }, { id: "2" }]);
    expect(list.error).toBeNull();
  });

  test("concurrent load more and polling cannot race cursors", async () => {
    const more = deferred<EmailPage<Item>>();
    const fetcher = vi.fn().mockResolvedValueOnce(page(["1"], "cursor-one")).mockReturnValueOnce(more.promise);
    const list = createEmailList<Item>(fetcher);
    await list.refresh();
    const pending = list.loadMore();
    await list.loadMore();
    await list.refreshSilent();
    expect(fetcher).toHaveBeenCalledTimes(2);
    more.resolve(page(["2"]));
    await pending;
    expect(list.items).toEqual([{ id: "1" }, { id: "2" }]);
  });

  test("explicit refresh resets pagination and supersedes a slow load-more response", async () => {
    const more = deferred<EmailPage<Item>>();
    const fetcher = vi.fn().mockResolvedValueOnce(page(["1"], "old"))
      .mockReturnValueOnce(more.promise).mockResolvedValueOnce(page(["new"], "new-cursor"))
      .mockResolvedValue(page(["end"]));
    const list = createEmailList<Item>(fetcher);
    await list.refresh();
    const pending = list.loadMore();
    await list.refresh();
    more.resolve(page(["obsolete"], "obsolete-cursor"));
    await pending;
    expect(list.items).toEqual([{ id: "new" }]);
    await list.loadMore();
    expect(fetcher).toHaveBeenLastCalledWith(16, "new-cursor");
  });

  test("silent refresh preserves older pages and their tail cursor", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["a", "b"], "b"))
      .mockResolvedValueOnce(page(["c", "d"], "d"))
      .mockResolvedValueOnce(page(["new", "a"], "a"))
      .mockResolvedValueOnce(page(["e"]));
    const list = createEmailList<Item>(fetcher, 2);
    await list.refresh();
    await list.loadMore();
    await list.refreshSilent();
    expect(list.items.map((item) => item.id)).toEqual(["new", "a", "b", "c", "d"]);
    await list.loadMore();
    expect(fetcher).toHaveBeenLastCalledWith(2, "d");
    expect(list.items.at(-1)?.id).toBe("e");
  });

  test("silent refresh without overlap resets to the head to avoid skipping unseen emails", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["old"], "old"))
      .mockResolvedValueOnce(page(["newest", "newer"], "newer"))
      .mockResolvedValueOnce(page(["gap", "old"]));
    const list = createEmailList<Item>(fetcher, 2);
    await list.refresh();
    await list.refreshSilent();
    expect(list.items.map((item) => item.id)).toEqual(["newest", "newer"]);
    await list.loadMore();
    expect(fetcher).toHaveBeenLastCalledWith(2, "newer");
    expect(list.items.map((item) => item.id)).toEqual(["newest", "newer", "gap", "old"]);
  });

  test("failed silent refresh preserves items and their continuation for retry", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["1"], "one"))
      .mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce(page(["2"]));
    const list = createEmailList<Item>(fetcher);
    await list.refresh();
    await expect(list.refreshSilent()).resolves.toBeUndefined();
    expect(list.error).toContain("offline");
    expect(list.items).toEqual([{ id: "1" }]);
    await list.loadMore();
    expect(fetcher).toHaveBeenLastCalledWith(16, "one");
  });

  test.each([
    { items: [], hasMore: true, nextCursor: "two" },
    { items: [{ id: "2" }], hasMore: true, nextCursor: null },
    { items: [{ id: "2" }], hasMore: true, nextCursor: "one" },
  ])("malformed continuation reports an error rather than silently truncating history: %j", async (invalid) => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["1"], "one")).mockResolvedValueOnce(invalid)
      .mockResolvedValueOnce(page(["2"]));
    const list = createEmailList<Item>(fetcher);
    await list.refresh();
    await expect(list.loadMore()).rejects.toThrow("did not advance");
    expect(list.items).toEqual([{ id: "1" }]);
    expect(list.hasMore).toBe(true);
    await list.loadMore();
    expect(fetcher).toHaveBeenLastCalledWith(16, "one");
  });

  test("cursor cycles are rejected", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(page(["1"], "one"))
      .mockResolvedValueOnce(page(["2"], "two")).mockResolvedValueOnce(page(["3"], "one"));
    const list = createEmailList<Item>(fetcher);
    await list.refresh();
    await list.loadMore();
    await expect(list.loadMore()).rejects.toThrow("did not advance");
    expect(list.items.map((item) => item.id)).toEqual(["1", "2"]);
  });
});

describe("downloaded snapshots", () => {
  test("shows disk data before remote completion and then replaces it", async () => {
    const remote = deferred<EmailPage<Item>>();
    const disk = { ...page(["disk"], "disk-cursor"), cache: { downloadedAt: 123, error: null } };
    const list = createEmailList<Item>(() => remote.promise, 16, async () => disk);
    const pending = list.refresh();
    await vi.waitFor(() => expect(list.items).toEqual([{ id: "disk" }]));
    expect(list.isLoading).toBe(false);
    expect(list.isRefreshing).toBe(true);
    expect(list.cacheInfo?.downloadedAt).toBe(123);
    remote.resolve(page(["fresh"]));
    await pending;
    expect(list.items).toEqual([{ id: "fresh" }]);
    expect(list.cacheInfo).toBeNull();
  });

  test("fallback keeps its remote error visible instead of claiming a successful sync", async () => {
    const cached = { ...page(["disk"], "older"), cache: { downloadedAt: 123, error: "Missing permissions" } };
    const list = createEmailList<Item>(async () => cached);
    await list.refresh();
    expect(list.error).toBe("Missing permissions");
    expect(list.cacheInfo?.downloadedAt).toBe(123);
    expect(list.hasMore).toBe(true);
  });

  test("clearing an account discards a late disk response before any remote request", async () => {
    const disk = deferred<EmailPage<Item> | null>();
    const fetcher = vi.fn(async () => page(["remote"]));
    const list = createEmailList<Item>(fetcher, 16, () => disk.promise);
    const pending = list.refresh();
    list.clearItems();
    disk.resolve({ ...page(["old-account"]), cache: { downloadedAt: 123, error: null } });
    await pending;
    expect(list.items).toEqual([]);
    expect(list.cacheInfo).toBeNull();
    expect(fetcher).not.toHaveBeenCalled();
  });

  test("background disk refresh exposes newly downloaded pages without a remote request", async () => {
    const disk = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce(page(["head"], "older"));
    const remote = vi.fn(async () => page(["head"]));
    const list = createEmailList<Item>(remote, 16, disk);
    await list.refresh();
    expect(list.hasMore).toBe(false);
    await list.refreshCached();
    expect(remote).toHaveBeenCalledTimes(1);
    expect(list.hasMore).toBe(true);
    expect(list.items).toEqual([{ id: "head" }]);
  });

  test("disk failure does not prevent a remote attempt", async () => {
    const list = createEmailList<Item>(async () => page(["remote"]), 16, async () => { throw new Error("disk failed"); });
    await list.refresh();
    expect(list.items).toEqual([{ id: "remote" }]);
  });
});
