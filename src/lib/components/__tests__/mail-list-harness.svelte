<script lang="ts">
  import { provideAccount } from "$lib/features/auth/account-context";
  import type { Mailbox } from "$lib/shared/local-mail";
  import MailList from "../mail-list.svelte";
  let { controls, onSelectMail = () => {} }: {
    controls: { mailbox: Mailbox; domain: string | null; active: boolean };
    onSelectMail?: (mail: import("$lib/shared/local-mail").MailListItem, mailbox: Mailbox) => void;
  } = $props();
  provideAccount(() => "account-a");
</script>
<MailList mailbox={controls.mailbox} domain={controls.domain} active={controls.active} {onSelectMail} onDomainsChange={() => {}} onArchiveStatusChange={() => {}} />
