import { invoke } from "@tauri-apps/api/core";
import type { SentEmail } from "../types/sent.type";
import { emailCache } from "../stores/email-cache.svelte";

export async function listSentEmails(
  limit?: number,
  offset?: number,
  forceRefresh = false
): Promise<SentEmail[]> {
  // Only use cache for initial load (offset 0)
  if (!forceRefresh && offset === 0) {
    const cached = emailCache.getList();
    if (cached) return cached
  }

  const emails = await invoke<SentEmail[]>("list_sent_emails", { limit, offset });

  // Cache only the initial load
  if (offset === 0) {
    emailCache.setList(emails);
  }

  return emails;
}

export async function getSentEmail(emailId: string): Promise<SentEmail> {
  // Check cache first
  const cached = emailCache.get(emailId);
  if (cached) {
    console.log(`Using cached email: ${emailId}`);
    return cached;
  }

  console.log(`Fetching email from API: ${emailId}`);
  const email = await invoke<SentEmail>("get_sent_email", { emailId });

  // Cache the result
  emailCache.set(emailId, email);

  return email;
}

export function invalidateEmailCache(): void {
  emailCache.invalidateList();
}

export function clearEmailCache(): void {
  emailCache.clear();
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

