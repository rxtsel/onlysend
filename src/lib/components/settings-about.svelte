<script lang="ts">
  import { LoaderCircle } from "@lucide/svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { toast } from "svelte-sonner";
  import { Button } from "$lib/components/ui/button";
  import * as Field from "$lib/components/ui/field";
  import * as Item from "$lib/components/ui/item";
  import AppUpdateControl from "$lib/components/app-update-control.svelte";
  import { appUpdate } from "$lib/shared/app-update.svelte";

  const busy = $derived(["checking", "downloading", "verifying", "installing"].includes(appUpdate.state.phase));
  const disabled = $derived(!appUpdate.canCheck() || appUpdate.state.info?.state === "disabled");
</script>

<section class="w-full" aria-label="About OnlySend">
  <Field.Group>
    <div class="mb-2 flex flex-col gap-1">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h3 class="text-base font-medium">OnlySend</h3>
        <span class="rounded bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground">Version {appUpdate.state.currentVersion}</span>
      </div>
      <Field.Description>
        A clean and minimal email composer by <a
          href="https://rxtsel.dev/?utm_source=onlysend&utm_medium=app&utm_campaign=about&ref=onlysend"
          onclick={async (event) => {
            event.preventDefault();
            try { await openUrl(event.currentTarget.href); }
            catch { toast.error("Could not open rxtsel.dev"); }
          }}
        >Rxtsel</a>, powered by Resend.
      </Field.Description>
    </div>
    <Item.Root variant="outline" size="sm">
      <Item.Content class="min-w-48">
        <Item.Title>App updates</Item.Title>
        <Item.Description class="line-clamp-none">
          {#if disabled}
            Updates are not enabled in this build.
          {:else if appUpdate.state.phase === "current"}
            <span role="status">You're using the latest version.</span>
          {:else}
            Check for the latest version of OnlySend.
          {/if}
        </Item.Description>
      </Item.Content>
      <Item.Actions>
        <Button variant="outline" onclick={() => appUpdate.checkNow()} disabled={busy || disabled}>
          {#if appUpdate.state.phase === "checking"}<LoaderCircle class="animate-spin motion-reduce:animate-none" aria-hidden="true" />{/if}
          {appUpdate.state.phase === "checking" ? "Checking for updates…" : "Check for updates"}
        </Button>
      </Item.Actions>
    </Item.Root>
    {#if appUpdate.state.error && !appUpdate.state.info?.version}
      <Field.Error>{appUpdate.state.error}</Field.Error>
    {/if}
    <AppUpdateControl />
  </Field.Group>
</section>
