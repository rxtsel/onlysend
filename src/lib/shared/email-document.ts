import { invoke } from "@tauri-apps/api/core";

export interface EmailDocument {
  html: string;
  token: string;
}

export function prepareEmailDocument(html: string): Promise<EmailDocument> {
  return invoke("prepare_email_document", { html });
}

/** Opaque frames have origin "null"; source AND per-document identity are required. */
export function emailLinkFromMessage(
  event: Pick<MessageEvent, "source" | "origin" | "data">,
  source: Window | null,
  token: string | undefined,
): string | null {
  if (!source || !token || event.source !== source || event.origin !== "null") return null;
  const data = event.data;
  if (!data || data.type !== "onlysend-email-link" || data.token !== token || typeof data.url !== "string") return null;
  return data.url;
}
