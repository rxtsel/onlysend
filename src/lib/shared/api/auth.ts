import { invoke } from "@tauri-apps/api/core";

export async function hasApiKey(accountId: string): Promise<boolean> {
  return await invoke<boolean>("has_api_key", { accountId });
}

/** True when onboarding was completed with either API key or OAuth. */
export interface OnboardingState {
  complete: boolean;
  authenticated: boolean;
  inboxEnabled: boolean;
}

export async function getOnboardingState(accountId: string): Promise<OnboardingState> {
  return await invoke<OnboardingState>("get_onboarding_state", { accountId });
}

export async function setInboxEnabled(accountId: string, enabled: boolean): Promise<void> {
  await invoke("set_inbox_enabled", { accountId, enabled });
}

export async function markSetupComplete(accountId: string): Promise<void> {
  await invoke("mark_setup_complete", { accountId });
}

export type ConnectionMethod = "oauth" | "api_key" | null;

export interface ConnectionStatus {
  method: ConnectionMethod;
}

export async function getConnectionStatus(accountId: string): Promise<ConnectionStatus> {
  return await invoke<ConnectionStatus>("get_connection_status", { accountId });
}

/** Starts the OAuth flow and resolves with the authorization URL. */
export async function connectResend(): Promise<string> {
  return await invoke<string>("connect_resend");
}

/**
 * Log out one account: remove its saved connection and revoke OAuth when possible.
 * The caller must identify the account explicitly.
 */
export async function disconnectResend(accountId: string): Promise<void> {
  await invoke("disconnect_resend", { accountId });
}

export async function saveApiKey(apiKey: string): Promise<string> {
  return await invoke<string>("save_api_key", { apiKey });
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
 * MULTI-ACCOUNT
 * --------------------------------------------------------- */
export interface AccountMeta {
  id: string;
  label: string;
  method: string;
  isActive: boolean;
}

export async function listAccounts(): Promise<AccountMeta[]> {
  return await invoke<AccountMeta[]>("list_accounts");
}

export async function setActiveAccount(accountId: string): Promise<void> {
  await invoke("set_active_account", { accountId });
}

/** Removes an account entirely (credential + entry). */
export async function removeAccount(
  accountId: string,
): Promise<void> {
  await invoke("remove_account", { accountId });
}
