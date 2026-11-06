import { goto } from "$app/navigation";
import { disconnectResend } from "$lib/shared/api/auth";
import { clearInboundStatus } from "$lib/shared/inbound-status.svelte";
import { mailUrl, mailboxFromPath } from "./mail-routes";
import { approveAccountDeparture, resetDepartureApproval, saveDrafts } from "./draft-navigation";

export const accountSwitch = $state({ busy: false });

export async function logoutMailAccount(accountId: string): Promise<boolean> {
  if (accountSwitch.busy) throw new Error("An account operation is already in progress");
  accountSwitch.busy = true;
  try {
    await saveDrafts(accountId);
    if (!approveAccountDeparture(accountId)) return false;
    await disconnectResend(accountId);
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
  accountSwitch.busy = true;
  try {
    await saveDrafts(currentAccountId);
    if (!approveAccountDeparture(currentAccountId)) return;
    const current = mailboxFromPath(pathname);
    const mailbox = current === "composer" ? "drafts" : current;
    // Route validation rejects unknown accounts. Navigation never carries a
    // message ID or composer state from the previous account.
    await goto(mailUrl(accountId, mailbox));
  } finally {
    resetDepartureApproval(currentAccountId);
    accountSwitch.busy = false;
  }
}
