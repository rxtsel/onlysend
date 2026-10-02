import { error } from "@sveltejs/kit";
import { getConnectionStatus, setActiveAccount } from "$lib/shared/api/auth";
import type { LayoutLoad } from "./$types";

export const load: LayoutLoad = async ({ params }) => {
  const accountId = params.accountId;
  try {
    // Reject invalid/deleted accounts; never substitute another account.
    await getConnectionStatus(accountId);
    await setActiveAccount(accountId); // Remember last visited account only.
  } catch {
    error(404, "This account is unavailable. Return to the connection screen.");
  }
  return { accountId };
};
