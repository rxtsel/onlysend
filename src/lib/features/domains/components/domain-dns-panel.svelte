<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { DomainSetupStore } from "$lib/features/domains/domain-setup-store.svelte";
  import DnsRecordsCard from "$lib/features/setup/components/dns-records-card.svelte";
  import type { DomainSummary } from "$lib/shared/api/domains";
  import { errorMessage } from "$lib/shared/utils/errors";

  let { accountId, domainId, onUpdated, onClose, closeLabel = "Configure later" }: {
    accountId: string;
    domainId: string;
    onUpdated: (domain: DomainSummary) => void;
    onClose: () => void;
    closeLabel?: string;
  } = $props();
  const store = new DomainSetupStore(untrack(() => accountId));
  let loading = $state(true);
  let error = $state<string | null>(null);
  let disposed = false;
  onDestroy(() => { disposed = true; store.dispose(); });
  $effect(() => {
    const detail = store.setupDetail;
    if (detail && !disposed) untrack(() => onUpdated(detail));
  });

  async function load() {
    loading = true;
    error = null;
    try {
      await store.loadFresh(domainId);
    } catch (err) {
      if (!disposed) error = errorMessage(err, "Could not load DNS records");
    } finally { if (!disposed) loading = false; }
  }
  async function act(action: () => Promise<void>) {
    error = null;
    try {
      await action();
    } catch (err) {
      if (!disposed) error = errorMessage(err, "Could not update this domain");
    }
  }
  function toggleReceiving(enabled: boolean) {
    if (enabled && !window.confirm("Changing MX records can redirect incoming email away from your current provider. Enable receiving for this domain?")) return;
    void act(() => store.toggleReceiving(enabled));
  }
  onMount(() => { void load(); });
</script>

<div class="flex flex-col gap-3 p-3">
  {#if loading}
    <p class="text-sm text-muted-foreground" role="status">Loading this domain's DNS records…</p>
  {/if}
  {#if error}
    <p class="text-sm text-destructive" role="alert">{error}</p>
    <Button variant="outline" size="sm" onclick={load}>Retry</Button>
  {/if}
  {#if store.isPolling}
    <p class="text-xs text-muted-foreground" role="status">Checking DNS status automatically every 5 seconds…</p>
  {/if}
  {#if store.pollingError}
    <p class="text-sm text-destructive" role="alert">
      {store.pollingError}{store.isPolling ? " Retrying automatically." : ""}
    </p>
  {/if}
  {#if store.pollingTimedOut}
    <p class="text-sm text-muted-foreground" role="status">
      Automatic checks paused. DNS propagation may take longer; refresh to check again.
    </p>
  {/if}
  {#if store.pollingTimedOut || store.pollingError}
    <Button variant="outline" size="sm" disabled={loading || store.isVerifying || store.isTogglingReceiving} onclick={load}>
      Refresh status
    </Button>
  {/if}
  {#if store.setupDetail}
    <DnsRecordsCard
      detail={store.setupDetail}
      bordered={false}
      isVerifying={store.isVerifying}
      isTogglingReceiving={store.isTogglingReceiving}
      onVerify={() => { void act(() => store.verify()); }}
      onToggleReceiving={toggleReceiving}
    />
  {/if}
  <Button variant="ghost" size="sm" onclick={onClose}>{closeLabel}</Button>
</div>
