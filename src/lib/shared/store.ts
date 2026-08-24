import { invoke } from "@tauri-apps/api/core";

export async function hasApiKey(): Promise<boolean> {
  return await invoke<boolean>("has_api_key");
}

/** True when onboarding was completed with either API key or OAuth. */
export interface OnboardingState {
  complete: boolean;
  authenticated: boolean;
  inboxEnabled: boolean;
}

export async function getOnboardingState(): Promise<OnboardingState> {
  return await invoke<OnboardingState>("get_onboarding_state");
}

export async function setInboxEnabled(enabled: boolean): Promise<void> {
  await invoke("set_inbox_enabled", { enabled });
}

export interface InboundSetupCache {
  id: string;
  name: string;
  status: string;
  capabilities: DomainCapabilities;
  records: DomainRecord[];
}

export async function getInboundSetupCache(): Promise<InboundSetupCache | null> {
  return await invoke<InboundSetupCache | null>("get_inbound_setup_cache");
}

export async function saveInboundSetupCache(
  detail: InboundSetupCache,
): Promise<void> {
  await invoke("save_inbound_setup_cache", { detail });
}

export async function markSetupComplete(): Promise<void> {
  await invoke("mark_setup_complete");
}

export type ConnectionMethod = "oauth" | "api_key" | null;

export interface ConnectionStatus {
  method: ConnectionMethod;
}

export async function getConnectionStatus(): Promise<ConnectionStatus> {
  return await invoke<ConnectionStatus>("get_connection_status");
}

/** Starts the OAuth flow and resolves with the authorization URL. */
export async function connectResend(): Promise<string> {
  return await invoke<string>("connect_resend");
}

export async function disconnectResend(): Promise<void> {
  await invoke("disconnect_resend");
}

export async function saveApiKey(apiKey: string): Promise<void> {
  await invoke("save_api_key", { apiKey });
}

/* ---------------------------------------------------------
 * PERMISSIONS
 * --------------------------------------------------------- */
export interface ProbeResult {
  fullAccess: boolean;
}

export async function probeFullAccess(credential: string): Promise<ProbeResult> {
  return await invoke<ProbeResult>("probe_full_access", { credential });
}

/* ---------------------------------------------------------
 * DOMAINS
 * --------------------------------------------------------- */
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

export async function listDomains(): Promise<DomainSummary[]> {
  return await invoke<DomainSummary[]>("list_domains");
}

export async function createDomain(options: {
  name: string;
  region?: string;
  enableReceiving?: boolean;
}): Promise<DomainDetail> {
  const { name, region, enableReceiving } = options;
  return await invoke<DomainDetail>("create_domain", {
    name,
    region,
    enableReceiving,
  });
}

export async function deleteDomain(domainId: string): Promise<boolean> {
  return await invoke<boolean>("delete_domain", { domainId });
}

export async function getDomain(domainId: string): Promise<DomainDetail> {
  return await invoke<DomainDetail>("get_domain", { domainId });
}

export async function verifyDomain(domainId: string): Promise<DomainDetail> {
  return await invoke<DomainDetail>("verify_domain", { domainId });
}

export async function setDomainReceiving(
  domainId: string,
  enable: boolean,
): Promise<DomainDetail> {
  return await invoke<DomainDetail>("set_domain_receiving", {
    domainId,
    enable,
  });
}

/* ---------------------------------------------------------
 * SELECTED DOMAIN
 * --------------------------------------------------------- */
export async function saveSelectedDomain(domain: string): Promise<void> {
  await invoke("save_selected_domain", { domain });
}

export async function getSelectedDomain(): Promise<string | null> {
  return await invoke<string | null>("get_selected_domain");
}

export async function getActiveDomain(): Promise<string | null> {
  return await invoke<string | null>("get_active_domain");
}

