import {
  createDomain, deleteDomain, getDomain, listDomains, setDomainReceiving,
  verifyDomain, type DomainDetail, type DomainSummary,
} from "$lib/shared/api/domains";

import { queueDomainRequest } from "./domain-requests";
import { errorMessage } from "$lib/shared/utils/errors";

const POLL_INTERVAL_MS = 5000;
const POLL_MAX_ATTEMPTS = 60;

export function canSelect(domain: DomainSummary): boolean {
  return domain.status === "verified" || domain.capabilities?.sending === "enabled";
}
export function allGreen(domain: DomainDetail): boolean {
  return domain.records.length > 0 && domain.records.every((record) => record.status === "verified");
}
/** Delivery may still be pending even when the aggregate sending status is verified. */
export function dnsVerificationPending(domain: DomainDetail): boolean {
  return domain.status !== "verified" || domain.records.some((record) =>
    record.status !== "verified" && !(
      (record.group === "Receiving" || record.group === "Receiving MX") &&
      domain.capabilities.receiving !== "enabled"
    ));
}
export function needsDns(domain: DomainDetail): boolean {
  return domain.status !== "verified" || !allGreen(domain) || domain.capabilities.receiving !== "enabled";
}
export function sendingReady(domain: DomainSummary): boolean {
  if (domain.capabilities?.sending !== "enabled") return false;
  if ("records" in domain) {
    const records = (domain as DomainDetail).records.filter((record) =>
      record.group !== "Receiving" && record.group !== "Receiving MX");
    return records.length > 0 && records.every((record) => record.status === "verified");
  }
  return domain.status === "verified";
}
export type { DomainSummary };

/** Each expanded panel owns an instance: no shared current-domain singleton. */
export class DomainSetupStore {
  constructor(readonly accountId: string) {}
  domains = $state<DomainSummary[]>([]);
  isLoading = $state(true);
  selectedName = $state("");
  selectedId = $state("");
  setupDetail = $state<DomainDetail | null>(null);
  verified = $state(false);
  isCreating = $state(false);
  isVerifying = $state(false);
  isDeleting = $state(false);
  isTogglingReceiving = $state(false);
  isPolling = $state(false);
  pollingTimedOut = $state(false);
  pollingError = $state<string | null>(null);
  #revision = 0;
  #listRequest = 0;
  #disposed = false;
  #pollTimer: ReturnType<typeof setTimeout> | undefined;
  #pollAttempts = 0;

  get activeList() { return this.domains; }
  #current(revision: number, id?: string): boolean {
    return !this.#disposed && revision === this.#revision && (!id || this.setupDetail?.id === id);
  }

  async load(): Promise<void> {
    const request = ++this.#listRequest;
    this.isLoading = true;
    try {
      const domains = await listDomains(this.accountId);
      if (!this.#disposed && request === this.#listRequest) this.domains = domains;
    } finally {
      if (!this.#disposed && request === this.#listRequest) this.isLoading = false;
    }
  }
  select(name: string, id: string): void { this.selectedName = name; this.selectedId = id; }
  resetSetup(): void {
    this.stopWork();
    this.setupDetail = null;
    this.verified = false;
  }

  async create(options: { name: string; region: string; enableReceiving: boolean }): Promise<void> {
    if (this.isCreating || this.#disposed) return;
    this.stopWork();
    const revision = this.#revision;
    this.isCreating = true;
    try {
      const created = await createDomain(this.accountId, options);
      if (!this.#current(revision)) return;
      this.applyDetail(created);
      this.select(created.name, created.id);
    } finally { this.isCreating = false; }
  }

  /** Reading a panel never triggers remote verification or enables receiving. */
  async loadFresh(id: string): Promise<{ detail: DomainDetail; needsDns: boolean }> {
    this.stopWork();
    const revision = this.#revision;
    const detail = await queueDomainRequest(() => getDomain(this.accountId, id));
    if (this.#current(revision) && detail.id === id) {
      this.applyDetail(detail);
      if (dnsVerificationPending(detail)) this.#startPolling(revision, id);
    }
    return { detail, needsDns: needsDns(detail) };
  }
  async remove(id: string): Promise<boolean> {
    this.isDeleting = true;
    try { return await deleteDomain(this.accountId, id); }
    finally { this.isDeleting = false; }
  }
  async verify(): Promise<void> {
    if (!this.setupDetail || this.isVerifying || this.isTogglingReceiving || this.#disposed) return;
    this.stopWork();
    const revision = this.#revision;
    const id = this.setupDetail.id;
    this.isVerifying = true;
    try {
      const detail = await queueDomainRequest(() => this.#current(revision, id)
        ? verifyDomain(this.accountId, id) : Promise.resolve(undefined));
      if (!detail || !this.#current(revision, id) || detail.id !== id) return;
      this.applyDetail(detail);
      if (dnsVerificationPending(detail)) this.#startPolling(revision, id);
    } finally { this.isVerifying = false; }
  }
  async toggleReceiving(enable: boolean): Promise<void> {
    if (!this.setupDetail || this.isTogglingReceiving || this.isVerifying || this.#disposed) return;
    this.stopWork();
    const revision = this.#revision;
    const id = this.setupDetail.id;
    this.isTogglingReceiving = true;
    try {
      const detail = await queueDomainRequest(() => this.#current(revision, id)
        ? setDomainReceiving(this.accountId, id, enable) : Promise.resolve(undefined));
      if (detail && this.#current(revision, id) && detail.id === id) {
        this.applyDetail(detail);
        if (dnsVerificationPending(detail)) this.#startPolling(revision, id);
      }
    } finally { this.isTogglingReceiving = false; }
  }
  stopWork(): void {
    this.#revision += 1;
    this.#stopPolling();
    this.pollingTimedOut = false;
    this.pollingError = null;
  }
  dispose(): void { this.#disposed = true; this.stopWork(); this.#listRequest += 1; }
  markLoaded(): void { this.isLoading = false; }
  applyDetail(detail: DomainDetail | null | undefined): void {
    if (!detail || this.#disposed) return;
    this.setupDetail = detail;
    this.verified = allGreen(detail);
    this.pollingError = null;
    this.pollingTimedOut = false;
    if (!dnsVerificationPending(detail)) this.#stopPolling();
  }
  #startPolling(revision: number, id: string): void {
    this.#stopPolling();
    this.#pollAttempts = 0;
    this.isPolling = true;
    const poll = async () => {
      if (!this.#current(revision, id)) return;
      this.#pollTimer = undefined;
      this.#pollAttempts += 1;
      try {
        const fresh = await queueDomainRequest(() => this.#current(revision, id)
          ? getDomain(this.accountId, id) : Promise.resolve(undefined));
        if (!fresh || !this.#current(revision, id)) return;
        if (fresh.id !== id) {
          this.pollingError = "Could not refresh this domain: unexpected domain response";
          this.#stopPolling();
          return;
        }
        this.applyDetail(fresh);
      } catch (error) {
        if (!this.#current(revision, id)) return;
        this.pollingError = errorMessage(error, "Could not refresh DNS status");
      }
      if (!this.#current(revision, id) || !this.isPolling) return;
      if (this.#pollAttempts >= POLL_MAX_ATTEMPTS) {
        this.#stopPolling();
        this.pollingTimedOut = true;
        return;
      }
      // Schedule after completion: a slow request cannot overlap the next one.
      this.#pollTimer = setTimeout(poll, POLL_INTERVAL_MS);
    };
    this.#pollTimer = setTimeout(poll, POLL_INTERVAL_MS);
  }
  #stopPolling(): void {
    if (this.#pollTimer !== undefined) clearTimeout(this.#pollTimer);
    this.#pollTimer = undefined;
    this.isPolling = false;
  }
}
