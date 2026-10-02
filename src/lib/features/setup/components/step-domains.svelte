<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  const accountId = useAccountId();
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { toast } from "svelte-sonner";

  import {
    SLIDE_IN,
    SLIDE_OUT,
    SLIDE_BACK_IN,
    SLIDE_BACK_OUT,
    createViewAnimator,
    VIEW_WRAP_CLASS,
  } from "../view-transition.svelte";
  import { copyToClipboard } from "@/lib/shared/services/clipboard.svelte";

  import { Button } from "@/lib/components/ui/button";
  import * as Card from "@/lib/components/ui/card";
  import * as Empty from "@/lib/components/ui/empty";
  import * as Item from "@/lib/components/ui/item";
  import { Skeleton } from "@/lib/components/ui/skeleton";
  import DnsRecordsCard from "./dns-records-card.svelte";
  import CreateDomainForm from "@/lib/features/domains/components/create-domain-form.svelte";
  import { DomainSetupStore, canSelect } from "@/lib/features/domains/domain-setup-store.svelte";
  import type { DomainSummary } from "@/lib/shared/api/domains";
  import { setInboxEnabled } from "@/lib/shared/api/auth";
  import {
    ArrowLeft,
    CheckCircle2,
    Globe,
    Plus,
    Trash,
  } from "@lucide/svelte";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let {
    domain = $bindable(""),
    onContinue,
  }: {
    domain?: string;
    onContinue: () => void;
  } = $props();

  const store = new DomainSetupStore(accountId);

  /* ---------------------------------------------------------
   * VIEW SWITCHING (slide + height animation)
   * --------------------------------------------------------- */
  type View = "list" | "setup";

  let view = $state<View>("list");
  const animator = createViewAnimator();

  async function switchView(next: View) {
    await animator.transition(next, () => {
      view = next;
    });
  }

  const STATUS_STYLES: Record<string, string> = {
    verified: "bg-green-500/10 text-green-600",
    pending: "bg-yellow-500/10 text-yellow-600",
    not_started: "bg-muted text-muted-foreground",
    failed: "bg-red-500/10 text-red-600",
    partially_verified: "bg-yellow-500/10 text-yellow-600",
    partially_failed: "bg-yellow-500/10 text-yellow-600",
    temporary_failure: "bg-yellow-500/10 text-yellow-600",
  };

  function statusLabel(status: string): string {
    return status.replaceAll("_", " ");
  }

  /* ---------------------------------------------------------
   * LIST ACTIONS
   * --------------------------------------------------------- */
  function select(d: DomainSummary) {
    store.select(d.name, d.id);
    domain = d.name;
  }

  /** Selectable once sending works — partial domains usually qualify. */
  function canSelectDomain(d: DomainSummary): boolean {
    return canSelect(d);
  }

  /** Selectable domains select directly; the rest open their DNS setup. */
  async function handleItemClick(d: DomainSummary) {
    if (canSelectDomain(d)) {
      select(d);
      return;
    }

    try {
      await store.loadFresh(d.id);
      await switchView("setup");
    } catch (err) {
      console.error(err);
      toast.error(`Failed to load domain details. ${err ?? ""}`.trim());
    }
  }

  /**
   * Continue routes through the DNS view whenever there is something
   * pending. Fully-green domains skip straight to email options.
   */
  async function handleContinue() {
    if (!domain) return;

    const summary = store.domains.find((d) => d.name === domain);
    if (!summary) {
      finishInboxFlag();
      onContinue();
      return;
    }

    const { needsDns } = await store.loadFresh(summary.id);
    if (needsDns) {
      await switchView("setup");
    } else {
      finishInboxFlag();
      onContinue();
    }
  }

  /* ---------------------------------------------------------
   * SETUP VIEW ACTIONS
   * --------------------------------------------------------- */
  function openCreate() {
    store.resetSetup();
    switchView("setup");
  }

  async function backToList() {
    store.stopWork();
    await switchView("list");
    store.load().catch((err) => {
      console.error(err);
      toast.error("Failed to load domains");
    });
  }

  async function handleCreate(options: {
    name: string;
    region: string;
    enableReceiving: boolean;
  }) {
    try {
      await store.create(options);
      toast.success("Domain created. Add the DNS records below.");
    } catch (err) {
      console.error(err);
      toast.error("Failed to create the domain");
    }
  }

  async function handleVerify() {
    try {
      await store.verify();
    } catch (err) {
      console.error(err);
      toast.error("Failed to trigger verification");
    }
  }

  async function handleReceivingToggle(checked: boolean) {
    try {
      await store.toggleReceiving(checked);
      toast.success(
        checked ? "Receiving enabled." : "Receiving disabled.",
      );
    } catch (err) {
      console.error(err);
      toast.error("Failed to update receiving");
    }
  }

  async function handleDeleteDomain() {
    if (!store.setupDetail || store.isDeleting) return;

    const name = store.setupDetail.name;
    try {
      await store.remove(store.setupDetail.id);
      toast.success(`${name} deleted`);
      domain = "";
      backToList();
    } catch (err) {
      console.error(err);
      toast.error("Failed to delete the domain");
    }
  }

  function finishInboxFlag() {
    const detail = store.setupDetail;
    if (!detail) return;
    const enabled =
      detail.capabilities.receiving === "enabled" &&
      detail.records.length > 0 &&
      detail.records.every((r) => r.status === "verified");
    setInboxEnabled(accountId, enabled).catch(console.error);
  }

  onMount(() => {
    store.load().catch((err) => {
      console.error(err);
      toast.error("Failed to load domains");
      store.markLoaded();
    });
  });
</script>

<div class="w-full max-w-sm">
  <div bind:this={animator.element} class={VIEW_WRAP_CLASS}>
    <!-- -----------------------------------------------------
         VIEW: LIST
    ------------------------------------------------------ -->
    {#if view === "list"}
      <div
        class="[grid-area:1/1] min-w-0 w-full"
        data-view="list"
        in:fly={SLIDE_IN}
        out:fly={SLIDE_OUT}
      >
        {#if store.isLoading}
          <div class="flex flex-col gap-2" aria-busy="true">
            {#each [0, 1, 2] as i (i)}
              <div class="border rounded-lg px-4 py-3.5 flex items-center gap-3">
                <Skeleton class="size-5 rounded-full shrink-0" />
                <Skeleton class="h-4 w-40" />
                <Skeleton class="h-5 w-16 ml-auto shrink-0" />
              </div>
            {/each}
          </div>
        {:else if store.domains.length === 0}
          <Empty.Root class="border border-dashed">
            <Empty.Header>
              <Empty.Media variant="icon">
                <Globe />
              </Empty.Media>
              <Empty.Title>No domains yet</Empty.Title>
              <Empty.Description>
                Add the domain you will send emails from.
              </Empty.Description>
            </Empty.Header>
            <Empty.Content>
              <Button onclick={openCreate}>
                <Plus /> Add domain
              </Button>
            </Empty.Content>
          </Empty.Root>
        {:else}
          <Item.Group>
            {#each store.domains as d (d.id)}
              <button
                type="button"
                class="text-left w-full"
                onclick={() => handleItemClick(d)}
              >
                <Item.Root
                  variant="outline"
                  size="sm"
                  class={domain === d.name ? "border-primary ring-1 ring-primary" : ""}
                >
                  <Item.Media>
                    {#if domain === d.name}
                      <CheckCircle2 class="size-5 text-primary" />
                    {:else}
                      <Globe class="size-5 text-muted-foreground" />
                    {/if}
                  </Item.Media>
                  <Item.Content>
                    <Item.Title>{d.name}</Item.Title>
                  </Item.Content>
                  <Item.Actions>
                    <span
                      class="text-[10px] uppercase tracking-wide font-medium px-2 py-0.5 rounded {STATUS_STYLES[d.status] ?? STATUS_STYLES.not_started}"
                    >
                      {statusLabel(d.status)}
                    </span>
                  </Item.Actions>
                </Item.Root>
              </button>
            {/each}
          </Item.Group>

          <Button
            type="button"
            variant="ghost"
            size="sm"
            class="mt-3 w-full"
            onclick={openCreate}
          >
            <Plus /> Add another domain
          </Button>
        {/if}

        {#if domain}
          <Button type="button" class="w-full mt-4" onclick={handleContinue}>
            Continue with {domain}
          </Button>
        {/if}
      </div>
    <!-- -----------------------------------------------------
         VIEW: SETUP (slides in from the right)
    ------------------------------------------------------ -->
    {:else}
      <div
        class="[grid-area:1/1] min-w-0 w-full"
        data-view="setup"
        in:fly={SLIDE_BACK_IN}
        out:fly={SLIDE_BACK_OUT}
      >
        {#if !store.setupDetail}
          <!-- CREATE FORM (extracted component) -->
          <CreateDomainForm
            isCreating={store.isCreating}
            onBack={backToList}
            onCreate={handleCreate}
          />
        {:else}
          <!-- DNS RECORDS (shared card) -->
          <Card.Root class="w-full overflow-hidden">
            <Card.Header>
              <Card.Title class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-2 min-w-0">
                  <span class="truncate">{store.setupDetail.name}</span>
                  {#if store.verified}
                    <span
                      class="text-[10px] uppercase tracking-wide font-medium px-2 py-0.5 rounded bg-green-500/10 text-green-600 shrink-0"
                    >
                      Verified
                    </span>
                  {:else}
                    <span
                      class="text-[10px] uppercase tracking-wide font-medium px-2 py-0.5 rounded shrink-0 {STATUS_STYLES[store.setupDetail.status] ?? STATUS_STYLES.not_started}"
                    >
                      {statusLabel(store.setupDetail.status)}
                    </span>
                  {/if}
                </span>
                {#if !store.verified}
                  {@render confirmDeleteDomain()}
                {/if}
              </Card.Title>
              <Card.Description>
                Add these records at your DNS provider for
                {store.setupDetail.name}. Click a name or value to copy it.
              </Card.Description>
            </Card.Header>

            <Card.Content>
              <DnsRecordsCard
                detail={store.setupDetail}
                bordered={false}
                showReceivingToggle={true}
                isVerifying={store.isVerifying}
                isTogglingReceiving={store.isTogglingReceiving}
                onVerify={() => store.verify().catch(console.error)}
                onToggleReceiving={handleReceivingToggle}
                onBack={backToList}
                onContinue={() => {
                  finishInboxFlag();
                  onContinue();
                }}
              />
            </Card.Content>
          </Card.Root>
        {/if}
      </div>
    {/if}
  </div>
</div>

{#snippet confirmDeleteDomain()}
  <AlertDialog.Root>
    <AlertDialog.Trigger>
      <Button
        type="button"
        variant="destructive"
        size="icon-sm"
        title="Delete domain"
        disabled={store.isDeleting}
      >
        <Trash />
      </Button>
    </AlertDialog.Trigger>
    <AlertDialog.Content>
      <AlertDialog.Header>
        <AlertDialog.Title>
          Delete {store.setupDetail?.name} from Resend?
        </AlertDialog.Title>
        <AlertDialog.Description>
          This permanently removes the domain and all its DNS records from
          your Resend account. This action cannot be undone.
        </AlertDialog.Description>
      </AlertDialog.Header>
      <AlertDialog.Footer>
        <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
        <AlertDialog.Action
          class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
          disabled={store.isDeleting}
          onclick={handleDeleteDomain}
        >
          Deleting...
        </AlertDialog.Action>
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>
{/snippet}
