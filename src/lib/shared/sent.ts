import { invoke } from "@tauri-apps/api/core";
import type { SentEmail } from "../types/sent.type";
import { emailCache } from "@/lib/features/sending/email-cache.svelte";

export async function listSentEmails(
  accountId: string,
  limit?: number,
  offset?: number,
  forceRefresh = false
): Promise<SentEmail[]> {
  // Only use cache for initial load (offset 0)
  if (!forceRefresh && offset === 0) {
    const cached = emailCache.getList(accountId, limit);
    if (cached) return cached
  }

  const generation = emailCache.generation(accountId);
  const emails = await invoke<SentEmail[]>("list_sent_emails", { accountId, limit, offset });

  // Cache only the initial load
  if (offset === 0 && generation === emailCache.generation(accountId)) {
    emailCache.setList(accountId, emails, limit);
  }

  return emails;
}

export async function getSentEmail(accountId: string, emailId: string): Promise<SentEmail> {
  // Check cache first
  const cached = emailCache.get(accountId, emailId);
  if (cached) {
    console.log(`Using cached email: ${emailId}`);
    return cached;
  }

  console.log(`Fetching email from API: ${emailId}`);
  const generation = emailCache.generation(accountId);
  const email = await invoke<SentEmail>("get_sent_email", { accountId, emailId });

  // An old account's request may finish after the cache was reset.
  if (generation === emailCache.generation(accountId)) emailCache.set(accountId, emailId, email);

  return email;
}

export function invalidateEmailCache(accountId: string): void {
  emailCache.invalidateList(accountId);
}

// For conditional render beetween html or Editor Tap component
export function isTemplateHtml(html: string | null): boolean {
  if (!html) return false;

  // 1) Has <style> tag
  if (/<style[\s\S]*?>[\s\S]*?<\/style>/i.test(html)) return true;

  // 2) Inline styles: style="..."
  if (/style\s*=\s*["'][^"']+["']/i.test(html)) return true;

  // 3) Typical email-layout tags or attributes
  if (/(<table|<tr|<td|<font|<center|bgcolor\s*=)/i.test(html)) return true;

  // 4) Many classes (very rough heuristic)
  const classMatches = html.match(/class\s*=\s*["'][^"']+["']/gi);
  if (classMatches && classMatches.length > 5) return true;

  return false;
}

