<script lang="ts">
  import Layout from "../../../routes/mail/[accountId]/+layout.svelte";
  import Composer from "../../../routes/mail/[accountId]/composer/+page.svelte";
  let { composer = false }: { composer?: boolean } = $props();
</script>
<Layout data={{ accountId: "account-a" }}>
  {#if composer}<Composer />{:else}<p>Message content</p>{/if}
</Layout>
