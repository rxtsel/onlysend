import { invoke } from "@tauri-apps/api/core";
import type { FromEmail } from "../types";

export function formatFromEmail(f: FromEmail) {
  return `${f.label} <${f.address}>`;
}

export async function listFromEmails(): Promise<FromEmail[]> {
  return await invoke("list_from_emails");
}

export async function createFromEmail(input: {
  label: string;
  address: string;
  isDefault?: boolean;
}): Promise<FromEmail> {
  return await invoke("create_from_email", input);
}

export async function updateFromEmail(input: {
  id: string;
  label?: string;
  address?: string;
  isDefault?: boolean;
}): Promise<FromEmail> {
  return await invoke("update_from_email", input);
}

export async function deleteFromEmail(id: string): Promise<void> {
  await invoke("delete_from_email", { id });
}

