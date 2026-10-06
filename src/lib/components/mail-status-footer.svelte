<script lang="ts">
  import { Database, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { buttonVariants } from "$lib/components/ui/button";
  import { archiveStatusPresentation, type ArchiveFooterData } from "$lib/shared/archive-status";
  import type { Mailbox } from "$lib/shared/local-mail";

  let { data, mailbox }: { data: ArchiveFooterData; mailbox: Mailbox } = $props();
  const view = $derived(archiveStatusPresentation(data));
  const mailboxLabel = $derived(mailbox === "inbox" ? "Inbox" : "Sent");
  const status = $derived(data.status);
  const issue = $derived(data.statusError ?? data.refreshError ?? status?.sync?.lastError);
</script>

<footer class="fixed inset-x-0 bottom-0 flex h-8 items-center gap-2 border-t bg-background px-3 text-xs text-muted-foreground" aria-label="Mail status">
  <Tooltip.Provider delayDuration={150}>
    <Tooltip.Root>
      <Tooltip.Trigger
        class={buttonVariants({ variant: "ghost", size: "icon", class: "size-6 shrink-0" })}
        aria-label={`${mailboxLabel}: ${view.label}. Local-copy details`}
      >
        {#if view.kind === "syncing" || view.kind === "checking"}
          <LoaderCircle class="animate-spin motion-reduce:animate-none" aria-hidden="true" />
        {:else if view.kind === "error"}
          <TriangleAlert class="text-destructive" aria-hidden="true" />
        {:else}
          <Database aria-hidden="true" />
        {/if}
      </Tooltip.Trigger>
      <Tooltip.Content side="top" align="start" sideOffset={8} class="max-w-[min(22rem,calc(100vw-1rem))]">
        <div class="flex flex-col gap-1">
          <p class="font-medium">Local copy · {mailboxLabel}</p>
          {#if status}
            <p>{status.downloadedMessages} messages · {status.downloadedBodies} bodies</p>
            {#if status.running}
              <p>{status.sync?.metadataCompletedAt ? "Metadata downloaded; copying bodies…" : "Downloading metadata…"}</p>
            {/if}
            {#if status.sync?.completedAt != null}
              <p>Last full download: {new Date(status.sync.completedAt * 1000).toLocaleString()}</p>
            {:else if !status.running}
              <p>Full download not completed.</p>
            {/if}
          {:else}
            <p>Local-copy status is not available yet.</p>
          {/if}
          {#if data.downloadedAt != null}
            <p>Downloaded mail: {new Date(data.downloadedAt * 1000).toLocaleString()}</p>
          {/if}
          {#if issue}
            <p class="break-words">{issue}</p>
            <p>Use Refresh to retry.</p>
          {/if}
        </div>
      </Tooltip.Content>
    </Tooltip.Root>
  </Tooltip.Provider>
  {#if view.kind !== "ready"}
    <span class="truncate" aria-hidden="true">
      {view.label}{view.kind === "syncing" && status ? ` · ${status.downloadedMessages} messages · ${status.downloadedBodies} bodies` : ""}
    </span>
  {/if}
  <span class="sr-only" role="status" aria-live="polite">{mailboxLabel}: {view.label}</span>
</footer>
