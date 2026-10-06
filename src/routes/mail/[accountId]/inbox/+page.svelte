<script lang="ts">
  import { Inbox } from "@lucide/svelte";
</script>

<div class="flex h-full items-center justify-center">
  <div class="flex flex-col items-center gap-4 text-center">
    <div class="rounded-full bg-muted p-6">
      <Inbox class="size-12 text-muted-foreground" />
    </div>
    <div class="flex flex-col gap-2">
      <h2 class="text-xl font-semibold">No email selected</h2>
      <p class="max-w-sm text-sm text-muted-foreground">
        Select an email from the sidebar to view its contents.
      </p>
    </div>
  </div>
</div>
