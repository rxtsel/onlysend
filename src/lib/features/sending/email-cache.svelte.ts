import type { SentEmail } from "@/lib/types";

class EmailCache {
  private cache = $state<Map<string, SentEmail>>(new Map());
  private listCache = $state<SentEmail[] | null>(null);
  private listCacheTimestamp = $state<number>(0);

  // Cache TTL: 5 minutes for list, indefinite for individual emails
  private readonly LIST_CACHE_TTL = 5 * 60 * 1000;

  // Get a single email from cache
  get(id: string): SentEmail | undefined {
    return this.cache.get(id);
  }

  // Set a single email in cache
  set(id: string, email: SentEmail): void {
    this.cache.set(id, email);
  }

  // Get list from cache if still valid
  getList(): SentEmail[] | null {
    const now = Date.now();
    if (this.listCache && (now - this.listCacheTimestamp) < this.LIST_CACHE_TTL) {
      return this.listCache;
    }
    return null;
  }

  // Set list in cache
  setList(emails: SentEmail[]): void {
    this.listCache = emails;
    this.listCacheTimestamp = Date.now();
  }

  // Invalidate list cache (e.g., after sending a new email)
  invalidateList(): void {
    this.listCache = null;
    this.listCacheTimestamp = 0;
  }

  // Clear all cache
  clear(): void {
    this.cache.clear();
    this.listCache = null;
    this.listCacheTimestamp = 0;
  }

  // Check if an email exists in cache
  has(id: string): boolean {
    return this.cache.has(id);
  }
}

// Export singleton instance
export const emailCache = new EmailCache();
