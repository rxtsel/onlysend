<script lang="ts">
  import { page } from "$app/state";
  import DraftReader from "$lib/components/draft-reader.svelte";
</script>

<DraftReader draftId={page.params.id!} />
