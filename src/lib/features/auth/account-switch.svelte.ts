import { tick } from "svelte";
import { goto } from "$app/navigation";
import { disconnectResend, setActiveAccount } from "@/lib/shared/api/auth";
import { inboundStatus } from "@/lib/shared/inbound-status.svelte";
import { emailCache } from "@/lib/features/sending/email-cache.svelte";

/** Log out with the same unmount/reset boundary used for switching accounts. */
export async function logoutMailAccount(): Promise<void> {
  if (accountSwitch.busy) throw new Error("An account operation is already in progress");
  accountSwitch.busy = true;
  try {
    await tick();
    await disconnectResend();
    // Root resolves the remaining account, or shows login when none remain.
    await goto("/");
  } finally {
    emailCache.clear();
    inboundStatus.ready = false;
    accountSwitch.generation += 1;
    accountSwitch.busy = false;
  }
}

/** Transitional context boundary until account IDs are part of every route. */
export const accountSwitch = $state({ busy: false, generation: 0 });

export async function switchMailAccount(accountId: string, pathname: string): Promise<void> {
  if (accountSwitch.busy) return;
  accountSwitch.busy = true;
  try {
    // Unmount the previous account before changing Rust's active pointer.
    await tick();
    // Never carry an email ID (or a compose form) into another account.
    await goto(pathname.startsWith("/mail/inbox") ? "/mail/inbox" : "/mail/sent");
    await setActiveAccount(accountId);
  } finally {
    // Also remount the original account if switching failed.
    emailCache.clear();
    inboundStatus.ready = false;
    accountSwitch.generation += 1;
    accountSwitch.busy = false;
  }
}
