import { invoke } from "@tauri-apps/api/core";

/** Native validation and platform-specific launch; callers surface failures. */
export async function openUrl(url: string): Promise<void> {
  await invoke("open_external_url", { url });
}
