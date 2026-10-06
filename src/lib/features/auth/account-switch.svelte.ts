import { goto } from "$app/navigation";
import { disconnectResend } from "$lib/shared/api/auth";
import { clearInboundStatus } from "$lib/shared/inbound-status.svelte";
import { emailCache } from "$lib/features/sending/email-cache.svelte";
import { mailUrl, mailboxFromPath } from "./mail-routes";
import { approveAccountDeparture, resetDepartureApproval } from "./draft-navigation";

export const accountSwitch = $state({ busy: false });

export async function logoutMailAccount(accountId: string): Promise<boolean> {
  if (accountSwitch.busy) throw new Error("An account operation is already in progress");
  if (!approveAccountDeparture(accountId)) return false;
  accountSwitch.busy = true;
  try {
    await disconnectResend(accountId);
    emailCache.clear(accountId);
    clearInboundStatus(accountId);
    await goto("/");
    return true;
  } finally {
    resetDepartureApproval(accountId);
    accountSwitch.busy = false;
  }
}

export async function switchMailAccount(accountId: string, pathname: string): Promise<void> {
  if (accountSwitch.busy) return;
  const currentAccountId = decodeURIComponent(pathname.split("/")[2]);
  if (!approveAccountDeparture(currentAccountId)) return;
  accountSwitch.busy = true;
  try {
    const mailbox = mailboxFromPath(pathname) === "inbox" ? "inbox" : "sent";
    // Route validation rejects unknown accounts. Navigation never carries a
    // message ID or composer state from the previous account.
    await goto(mailUrl(accountId, mailbox));
  } finally {
    resetDepartureApproval(currentAccountId);
    accountSwitch.busy = false;
  }
}
