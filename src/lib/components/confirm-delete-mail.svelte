<script lang="ts">
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let { open = $bindable(false), subject = "", count, draft = false, onConfirm }: {
    open?: boolean;
    subject?: string;
    count?: number;
    draft?: boolean;
    onConfirm: () => void;
  } = $props();
</script>

<AlertDialog.Root bind:open>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>{draft ? (count !== undefined ? `Delete ${count} selected drafts?` : "Delete this draft?") : count !== undefined ? `Delete ${count} selected emails from this device?` : "Delete this email from this device?"}</AlertDialog.Title>
      <AlertDialog.Description>
        {#if draft}
          {count !== undefined ? `The ${count} selected drafts` : `“${subject || "(No subject)"}”`} and their attachments will be permanently deleted from this device.
        {:else if count !== undefined}
          Only the {count} selected row messages will be hidden on this device, not deleted from Resend.
          Other messages in their conversations are not affected.
        {:else}
          “{subject || "(No subject)"}” will be hidden only on this device, not deleted from Resend.
          Only this message is affected, not the entire conversation.
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action variant="destructive" onclick={() => { open = false; onConfirm(); }}>Delete</AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
