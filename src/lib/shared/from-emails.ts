import { invoke } from "@tauri-apps/api/core";
import type { FromEmail } from "../types";

export function formatFromEmail(f: FromEmail) {
  return `${f.label} <${f.address}>`;
}

export async function listFromEmails(accountId: string): Promise<FromEmail[]> {
  return await invoke("list_from_emails", { accountId });
}

export async function createFromEmail(accountId: string, input: {
  label: string;
  address: string;
  isDefault?: boolean;
}): Promise<FromEmail> {
  return await invoke("create_from_email", { ...input, accountId });
}

export async function updateFromEmail(accountId: string, input: {
  id: string;
  label?: string;
  address?: string;
  isDefault?: boolean;
}): Promise<FromEmail> {
  return await invoke("update_from_email", { ...input, accountId });
}

export async function deleteFromEmail(accountId: string, id: string): Promise<void> {
  await invoke("delete_from_email", { accountId, id });
}

