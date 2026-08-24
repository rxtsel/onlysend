import { describe, expect, test, vi } from "vitest";
import { createEmailList } from "./email-list.svelte.js";

type Item = { id: number };

function makeFetcher(pages: Record<number, Item[]>) {
  return vi.fn((limit: number, offset: number): Promise<Item[]> => {
    const page = offset / limit;
    return Promise.resolve(pages[page] ?? []);
  });
}

describe("createEmailList", () => {
  test("refresh loads first page and clears previous items", async () => {
    const fetcher = makeFetcher({ 0: [{ id: 1 }, { id: 2 }] });
    const list = createEmailList(fetcher, 16);

    await list.refresh();

    expect(list.items).toEqual([{ id: 1 }, { id: 2 }]);
    expect(list.isLoading).toBe(false);
    expect(list.hasMore).toBe(false);
    expect(fetcher).toHaveBeenCalledWith(16, 0);
  });

  test("loadMore appends the next page", async () => {
    const fetcher = makeFetcher({
      0: [{ id: 1 }, { id: 2 }],
      1: [{ id: 3 }],
    });
    const list = createEmailList(fetcher, 2);

    await list.refresh();
    expect(list.items.map((i) => i.id)).toEqual([1, 2]);
    expect(list.hasMore).toBe(true);

    await list.loadMore();
    expect(list.items.map((i) => i.id)).toEqual([1, 2, 3]);
    // Partial page means no more data
    expect(list.hasMore).toBe(false);
  });

  test("full page keeps hasMore true", async () => {
    const pages: Record<number, Item[]> = {
      0: Array.from({ length: 16 }, (_, i) => ({ id: i })),
      1: Array.from({ length: 16 }, (_, i) => ({ id: i + 100 })),
    };
    const list = createEmailList(makeFetcher(pages), 16);

    await list.refresh();
    expect(list.hasMore).toBe(true);

    await list.loadMore();
    expect(list.items).toHaveLength(32);
  });

  test("silent errors are swallowed and state is preserved", async () => {
    let shouldFail = false;
    const fetcher = vi.fn(() =>
      shouldFail
        ? Promise.reject(new Error("boom"))
        : Promise.resolve([{ id: 1 }]),
    );
    const list = createEmailList<Item>(fetcher as any);

    await list.refreshSilent().catch(() => {
      throw new Error("silent refresh must not throw");
    });

    // Failed initial load still flips isLoading off so UI isn't stuck
    expect(list.isLoading).toBe(false);

    shouldFail = false;
    await list.refresh();
    expect(list.items.length).toBe(1);
  });

  test("non-silent errors propagate to the caller", async () => {
    const fetcher = vi.fn(() => Promise.reject(new Error("boom")));
    const list = createEmailList<Item>(fetcher as any);

    await expect(list.refresh()).rejects.toThrow("boom");
  });
});
