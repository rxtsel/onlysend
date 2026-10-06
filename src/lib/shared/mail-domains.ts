import type { InboundEmail } from "./inbound";
import type { SentEmail } from "$lib/types";

export function mailDomains(mail: InboundEmail | SentEmail): string[] {
  if ("domains" in mail) return mail.domains.length ? mail.domains : ["unknown"];
  const host = mail.from.split("@").at(-1)?.trim().replace(/>$/, "").toLowerCase();
  return mail.from.includes("@") && host ? [host] : ["unknown"];
}
