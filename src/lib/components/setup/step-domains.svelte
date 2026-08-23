<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";

  import {
    SLIDE_IN,
    SLIDE_OUT,
    SLIDE_BACK_IN,
    SLIDE_BACK_OUT,
    createViewAnimator,
    VIEW_WRAP_CLASS,
  } from "./view-transition.svelte";

  import {
    createDomain,
    getDomain,
    listDomains,
    setDomainReceiving,
    verifyDomain,
    type DomainDetail,
    type DomainSummary,
  } from "@/lib/commom/store";
  import { toast } from "svelte-sonner";

  import { Button } from "@/lib/components/ui/button";
  import * as Card from "@/lib/components/ui/card";
  import * as Empty from "@/lib/components/ui/empty";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import * as Item from "@/lib/components/ui/item";
  import { Skeleton } from "@/lib/components/ui/skeleton";
  import { Switch } from "@/lib/components/ui/switch";
  import {
    ArrowLeft,
    CheckCircle2,
    Copy,
    Globe,
    Loader,
    Plus,
    XCircle,
  } from "@lucide/svelte";

  let {
    domain = $bindable(""),
    onContinue,
  }: {
    domain?: string;
    onContinue: () => void;
  } = $props();

  /* ---------------------------------------------------------
   * STATE
   * --------------------------------------------------------- */
  type View = "list" | "setup";

  let view = $state<View>("list");
  let domains = $state<DomainSummary[]>([]);
  let isLoading = $state(true);

  // Setup view state
  let newDomainName = $state("");
  let isCreating = $state(false);
  let setupDetail = $state<DomainDetail | null>(null);
  let isVerifying = $state(false);
  let verified = $state(false);
  let errors = $state<Record<string, string>>({});

  const POLL_INTERVAL_MS = 5000;
  const POLL_MAX_ATTEMPTS = 36; // ~3 minutes

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
   * ANIMATED VIEW SWITCHING (slide + height)
   * --------------------------------------------------------- */
  const animator = createViewAnimator();

  async function switchView(next: View) {
    await animator.transition(next, () => {
      view = next;
    });
  }

  /* ---------------------------------------------------------
   * LIST
   * --------------------------------------------------------- */
  async function loadDomains() {
    isLoading = true;
    try {
      domains = await listDomains();
    } catch (err) {
      console.error(err);
      toast.error("Failed to load domains");
    } finally {
      isLoading = false;
    }
  }

  function select(name: string) {
    domain = name;
  }

  /** Verified domains select directly; unverified ones open their DNS setup. */
  async function handleItemClick(d: DomainSummary) {
    if (d.status === "verified") {
      select(d.name);
      return;
    }

    try {
      const detail = await getDomain(d.id);
      setupDetail = detail;
      verified = allRecordsVerified(detail);
      errors = {};
      await switchView("setup");
    } catch (err) {
      console.error(err);
      toast.error("Failed to load domain details");
    }
  }

  function openSetup() {
    newDomainName = "";
    setupDetail = null;
    verified = false;
    errors = {};
    switchView("setup");
  }

  function backToList() {
    stopPolling();
    switchView("list");
    loadDomains();
  }

  /* ---------------------------------------------------------
   * SETUP: CREATE + DNS RECORDS
   * --------------------------------------------------------- */
  async function handleCreate(e: SubmitEvent) {
    e.preventDefault();
    errors = {};

    const name = newDomainName.trim().toLowerCase();
    if (!name || !name.includes(".")) {
      errors.domainName = "Enter a valid domain (e.g. yourdomain.com)";
      return;
    }

    try {
      isCreating = true;
      setupDetail = await createDomain(name);
    } catch (err) {
      console.error(err);
      toast.error("Failed to create the domain");
    } finally {
      isCreating = false;
    }
  }

  async function copyRecord(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      toast.success("Copied to clipboard");
    } catch (err) {
      console.error(err);
      toast.error("Could not copy");
    }
  }

  /* ---------------------------------------------------------
   * RECEIVING TOGGLE
   * --------------------------------------------------------- */
  let isTogglingReceiving = $state(false);

  async function handleReceivingToggle(checked: boolean) {
    if (!setupDetail || isTogglingReceiving) return;

    try {
      isTogglingReceiving = true;
      setupDetail = await setDomainReceiving(setupDetail.id, checked);
      toast.success(
        checked
          ? "Receiving enabled. Add the MX record below"
          : "Receiving disabled",
      );
    } catch (err) {
      console.error(err);
      toast.error("Failed to update receiving");
    } finally {
      isTogglingReceiving = false;
    }
  }

  /* ---------------------------------------------------------
   * VERIFY + POLLING
   * --------------------------------------------------------- */
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = undefined;
    }
  }

  function allRecordsVerified(detail: DomainDetail): boolean {
    return (
      detail.status === "verified" ||
      (detail.records.length > 0 &&
        detail.records.every((r) => r.status === "verified"))
    );
  }

  async function pollStatus() {
    if (!setupDetail || verified) return stopPolling();

    try {
      const detail = await getDomain(setupDetail.id);
      setupDetail = detail;

      if (allRecordsVerified(detail)) {
        stopPolling();
        verified = true;
        domain = detail.name;
        toast.success(`${detail.name} is verified!`);

        setTimeout(() => {
          backToList();
        }, 1200);
      }
    } catch (err) {
      console.error(err);
    }
  }

  async function handleVerify() {
    if (!setupDetail || isVerifying) return;

    try {
      isVerifying = true;
      setupDetail = await verifyDomain(setupDetail.id);

      let attempts = 0;
      stopPolling();
      pollTimer = setInterval(() => {
        attempts += 1;
        if (attempts > POLL_MAX_ATTEMPTS || verified) {
          stopPolling();
          if (!verified) {
            isVerifying = false;
            toast.error(
              "Verification is taking too long. Check your DNS records and try again.",
            );
          }
          return;
        }
        pollStatus();
      }, POLL_INTERVAL_MS);
    } catch (err) {
      console.error(err);
      toast.error("Failed to trigger verification");
    } finally {
      isVerifying = false;
    }
  }

  onMount(() => {
    loadDomains();
    return () => stopPolling();
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
        {#if isLoading}
          <div class="flex flex-col gap-2" aria-busy="true">
            {#each [0, 1, 2] as i (i)}
              <div class="border rounded-lg px-4 py-3.5 flex items-center gap-3">
                <Skeleton class="size-5 rounded-full shrink-0" />
                <Skeleton class="h-4 w-40" />
                <Skeleton class="h-5 w-16 ml-auto shrink-0" />
              </div>
            {/each}
          </div>
        {:else if domains.length === 0}
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
              <Button onclick={openSetup}>
                <Plus /> Add domain
              </Button>
            </Empty.Content>
          </Empty.Root>
        {:else}
          <Item.Group>
            {#each domains as d (d.id)}
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
            onclick={openSetup}
          >
            <Plus /> Add another domain
          </Button>
        {/if}

        {#if domain}
          <Button type="button" class="w-full mt-4" onclick={onContinue}>
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
        {#if !setupDetail}
          <!-- CREATE FORM -->
          <form onsubmit={handleCreate}>
            <Field.Group>
              <Field.Field>
                <Field.Label for="domainName">Domain</Field.Label>
                <Input
                  id="domainName"
                  bind:value={newDomainName}
                  placeholder="updates.example.com"
                  aria-invalid={!!errors.domainName}
                  autofocus
                />
                {#if errors.domainName}
                  <Field.Error>{errors.domainName}</Field.Error>
                {/if}
                <Field.Description>
                  <span class="font-medium">Tip:</span> Resend recommends using
                  a subdomain (e.g. <code
                    class="font-mono text-[11px] px-1 py-0.5 rounded bg-muted"
                  >
                    updates.example.com</code
                  >) to keep your root domain's email delivery unaffected.
                </Field.Description>
              </Field.Field>

              <div class="flex gap-2">
                <Button
                  type="button"
                  variant="outline"
                  class="flex-1"
                  onclick={backToList}
                >
                  <ArrowLeft /> Back
                </Button>
                <Button type="submit" class="flex-1" disabled={isCreating}>
                  {isCreating ? "Creating..." : "Create"}
                </Button>
              </div>
            </Field.Group>
          </form>
        {:else}
          <!-- DNS RECORDS -->
          <Card.Root class="w-full overflow-hidden">
            <Card.Header>
              <Card.Title class="flex items-center justify-between">
                <span>{setupDetail.name}</span>
                {#if verified}
                  <span
                    class="text-[10px] uppercase tracking-wide font-medium px-2 py-0.5 rounded bg-green-500/10 text-green-600"
                  >
                    Verified
                  </span>
                {/if}
              </Card.Title>
              <Card.Description>
                Add these records to your DNS provider. Click a name or value
                to copy it.
              </Card.Description>
              <Card.Action>
                <Button
                  type="button"
                  size="sm"
                  onclick={handleVerify}
                  disabled={verified || isVerifying}
                >
                  {#if verified}
                    <CheckCircle2 /> Done
                  {:else}
                    Verify domain
                  {/if}
                </Button>
              </Card.Action>
            </Card.Header>

            <Card.Content>
              <div class="flex flex-col gap-2">
                {#each setupDetail.records as record, i (i)}
                  {@const isOk = record.status === "verified"}
                  {@const isFailed = record.status === "failed"}
                  <div class="border rounded-md px-3 py-2 min-w-0">
                    <div class="flex items-center gap-2 mb-1 min-w-0">
                      {#if isOk}
                        <CheckCircle2 class="size-4 text-green-600 shrink-0" />
                      {:else if isFailed}
                        <XCircle class="size-4 text-red-600 shrink-0" />
                      {:else}
                        <Loader
                          class="size-4 animate-spin shrink-0 text-muted-foreground"
                        />
                      {/if}
                      <span
                        class="text-xs font-medium shrink-0"
                        title={`${record.recordType} record`}
                      >
                        {record.recordType}
                      </span>

                      <!-- NAME: click or hover-copy -->
                      <div class="group/name flex items-center gap-x-1 min-w-0 flex-1">
                        <button
                          type="button"
                          class="text-xs text-muted-foreground truncate min-w-0 cursor-pointer hover:bg-muted/60 rounded px-1 -mx-1"
                          title={`Copy ${record.name}`}
                          onclick={() => copyRecord(record.name)}
                        >
                          {record.name}
                        </button>
                        <button
                          type="button"
                          class="shrink-0 opacity-0 group-hover/name:opacity-100 transition-opacity cursor-pointer text-muted-foreground hover:text-foreground"
                          title={`Copy ${record.name}`}
                          tabindex="-1"
                          onclick={() => copyRecord(record.name)}
                        >
                          <Copy class="size-3" />
                        </button>
                      </div>
                    </div>

                    <!-- VALUE: click or hover-copy -->
                    <div class="group/value flex items-center gap-x-1 pl-6 min-w-0">
                      <button
                        type="button"
                        class="font-mono text-[11px] text-muted-foreground truncate min-w-0 cursor-pointer hover:bg-muted/60 rounded px-1 -mx-1"
                        title={`Copy ${record.value}`}
                        onclick={() => copyRecord(record.value)}
                      >
                        {record.value}
                      </button>
                      <button
                        type="button"
                        class="shrink-0 opacity-0 group-hover/value:opacity-100 transition-opacity cursor-pointer text-muted-foreground hover:text-foreground"
                        title={`Copy ${record.value}`}
                        tabindex="-1"
                        onclick={() => copyRecord(record.value)}
                      >
                        <Copy class="size-3" />
                      </button>
                    </div>
                  </div>
                {/each}

                {#if isVerifying && !verified}
                  <p class="text-xs text-muted-foreground text-center pt-1">
                    Checking DNS propagation...
                  </p>
                {/if}

                <p class="text-xs text-muted-foreground border-t pt-3">
                  Optional: add a DMARC TXT record (
                  <code class="font-mono">_dmarc.{setupDetail.name}</code> ) at
                  your provider for anti-spoofing protection.
                </p>

                <!-- RECEIVING -->
                <div class="border rounded-md px-3 py-2.5 flex items-center gap-3 mt-2">
                  <div class="min-w-0 flex-1">
                    <p class="text-sm font-medium">Receiving (inbound)</p>
                    {#if setupDetail.status === "verified"}
                      <p class="text-xs text-muted-foreground">
                        Adds an MX record so this domain can receive emails.
                      </p>
                    {:else}
                      <p class="text-xs text-muted-foreground">
                        Verify the domain first to enable receiving.
                      </p>
                    {/if}
                  </div>
                  <Switch
                    checked={setupDetail.capabilities.receiving === "enabled"}
                    disabled={setupDetail.status !== "verified" || isTogglingReceiving}
                    onCheckedChange={handleReceivingToggle}
                  />
                </div>
              </div>
            </Card.Content>

            <Card.Footer>
              <Button
                type="button"
                variant="outline"
                size="sm"
                class="w-full"
                onclick={backToList}
              >
                <ArrowLeft /> Back to domains
              </Button>
            </Card.Footer>
          </Card.Root>
        {/if}
      </div>
    {/if}
  </div>
</div>
