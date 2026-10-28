import { invoke } from "@tauri-apps/api/core";

export interface EmailDocument {
  html: string;
  token: string;
}

export function prepareEmailDocument(html: string): Promise<EmailDocument> {
  // Inert parsing preserves html/body styles that Ammonia's fragment parser drops.
  const parsed = new DOMParser().parseFromString(html, "text/html");
  return invoke("prepare_email_document", {
    html,
    htmlAttributes: Object.fromEntries(Array.from(parsed.documentElement.attributes, ({ name, value }) => [name, value])),
    bodyAttributes: Object.fromEntries(Array.from(parsed.body.attributes, ({ name, value }) => [name, value])),
  });
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
