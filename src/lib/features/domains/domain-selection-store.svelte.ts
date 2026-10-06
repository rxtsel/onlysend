import {
  listDomains, getDomainPreferences, saveDomainPreferences,
  type DomainSummary,
} from "$lib/shared/api/domains";
import { errorMessage } from "$lib/shared/utils/errors";

/** Inclusion drives setup only; all accessible mail remains in account history. */
export class DomainSelectionStore {
  constructor(readonly accountId: string) {}
  domains = $state<DomainSummary[]>([]);
  includedIds = $state<string[]>([]);
  isLoading = $state(true);
  isSaving = $state(false);
  error = $state<string | null>(null);
  #generation = 0;
  #disposed = false;

  get included(): DomainSummary[] {
    return this.domains.filter((domain) => this.includedIds.includes(domain.id));
  }
  get missingIds(): string[] {
    return this.includedIds.filter((id) => !this.domains.some((domain) => domain.id === id));
  }

  async load(): Promise<void> {
    const generation = ++this.#generation;
    this.isLoading = true;
    this.error = null;
    try {
      const [domains, saved] = await Promise.all([
        listDomains(this.accountId), getDomainPreferences(this.accountId),
      ]);
      if (this.#disposed || generation !== this.#generation) return;
      this.domains = domains;
      // Legacy selected_domain remains a composer default, not an exclusion.
      this.includedIds = saved === null ? domains.map((domain) => domain.id) : [...saved.includedDomainIds];
    } catch (error) {
      if (!this.#disposed && generation === this.#generation)
        this.error = errorMessage(error, "Could not load domains or saved selection");
    } finally {
      if (!this.#disposed && generation === this.#generation) this.isLoading = false;
    }
  }

  toggle(id: string, included: boolean): void {
    if (this.isSaving) return;
    this.includedIds = included ? [...new Set([...this.includedIds, id])] : this.includedIds.filter((item) => item !== id);
  }
  selectAll(): void { if (!this.isSaving) this.includedIds = this.domains.map((domain) => domain.id); }
  deselectAll(): void { if (!this.isSaving) this.includedIds = []; }

  updateDomain(domain: DomainSummary, created = false): void {
    if (this.#disposed) return;
    this.domains = [...this.domains.filter((item) => item.id !== domain.id), domain];
    if (created) this.toggle(domain.id, true);
  }

  async save(): Promise<boolean> {
    if (this.#disposed || this.isSaving || this.isLoading || this.error) return false;
    this.isSaving = true;
    try {
      await saveDomainPreferences(this.accountId, { includedDomainIds: [...this.includedIds] });
      return !this.#disposed;
    } finally {
      this.isSaving = false;
    }
  }

  dispose(): void { this.#disposed = true; this.#generation += 1; }
}
