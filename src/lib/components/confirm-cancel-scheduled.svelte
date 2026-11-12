<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import { errorMessage } from "$lib/shared/utils/errors";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let { open = $bindable(false), emailId, subject, alreadyCanceled = false }: { open?: boolean; emailId: string; subject: string; alreadyCanceled?: boolean } = $props();
  const accountId = useAccountId();
  let canceling = $state(false);
  async function cancel(event: MouseEvent) {
    event.preventDefault();
    if (canceling) return;
    canceling = true;
    try {
      const draftId = await invoke<string>("cancel_scheduled_email", { accountId, emailId });
      open = false;
      toast.success("Scheduled send canceled. Message restored to Drafts.");
      await goto(`${mailUrl(accountId, "composer")}?draft=${encodeURIComponent(draftId)}`)
        .catch(() => toast.error("Message is in Drafts, but could not be opened."));
    } catch (error) {
      toast.error(errorMessage(error, "Could not cancel scheduled email. Please try again."));
    } finally { canceling = false; }
  }
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content onEscapeKeydown={(event) => { if (canceling) event.preventDefault(); }}>
    <AlertDialog.Header>
      <AlertDialog.Title>{alreadyCanceled ? "Restore to Drafts?" : "Cancel scheduled send?"}</AlertDialog.Title>
      <AlertDialog.Description>{alreadyCanceled ? "This send is already canceled." : `Resend will stop sending “${subject || "(No subject)"}”.`} Its content and attachments will return to Drafts on this device. You can edit and schedule it again.</AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel disabled={canceling}>{alreadyCanceled ? "Keep in Sent" : "Keep scheduled"}</AlertDialog.Cancel>
      <AlertDialog.Action onclick={cancel} disabled={canceling}>{canceling ? (alreadyCanceled ? "Restoring…" : "Canceling…") : alreadyCanceled ? "Restore draft" : "Cancel send and restore draft"}</AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
