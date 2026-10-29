import { toast } from "svelte-sonner";
import { setMailDeleted, type Mailbox } from "../local-mail";
import { errorMessage } from "../utils/errors";

/** Persist first: closing the app must not resurrect deleted mail. */
export async function deleteLocalMail(accountId: string, mailbox: Mailbox, emailId: string): Promise<boolean> {
  const deletion = setMailDeleted(accountId, mailbox, emailId, true);
  toast.promise(deletion, {
    loading: "Deleting from this device…",
    error: (error) => errorMessage(error, "Could not delete email"),
  });
  try {
    await deletion;
  } catch {
    return false;
  }
  const deadline = Date.now() + 5000;
  toast.success("Email deleted from this device", {
    duration: 5000,
    action: {
      label: "Undo",
      onClick: () => {
        if (Date.now() >= deadline) {
          toast.info("The undo window has expired");
          return;
        }
        toast.promise(setMailDeleted(accountId, mailbox, emailId, false), {
          loading: "Restoring email…",
          success: "Email restored",
          error: (error) => errorMessage(error, "Could not restore email"),
        });
      },
    },
  });
  return true;
}
