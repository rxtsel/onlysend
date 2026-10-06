<script lang="ts">
  import { ArrowUpCircle, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { Button } from "$lib/components/ui/button";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { appUpdate } from "$lib/shared/app-update.svelte";
  import { confirmAppDeparture } from "$lib/features/auth/draft-navigation";

  let open = $state(false);
  let linkError = $state<string | null>(null);
  const info = $derived(appUpdate.state.info);
  const busy = $derived(["downloading", "verifying", "installing"].includes(appUpdate.state.phase));
  const progress = $derived(appUpdate.state.total && appUpdate.state.total > 0
    ? ` · ${Math.min(100, Math.floor(appUpdate.state.downloaded / appUpdate.state.total * 100))}%` : "");

  async function install(event: MouseEvent) {
    event.preventDefault();
    if (busy || !confirmAppDeparture()) return;
    await appUpdate.install();
  }

  async function download() {
    try {
      await openUrl("https://github.com/rxtsel/onlysend/releases/latest");
      linkError = null;
    } catch (error) { linkError = String(error); }
  }
</script>

{#if info?.version}
  <AlertDialog.Root bind:open>
    <AlertDialog.Trigger class="inline-flex items-center gap-1 truncate text-primary hover:underline focus-visible:outline focus-visible:outline-ring" aria-label={`Update available: ${info.version}`}>
      {#if busy}
        <LoaderCircle class="size-3 animate-spin motion-reduce:animate-none" aria-hidden="true" />
        <span role="status">{appUpdate.state.phase === "installing" ? "Installing update…" : appUpdate.state.phase === "verifying" ? "Verifying update…" : `Downloading update${progress}`}</span>
      {:else}
        <ArrowUpCircle class="size-3" aria-hidden="true" />
        <span>Update available · v{info.version}</span>
      {/if}
    </AlertDialog.Trigger>
    <AlertDialog.Content onEscapeKeydown={(event) => { if (busy) event.preventDefault(); }}>
      <AlertDialog.Header>
        <AlertDialog.Title>Update OnlySend to v{info.version}</AlertDialog.Title>
        <AlertDialog.Description>
          {#if info.installSupported}
            Downloading does not close the app. Once verified, installation will close and restart OnlySend. Unsaved drafts must be discarded before installing. Your accounts and local mail archive will be retained.
          {:else}
            This installation is managed outside the app. Download the new Debian package and install it using your package manager. Your accounts and local mail archive will be retained.
          {/if}
        </AlertDialog.Description>
      </AlertDialog.Header>
      {#if info.notes}
        <pre class="max-h-64 overflow-y-auto whitespace-pre-wrap break-words font-sans text-sm">{info.notes}</pre>
      {/if}
      {#if appUpdate.state.error || linkError}
        <p class="text-sm text-destructive" role="alert">{appUpdate.state.error ?? linkError}</p>
      {/if}
      {#if busy}
        <p class="text-sm" role="status">{appUpdate.state.phase === "installing" ? "Installing update…" : appUpdate.state.phase === "verifying" ? "Verifying update…" : `Downloading update${progress}`}</p>
      {/if}
      <AlertDialog.Footer>
        <AlertDialog.Cancel disabled={busy}>Later</AlertDialog.Cancel>
        {#if info.installSupported}
          {#if appUpdate.state.verified}
            <AlertDialog.Action onclick={install} disabled={busy}>Install and restart</AlertDialog.Action>
          {:else}
            <Button onclick={() => appUpdate.download()} disabled={busy}>Download update</Button>
          {/if}
        {:else}
          <Button onclick={download}>Open downloads</Button>
        {/if}
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>
{:else if appUpdate.state.error}
  <span title={`Could not check for updates: ${appUpdate.state.error}`} aria-label="Could not check for updates">
    <TriangleAlert class="size-3" aria-hidden="true" />
  </span>
{/if}
