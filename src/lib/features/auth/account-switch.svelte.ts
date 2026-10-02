import { tick } from "svelte";
import { goto } from "$app/navigation";
import { disconnectResend } from "$lib/shared/api/auth";
import { clearInboundStatus } from "$lib/shared/inbound-status.svelte";
import { emailCache } from "$lib/features/sending/email-cache.svelte";
import { mailUrl, mailboxFromPath } from "./mail-routes";

export const accountSwitch = $state({ busy: false, generation: 0 });

export async function logoutMailAccount(accountId: string): Promise<void> {
  if (accountSwitch.busy) throw new Error("An account operation is already in progress");
  accountSwitch.busy = true;
  try {
    await tick();
    await disconnectResend(accountId);
    emailCache.clear(accountId);
    clearInboundStatus(accountId);
    await goto("/");
  } finally {
    accountSwitch.generation += 1;
    accountSwitch.busy = false;
  }
}

export async function switchMailAccount(accountId: string, pathname: string): Promise<void> {
  if (accountSwitch.busy) return;
  accountSwitch.busy = true;
  try {
    await tick();
    const mailbox = mailboxFromPath(pathname) === "inbox" ? "inbox" : "sent";
    // Route validation rejects unknown accounts. Navigation never carries a
    // message ID or composer state from the previous account.
    await goto(mailUrl(accountId, mailbox));
  } finally {
    accountSwitch.generation += 1;
    accountSwitch.busy = false;
  }
}
