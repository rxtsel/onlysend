<script lang="ts">
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let { open = $bindable(false), subject, onConfirm }: {
    open?: boolean;
    subject: string;
    onConfirm: () => void;
  } = $props();
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete this email from this device?</AlertDialog.Title>
      <AlertDialog.Description>
        “{subject || "(No subject)"}” will be hidden only on this device, not deleted from Resend.
        Only this message is affected, not the entire conversation.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action variant="destructive" onclick={() => { open = false; onConfirm(); }}>Delete</AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
