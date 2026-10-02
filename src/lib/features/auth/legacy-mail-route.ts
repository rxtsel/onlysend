import { redirect } from "@sveltejs/kit";
import { listAccounts } from "$lib/shared/api/auth";
import { mailUrl, type Mailbox } from "./mail-routes";

/** Legacy bookmarks have no account ownership. Never reuse their message IDs. */
export function legacyMailRoute(mailbox: Mailbox) {
  return async () => {
    const accounts = await listAccounts();
    const account = accounts.find((item) => item.isActive) ?? accounts[0];
    redirect(307, account ? mailUrl(account.id, mailbox) : "/");
  };
}
