import { markSetupComplete } from "$lib/shared/api/auth";
import { saveSelectedDomain } from "$lib/shared/api/domains";
import { createFromEmail, listFromEmails, updateFromEmail } from "$lib/shared/from-emails";

type SenderOption = { label: string; address: string };

/** Local setup is retryable: do not duplicate identities or mark partial saves complete. */
export async function finishAccountSetup(accountId: string, options: SenderOption[], fallbackDomain: string): Promise<void> {
  const senders = options.map((option) => ({ ...option }));
  const existing = await listFromEmails(accountId);
  for (const [index, option] of senders.entries()) {
    const saved = existing.find((sender) => sender.address.toLowerCase() === option.address.toLowerCase());
    const input = { ...option, isDefault: index === 0 };
    if (saved) await updateFromEmail(accountId, { ...input, id: saved.id });
    else await createFromEmail(accountId, input);
  }
  const defaultDomain = senders[0]?.address.split("@").at(-1) ?? fallbackDomain;
  if (defaultDomain) await saveSelectedDomain(accountId, defaultDomain);
  await markSetupComplete(accountId);
}
