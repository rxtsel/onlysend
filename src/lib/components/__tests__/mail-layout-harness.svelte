<script lang="ts">
  import Layout from "../../../routes/mail/[accountId]/+layout.svelte";
  import Composer from "../../../routes/mail/[accountId]/composer/+page.svelte";
  import MailDetail from "./mail-detail-probe.svelte";
  let { composer = false, threaded = null }: { composer?: boolean; threaded?: boolean | null } = $props();
</script>
<Layout data={{ accountId: "account-a" }}>
  {#if composer}<Composer />{:else if threaded !== null}<MailDetail {threaded} />{:else}<p>Message content</p>{/if}
</Layout>
