import { toast } from "svelte-sonner";
import { setMailDeleted, setMailsDeleted, type Mailbox } from "../local-mail";
import { errorMessage } from "../utils/errors";

/** Persist first: closing the app must not resurrect deleted mail. */
export async function deleteLocalMail(accountId: string, mailbox: Mailbox, emailId: string): Promise<boolean> {
  return deleteMail(accountId, mailbox, [emailId], false);
}

export async function deleteLocalMails(accountId: string, mailbox: Mailbox, emailIds: string[]): Promise<boolean> {
  const ids = [...new Set(emailIds)];
  if (!ids.length) return false;
  return deleteMail(accountId, mailbox, ids, true);
}

async function deleteMail(accountId: string, mailbox: Mailbox, ids: string[], bulk: boolean): Promise<boolean> {
  const persist = (deleted: boolean) => bulk
    ? setMailsDeleted(accountId, mailbox, ids, deleted)
    : setMailDeleted(accountId, mailbox, ids[0], deleted);
  const deletion = persist(true);
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
  toast.success(bulk ? `${ids.length} emails deleted from this device` : "Email deleted from this device", {
    duration: 5000,
    action: {
      label: "Undo",
      onClick: () => {
        if (Date.now() >= deadline) {
          toast.info("The undo window has expired");
          return;
        }
        toast.promise(persist(false), {
          loading: "Restoring email…",
          success: bulk ? "Emails restored" : "Email restored",
          error: (error) => errorMessage(error, "Could not restore email"),
        });
      },
    },
  });
  return true;
}
