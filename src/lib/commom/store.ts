import { invoke } from "@tauri-apps/api/core";

export async function hasApiKey(): Promise<boolean> {
  return await invoke<boolean>("has_api_key");
}

/** True when onboarding was completed with either API key or OAuth. */
export async function isAuthenticated(): Promise<boolean> {
  return await invoke<boolean>("is_authenticated");
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

export async function getApiKey(): Promise<string | null> {
  return await invoke<string | null>("get_api_key");
}

export async function deleteApiKey(): Promise<void> {
  await invoke("delete_api_key");
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
  recordType: string;
  name: string;
  value: string;
  status: string;
}

export interface DomainSummary {
  id: string;
  name: string;
  status: string;
}

export interface DomainCapabilities {
  sending: string;
  receiving: string;
}

export interface DomainDetail extends DomainSummary {
  capabilities: DomainCapabilities;
  records: DomainRecord[];
}

export async function listDomains(): Promise<DomainSummary[]> {
  return await invoke<DomainSummary[]>("list_domains");
}

export async function createDomain(name: string): Promise<DomainDetail> {
  return await invoke<DomainDetail>("create_domain", { name });
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

