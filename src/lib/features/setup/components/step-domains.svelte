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
  } from "../view-transition.svelte";

  import {
    createDomain,
    deleteDomain,
    getDomain,
    listDomains,
    setDomainReceiving,
    verifyDomain,
    type DomainDetail,
    type DomainSummary,
  } from "@/lib/shared/api/domains";
  import { setInboxEnabled } from "@/lib/shared/api/auth";
  import { Trash } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import { copyToClipboard } from "@/lib/shared/services/clipboard.svelte";

  import { Button } from "@/lib/components/ui/button";
  import * as Card from "@/lib/components/ui/card";
  import * as Empty from "@/lib/components/ui/empty";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import * as Item from "@/lib/components/ui/item";
  import { Skeleton } from "@/lib/components/ui/skeleton";
  import { Switch } from "@/lib/components/ui/switch";
  import * as Select from "@/lib/components/ui/select";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import DnsRecordsCard from "@/lib/features/setup/components/dns-records-card.svelte";

  const REGIONS = [
    { value: "us-east-1", label: "US East (Virginia)" },
    { value: "eu-west-1", label: "Europe (Ireland)" },
    { value: "sa-east-1", label: "South America (São Paulo)" },
    { value: "ap-northeast-1", label: "Asia Pacific (Tokyo)" },
  ];
  import {
    ArrowLeft,
    CheckCircle2,
    Copy,
    Globe,
    Info,
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
  /** Resend id of the currently selected domain. */
  let selectedId = $state("");
  let isLoading = $state(true);

  // Setup view state
  let newDomainName = $state("");
  let newDomainRegion = $state("us-east-1");
  let newDomainInbox = $state(true);
  let isCreating = $state(false);
  let setupDetail = $state<DomainDetail | null>(null);
  let isVerifying = $state(false);
  let verified = $state(false);
  let isTogglingReceiving = $state(false);

  /** Ready to move on once sending works for this domain. */
  const setupUsable = $derived(
    !!setupDetail &&
      (setupDetail.status === "verified" ||
        setupDetail.capabilities.sending === "enabled"),
  );

  async function handleReceivingToggle(checked: boolean) {
    if (!setupDetail || isTogglingReceiving) return;

    try {
      isTogglingReceiving = true;
      setupDetail = await setDomainReceiving(setupDetail.id, checked);
      toast.success(
        checked
          ? "Receiving enabled. Add the MX record below."
          : "Receiving disabled",
      );
      if (allRecordsVerified(setupDetail)) {
        verified = true;
      }
    } catch (err) {
      console.error(err);
      toast.error("Failed to update receiving");
    } finally {
      isTogglingReceiving = false;
    }
  }

  function continueToEmails() {
    if (!setupDetail) return;
    setInboxEnabled(
      setupDetail.capabilities.receiving === "enabled" &&
        allRecordsVerified(setupDetail),
    ).catch(console.error);
    onContinue();
  }
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

      // Observation point: keep the Inbox visibility flag in sync with
      // what Resend reports for the user's domains.
      const inboxReady = domains.some(
        (d) =>
          d.capabilities?.receiving === "enabled" &&
          ["verified", "partially_verified"].includes(d.status),
      );
      setInboxEnabled(inboxReady).catch(console.error);
    } catch (err) {
      console.error(err);
      toast.error("Failed to load domains");
    } finally {
      isLoading = false;
    }
  }

  function select(name: string, id: string) {
    domain = name;
    selectedId = id;
  }

  /**
   * A domain is selectable once sending works for it — a
   * partially-verified domain usually qualifies even though its global
   * status isn't fully "verified" yet.
   */
  function canSelect(d: DomainSummary): boolean {
    return d.status === "verified" || d.capabilities?.sending === "enabled";
  }

  /** Selectable domains select directly; the rest open their DNS setup. */
  async function handleItemClick(d: DomainSummary) {
    if (canSelect(d)) {
      select(d.name, d.id);
      return;
    }

    try {
      const detail = await getDomain(d.id);
      setupDetail = detail;
      verified = allRecordsVerified(detail);
      errors = {};
      await switchView("setup");
      maybeAutoVerify();
    } catch (err) {
      console.error(err);
      toast.error(`Failed to load domain details. ${err ?? ""}`.trim());
    }
  }

  /**
   * Continue routes through the DNS view whenever there is something
   * pending: unverified records or receiving not enabled yet.
   * Fully-green domains skip straight to email options.
   */
  async function handleContinue() {
    if (!domain) return;

    const summary = domains.find((d) => d.name === domain);
    if (!summary) {
      onContinue();
      return;
    }

    try {
      const detail = await getDomain(summary.id);
      setupDetail = detail;
      verified = allRecordsVerified(detail);
      errors = {};

      const needsDns =
        detail.status !== "verified" ||
        !allRecordsVerified(detail) ||
        detail.capabilities.receiving !== "enabled";

      if (needsDns) {
        await switchView("setup");
        maybeAutoVerify();
      } else {
        onContinue();
      }
    } catch (err) {
      console.error(err);
      toast.error(`Failed to load domain details. ${err ?? ""}`.trim());
    }
  }

  /**
   * Replicates the dashboard refresh: trigger a re-verification cycle
   * once when entering the DNS view with pending records, then let the
   * bounded polling converge the statuses. Skipped for brand-new domains
   * (nothing to verify until the user adds their DNS records).
   */
  function maybeAutoVerify() {
    if (
      !setupDetail ||
      verified ||
      setupDetail.status === "not_started" ||
      allRecordsVerified(setupDetail)
    ) {
      return;
    }
    handleVerify();
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
      const created = await createDomain({
        name,
        region: newDomainRegion,
        enableReceiving: newDomainInbox,
      });
      setupDetail = created;
      toast.success("Domain created. Add the DNS records below.");
    } catch (err) {
      console.error(err);
      toast.error("Failed to create the domain");
    } finally {
      isCreating = false;
    }
  }

  async function copyRecord(value: string) {
    await copyToClipboard(value);
  }
  /* ---------------------------------------------------------
   * DELETE DOMAIN (only while not fully verified)
   * --------------------------------------------------------- */
  let isDeleting = $state(false);

  async function handleDeleteDomain() {
    if (!setupDetail || isDeleting) return;

    try {
      isDeleting = true;
      await deleteDomain(setupDetail.id);
      toast.success(`${setupDetail.name} deleted`);
      setupDetail = null;
      domain = "";
      backToList();
    } catch (err) {
      console.error(err);
      toast.error("Failed to delete the domain");
    } finally {
      isDeleting = false;
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
        setInboxEnabled(true).catch(console.error);

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

              <Field.Field>
                <Field.Label for="region">Region</Field.Label>
                <Select.Root type="single" name="region" bind:value={newDomainRegion}>
                  <Select.Trigger id="region" class="w-full">
                    {REGIONS.find((r) => r.value === newDomainRegion)?.label}
                  </Select.Trigger>
                  <Select.Content>
                    {#each REGIONS as r (r.value)}
                      <Select.Item value={r.value}>{r.label}</Select.Item>
                    {/each}
                  </Select.Content>
                </Select.Root>
                <Field.Description>
                  Where emails will be sent from. Closest to your audience is
                  best.
                </Field.Description>
              </Field.Field>

              <div class="flex items-center gap-3">
                <div class="min-w-0 flex-1">
                  <p class="text-sm font-medium">Inbox (receiving)</p>
                  <p class="text-xs text-muted-foreground">
                    Adds an MX record so this domain can receive email in
                    OnlySend.
                  </p>
                </div>
                <Switch bind:checked={newDomainInbox} />
              </div>

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
          <!-- DNS RECORDS (shared card) -->
          <Card.Root class="w-full overflow-hidden">
            <Card.Header>
              <Card.Title class="flex items-center justify-between gap-2">
                <span class="flex items-center gap-2 min-w-0">
                  <span class="truncate">{setupDetail.name}</span>
                  {#if verified}
                    <span
                      class="text-[10px] uppercase tracking-wide font-medium px-2 py-0.5 rounded bg-green-500/10 text-green-600 shrink-0"
                    >
                      Verified
                    </span>
                  {:else}
                    <span
                      class="text-[10px] uppercase tracking-wide font-medium px-2 py-0.5 rounded shrink-0 {STATUS_STYLES[setupDetail.status] ?? STATUS_STYLES.not_started}"
                    >
                      {statusLabel(setupDetail.status)}
                    </span>
                  {/if}
                </span>
                {#if !verified}
                  {@render confirmDeleteDomain()}
                {/if}
              </Card.Title>
              <Card.Description>
                Add these records at your DNS provider for
                {setupDetail.name}. Click a name or value to copy it.
              </Card.Description>
            </Card.Header>

            <Card.Content>
              <DnsRecordsCard
                detail={setupDetail}
                bordered={false}
                isVerifying={isVerifying}
                isTogglingReceiving={isTogglingReceiving}
                onVerify={handleVerify}
                onToggleReceiving={handleReceivingToggle}
                onBack={backToList}
                onContinue={continueToEmails}
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
        disabled={isDeleting}
      >
        <Trash />
      </Button>
    </AlertDialog.Trigger>
    <AlertDialog.Content>
      <AlertDialog.Header>
        <AlertDialog.Title>
          Delete {setupDetail?.name} from Resend?
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
          onclick={handleDeleteDomain}
        >
          {isDeleting ? "Deleting..." : "Delete domain"}
        </AlertDialog.Action>
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>
{/snippet}
