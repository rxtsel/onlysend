import { invoke } from "@tauri-apps/api/core";

export interface DomainRecord {
  /** Record group: "SPF" | "DKIM" | "Receiving MX" | "Tracking" | ... */
  group: string;
  recordType: string;
  name: string;
  value: string;
  status: string;
  ttl: string;
  priority?: number | null;
}

export interface DomainCapabilities {
  sending: string;
  receiving: string;
}

export interface DomainSummary {
  id: string;
  name: string;
  status: string;
  capabilities?: DomainCapabilities;
}

export interface DomainDetail extends DomainSummary {
  capabilities: DomainCapabilities;
  records: DomainRecord[];
}

export async function listDomains(accountId: string): Promise<DomainSummary[]> {
  return await invoke<DomainSummary[]>("list_domains", { accountId });
}

export async function createDomain(accountId: string, options: {
  name: string;
  region?: string;
  enableReceiving?: boolean;
}): Promise<DomainDetail> {
  const { name, region, enableReceiving } = options;
  return await invoke<DomainDetail>("create_domain", { accountId,
    name,
    region,
    enableReceiving,
  });
}

export async function deleteDomain(accountId: string, domainId: string): Promise<boolean> {
  return await invoke<boolean>("delete_domain", { accountId, domainId });
}

export async function getDomain(accountId: string, domainId: string): Promise<DomainDetail> {
  return await invoke<DomainDetail>("get_domain", { accountId, domainId });
}

export async function verifyDomain(accountId: string, domainId: string): Promise<DomainDetail> {
  return await invoke<DomainDetail>("verify_domain", { accountId, domainId });
}

export async function setDomainReceiving(
  accountId: string,
  domainId: string,
  enable: boolean,
): Promise<DomainDetail> {
  return await invoke<DomainDetail>("set_domain_receiving", { accountId,
    domainId,
    enable,
  });
}

/* ---------------------------------------------------------
 * SELECTED DOMAIN
 * --------------------------------------------------------- */
export async function saveSelectedDomain(accountId: string, domain: string): Promise<void> {
  await invoke("save_selected_domain", { accountId, domain });
}

export async function getSelectedDomain(accountId: string): Promise<string | null> {
  return await invoke<string | null>("get_selected_domain", { accountId });
}

export async function getActiveDomain(accountId: string): Promise<string | null> {
  return await invoke<string | null>("get_active_domain", { accountId });
}

/* ---------------------------------------------------------
 * INBOUND SETUP CACHE (last-known domain detail)
 * --------------------------------------------------------- */
export interface InboundSetupCache {
  id: string;
  name: string;
  status: string;
  capabilities: DomainCapabilities;
  records: DomainRecord[];
}

export async function getInboundSetupCache(accountId: string): Promise<InboundSetupCache | null> {
  return await invoke<InboundSetupCache | null>("get_inbound_setup_cache", { accountId });
}

export async function saveInboundSetupCache(
  accountId: string,
  detail: InboundSetupCache,
): Promise<void> {
  await invoke("save_inbound_setup_cache", { accountId, detail });
}
