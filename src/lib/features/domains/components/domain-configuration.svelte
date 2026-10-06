<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { DomainSelectionStore } from "$lib/features/domains/domain-selection-store.svelte";
  import { DomainSetupStore, sendingReady } from "$lib/features/domains/domain-setup-store.svelte";
  import type { DomainSummary } from "$lib/shared/api/domains";
  import { errorMessage } from "$lib/shared/utils/errors";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import * as Field from "$lib/components/ui/field";
  import * as Collapsible from "$lib/components/ui/collapsible";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import { ChevronDown, Plus } from "@lucide/svelte";
  import DomainDnsPanel from "$lib/features/domains/components/domain-dns-panel.svelte";
  import CreateDomainForm from "$lib/features/domains/components/create-domain-form.svelte";

  const accountId = useAccountId();
  let { domain = $bindable(""), selectedDomains = $bindable([]), onSaved, mode = "setup" }: {
    domain?: string;
    selectedDomains?: DomainSummary[];
    onSaved: (domains: DomainSummary[]) => void;
    mode?: "setup" | "settings";
  } = $props();
  const store = new DomainSelectionStore(accountId);
  const creator = new DomainSetupStore(accountId);
  let expanded = $state<string[]>([]);
  let showCreate = $state(false);
  let actionError = $state<string | null>(null);
  let disposed = false;
  $effect(() => {
    selectedDomains = store.included;
    const ready = store.included.filter(sendingReady);
    if (!ready.some((item) => item.name === domain)) domain = ready[0]?.name ?? "";
  });
  onMount(() => { void store.load(); });
  onDestroy(() => { disposed = true; store.dispose(); creator.dispose(); });

  function expand(id: string, open: boolean) {
    expanded = open ? [...new Set([...expanded, id])] : expanded.filter((item) => item !== id);
  }
  async function create(options: { name: string; region: string; enableReceiving: boolean }) {
    actionError = null;
    try {
      await creator.create(options);
      if (disposed || !creator.setupDetail) return;
      store.updateDomain(creator.setupDetail, true);
      expand(creator.setupDetail.id, true);
      showCreate = false;
    } catch (error) {
      if (!disposed) actionError = errorMessage(error, "Could not create this domain");
    }
  }
  async function proceed() {
    actionError = null;
    try {
      if (await store.save()) onSaved(store.included);
    } catch (error) {
      if (!disposed) actionError = errorMessage(error, "Could not save domain selection");
    }
  }
</script>

<div class="flex w-full max-w-md flex-col gap-4">
  <Field.Set>
    <Field.Legend>{mode === "setup" ? "Choose your domains" : "Domains & receiving"}</Field.Legend>
    <Field.Description>
      {mode === "setup"
        ? "Choose the domains to configure for this account. You can finish DNS setup later."
        : "Manage this account's domains, DNS records, and receiving settings."}
    </Field.Description>
    <Field.Description>
      Selection does not change DNS or hide email history. Receiving is enabled separately for each domain.
    </Field.Description>
    {#if store.isLoading}
      <div class="flex flex-col gap-2" aria-busy="true">
        {#each [0, 1, 2] as i (i)}<Skeleton class="h-12 w-full" />{/each}
      </div>
    {:else if store.error}
      <p class="text-sm text-destructive" role="alert">{store.error}</p>
      <Button variant="outline" onclick={() => store.load()}>Retry</Button>
      <Field.Description>Check that this connection has permission to access domains.</Field.Description>
    {:else}
      <div class="flex gap-2">
        <Button variant="outline" size="sm" disabled={store.isSaving} onclick={() => store.selectAll()}>Select all</Button>
        <Button variant="ghost" size="sm" disabled={store.isSaving} onclick={() => store.deselectAll()}>Deselect all</Button>
      </div>
      {#if store.domains.length === 0}
        <Field.Description>No domains yet. Add a domain to configure sending and receiving.</Field.Description>
      {/if}
      <Field.Group>
        {#each store.domains as item (item.id)}
          <div class="rounded-md border">
            <Field.Field orientation="horizontal" class="p-3">
              <Checkbox
                id={`include-${accountId}-${item.id}`}
                checked={store.includedIds.includes(item.id)}
                disabled={store.isSaving}
                onCheckedChange={(checked) => store.toggle(item.id, checked)}
              />
              <Field.Content>
                <Field.Label for={`include-${accountId}-${item.id}`}>{item.name}</Field.Label>
                <Field.Description>
                  Sending: {sendingReady(item) ? "ready" : "pending or disabled"} ·
                  Receiving: {item.capabilities?.receiving ?? "unknown"} · DNS: {item.status.replaceAll("_", " ")}
                </Field.Description>
              </Field.Content>
            </Field.Field>
            <Collapsible.Root open={expanded.includes(item.id)} onOpenChange={(open) => expand(item.id, open)}>
              <Collapsible.Trigger class="group flex w-full items-center justify-between px-3 pb-3 text-sm">
                DNS and receiving <ChevronDown class="size-4 transition-transform group-data-[state=open]:rotate-180" />
              </Collapsible.Trigger>
              <Collapsible.Content>
                {#if expanded.includes(item.id)}
                  <DomainDnsPanel
                    {accountId}
                    domainId={item.id}
                    onUpdated={(detail) => store.updateDomain(detail)}
                    onClose={() => expand(item.id, false)}
                    closeLabel={mode === "setup" ? "Configure later" : "Close details"}
                  />
                {/if}
              </Collapsible.Content>
            </Collapsible.Root>
          </div>
        {/each}
      </Field.Group>
      {#if store.missingIds.length > 0}
        <Field.Description>
          {store.missingIds.length} previously selected domain(s) are no longer accessible.
          Their saved selection is kept until you update it.
        </Field.Description>
      {/if}
      {#if store.included.length === 0}
        <Field.Description>
          {mode === "setup"
            ? "No domains selected. You can finish now and configure them in Settings later."
            : "No domains selected. Select domains above to include them in your configuration; email history remains available."}
        </Field.Description>
      {:else}
        <Field.Description>
          {store.included.filter(sendingReady).length} ready for sending ·
          {store.included.filter((item) => !sendingReady(item)).length} pending or disabled.
          {mode === "setup" ? "Pending DNS does not block finishing setup." : "Pending domains can be configured individually above."}
        </Field.Description>
      {/if}
    {/if}
  </Field.Set>
  {#if actionError}<p class="text-sm text-destructive" role="alert">{actionError}</p>{/if}
  {#if showCreate}
    <CreateDomainForm isCreating={creator.isCreating} onBack={() => { showCreate = false; }} onCreate={create} />
  {:else if !store.isLoading && !store.error}
    <Button variant="outline" disabled={store.isSaving} onclick={() => { showCreate = true; }}><Plus /> Add domain</Button>
  {/if}
  <Button disabled={store.isLoading || store.isSaving || !!store.error || creator.isCreating} onclick={proceed}>
    {store.isSaving ? "Saving…" : mode === "setup" ? "Continue" : "Save selection"}
  </Button>
</div>
