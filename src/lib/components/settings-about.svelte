<script lang="ts">
  import { LoaderCircle } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import AppUpdateControl from "$lib/components/app-update-control.svelte";
  import { appUpdate } from "$lib/shared/app-update.svelte";

  const busy = $derived(["checking", "downloading", "verifying", "installing"].includes(appUpdate.state.phase));
  const disabled = $derived(!appUpdate.canCheck() || appUpdate.state.info?.state === "disabled");
</script>

<section class="flex flex-col gap-4" aria-label="About OnlySend">
  <div class="flex flex-col gap-1">
    <h2 class="text-lg font-semibold">OnlySend</h2>
    <p class="text-sm text-muted-foreground">Version {appUpdate.state.currentVersion}</p>
    <p class="text-sm text-muted-foreground">A clean and minimal email composer powered by Resend.</p>
  </div>
  <div class="flex flex-col items-start gap-3">
    <Button variant="outline" onclick={() => appUpdate.checkNow()} disabled={busy || disabled}>
      {#if appUpdate.state.phase === "checking"}<LoaderCircle class="animate-spin motion-reduce:animate-none" aria-hidden="true" />{/if}
      {appUpdate.state.phase === "checking" ? "Checking for updates…" : "Check for updates"}
    </Button>
    {#if disabled}
      <p class="text-sm text-muted-foreground">Updates are not enabled in this build.</p>
    {:else if appUpdate.state.phase === "current"}
      <p class="text-sm text-muted-foreground" role="status">You're using the latest version.</p>
    {/if}
    {#if appUpdate.state.error && !appUpdate.state.info?.version}
      <p class="text-sm text-destructive" role="alert">{appUpdate.state.error}</p>
    {/if}
    <AppUpdateControl />
  </div>
</section>
