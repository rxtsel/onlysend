<script lang="ts">
  import { page } from "$app/state";
  import MailReader from "$lib/components/mail-reader.svelte";
</script>

<MailReader mailbox="inbox" emailId={page.params.id} />
