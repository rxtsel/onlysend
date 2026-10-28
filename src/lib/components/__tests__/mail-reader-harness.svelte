<script lang="ts">
  import { provideAccount } from "$lib/features/auth/account-context";
  import { setContext } from "svelte";
  import type { MailListItem, Mailbox } from "$lib/shared/local-mail";
  import Reader from "../mail-reader.svelte";
  let { mailbox = "inbox", preview = null }: { mailbox?: Mailbox; preview?: { accountId: string; mailbox: Mailbox; mail: MailListItem } | null } = $props();
  provideAccount(() => "account-a");
  setContext("mail-list-preview", () => preview);
</script>
<Reader {mailbox} emailId="b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1" />
