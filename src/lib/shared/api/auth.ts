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

/**
 * Full credential logout: revokes the OAuth grant (if any), removes any
 * stored API key and resets setup flags. Local data is preserved.
 */
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
): Promise<string | null> {
  return await invoke<string | null>("remove_account", { accountId });
}
