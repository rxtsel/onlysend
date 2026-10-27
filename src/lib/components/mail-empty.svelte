<script lang="ts">
  import { Mail } from "@lucide/svelte";
</script>

<div class="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
  <div class="flex size-14 items-center justify-center rounded-2xl bg-muted"><Mail class="size-6 text-muted-foreground" aria-hidden="true" /></div>
  <h2 class="mt-2 text-base font-medium">No message selected</h2>
  <p class="max-w-64 text-sm leading-6 text-muted-foreground">Choose a message from the list to read it here.</p>
</div>
