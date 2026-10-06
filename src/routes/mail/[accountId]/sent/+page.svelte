<script lang="ts">
  import { Mail } from "@lucide/svelte/icons";
</script>

<div class="flex h-full items-center justify-center">
  <div class="flex flex-col items-center gap-4 text-center">
    <div class="rounded-full bg-muted p-6">
      <Mail class="h-12 w-12 text-muted-foreground" />
    </div>
    <div class="space-y-2">
      <h2 class="text-xl font-semibold">No email selected</h2>
      <p class="text-sm text-muted-foreground max-w-sm">
        Select an email from the sidebar to view its contents, or compose a new
        message to get started.
      </p>
    </div>
  </div>
</div>
