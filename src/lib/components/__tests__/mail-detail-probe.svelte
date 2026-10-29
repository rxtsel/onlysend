<script lang="ts">
  import { getContext, onMount } from "svelte";
  let { threaded }: { threaded: boolean } = $props();
  const setSubject = getContext<(subject: string) => void>("mail-subject");
  const setThreadStatus = getContext<(threaded: boolean) => void>("mail-thread-status");
  onMount(() => { setSubject("A long message subject"); setThreadStatus(threaded); });
</script>
<p>Message content</p>
