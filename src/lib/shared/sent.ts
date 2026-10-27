import { invoke } from "@tauri-apps/api/core";
import type { SentEmail } from "$lib/types";

/** Downloaded details and account isolation are owned by Rust/SQLite. */
export function getSentEmail(accountId: string, emailId: string): Promise<SentEmail> {
  return invoke("get_sent_email", { accountId, emailId });
}
