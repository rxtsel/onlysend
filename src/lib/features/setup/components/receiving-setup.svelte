<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  const accountId = useAccountId();
  const inboundStatus = getInboundStatus(accountId);
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicInOut } from "svelte/easing";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { errorMessage } from "@/lib/shared/utils/errors";
  import { isAuthError, authErrorToast } from "@/lib/shared/services/auth-toast.svelte";

  import {
    getInboundSetupCache,
    saveInboundSetupCache,
  } from "@/lib/shared/api/domains";
  import {
    getDomain,
    listDomains,
    setDomainReceiving,
    verifyDomain,
    type DomainDetail,
    type DomainSummary,
  } from "@/lib/shared/api/domains";
  import { setInboxEnabled } from "@/lib/shared/api/auth";
  import { toast } from "svelte-sonner";
  import { getInboundStatus } from "@/lib/shared/inbound-status.svelte";
  import DnsRecordsCard from "@/lib/features/setup/components/dns-records-card.svelte";

  import { Button } from "@/lib/components/ui/button";
  import * as Empty from "@/lib/components/ui/empty";
  import {
    Inbox,
    Globe,
    Loader,
    XCircle,
  } from "@lucide/svelte";

  /* ---------------------------------------------------------
   * STATE MACHINE
   *  loading -> no-domain | domain-unverified | receiving-off
   *          -> mx-pending -> ready
   * --------------------------------------------------------- */
  type ReceivingState =
    | "loading"
    | "no-domain"
    | "domain-unverified"
    | "receiving-off"
    | "mx-pending"
    | "ready";

  let {
    /** Render Empty states with a dashed border (standalone contexts). */
    bordered = true,
  }: {
    bordered?: boolean;
  } = $props();

  const emptyCls = $derived(bordered ? "border border-dashed" : "");

  let rxState = $state<ReceivingState>("loading");
  let domain = $state<DomainSummary | null>(null);
  let detail = $state<DomainDetail | null>(null);

  let isEnabling = $state(false);
  let isVerifying = $state(false);

  const POLL_INTERVAL_MS = 5000;
  const POLL_MAX_ATTEMPTS = 60; // ~5 minutes
  let pollTimer: ReturnType<typeof setInterval> | undefined;
  let pollAttempts = 0;
  let disposed = false;

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = undefined;
    }
  }

  /** Keeps the shared readiness signal + persisted flag in sync. */
  function syncReadiness(current: ReceivingState) {
    if (disposed) return;
    inboundStatus.ready = current === "ready";
    if (current !== "loading") {
      setInboxEnabled(accountId, current === "ready").catch(console.error);
    }
  }

  function applyDetail(d: DomainDetail) {
    if (disposed) return;
    detail = d;

    // Persist last-known state so the card renders instantly next time.
    saveInboundSetupCache(accountId, JSON.parse(JSON.stringify(d))).catch(console.error);

    if (d.status !== "verified") {
      rxState = "domain-unverified";
    } else if (d.capabilities.receiving !== "enabled") {
      rxState = "receiving-off";
    } else {
      rxState = allRecordsVerified(d) ? "ready" : "mx-pending";
    }
    syncReadiness(rxState);
  }

  async function loadDetail(id: string) {
    applyDetail(await getDomain(accountId, id));
  }

  function allRecordsVerified(d: DomainDetail): boolean {
    return (
      d.status === "verified" ||
      (d.records.length > 0 && d.records.every((r) => r.status === "verified"))
    );
  }

  function startPollingIfPending() {
    if (disposed || rxState !== "mx-pending") return;

    pollAttempts = 0;
    stopPolling();
    pollTimer = setInterval(async () => {
      pollAttempts += 1;
      if (pollAttempts > POLL_MAX_ATTEMPTS || !detail) {
        stopPolling();
        return;
      }

      try {
        const updated = await getDomain(accountId, detail.id);
        if (disposed) return;
        detail = updated;
        if (allRecordsVerified(detail)) {
          stopPolling();
          rxState = "ready";
          syncReadiness(rxState);
        }
      } catch (err) {
        console.error(err);
      }
    }, POLL_INTERVAL_MS);
  }

  async function handleEnable() {
    if (!detail || isEnabling) return;

    try {
      isEnabling = true;
      const updated = await setDomainReceiving(accountId, detail.id, true);
      if (disposed) return;
      detail = updated;
      toast.success("Receiving enabled. Add the MX record below.");
      rxState = "mx-pending";
      startPollingIfPending();
    } catch (err) {
      console.error(err);
      toast.error(errorMessage(err, "Failed to enable receiving"));
    } finally {
      isEnabling = false;
    }
  }

  async function handleVerify() {
    if (!detail || isVerifying) return;

    try {
      isVerifying = true;
      const updated = await verifyDomain(accountId, detail.id);
      if (disposed) return;
      detail = updated;
      startPollingIfPending();
    } catch (err) {
      console.error(err);
      toast.error(errorMessage(err, "Failed to trigger verification"));
    } finally {
      isVerifying = false;
    }
  }

  onMount(() => {
    (async () => {
      // 1) Instant paint from the local cache (no request).
      try {
        const cached = await getInboundSetupCache(accountId);
        if (disposed) return;
        if (cached) {
          applyDetail(cached as DomainDetail);
        }
      } catch (err) {
        console.error(err);
      }

      // 2) A single fresh fetch to true-up statuses. No continuous polling:
      //    verification checks run only when the user clicks Verify/Enable.
      try {
        if (disposed) return;
        const domains = await listDomains(accountId);
        if (disposed) return;
        const active =
          domains.find((d) => d.status === "verified") ?? domains[0] ?? null;

        if (!active) {
          domain = null;
          rxState = "no-domain";
          syncReadiness(rxState);
          return;
        }

        domain = active;
        await loadDetail(active.id);
      } catch (err) {
        console.error(err);
        if (disposed) return;
        if (isAuthError(err)) {
          authErrorToast(err);
        } else {
          toast.error(errorMessage(err, "Failed to check receiving status"));
        }
        if (!detail) rxState = "no-domain";
      }
    })();

    return () => {
      disposed = true;
      stopPolling();
    };
  });
</script>

<div class="flex h-full items-center justify-center w-full max-w-full">
  {#if rxState === "loading"}
    <Loader class="animate-spin text-muted-foreground" />
  {:else if rxState === "no-domain"}
    <div class="w-full max-w-sm">
      <Empty.Root class={emptyCls}>
        <Empty.Header>
          <Empty.Media variant="icon">
            <Globe />
          </Empty.Media>
          <Empty.Title>No domain yet</Empty.Title>
          <Empty.Description>
            Complete the setup wizard to add a sending domain.
          </Empty.Description>
        </Empty.Header>
      </Empty.Root>
    </div>
  {:else if rxState === "domain-unverified"}
    <div class="w-full max-w-sm">
      <Empty.Root class={emptyCls}>
        <Empty.Header>
          <Empty.Media variant="icon">
            <XCircle />
          </Empty.Media>
          <Empty.Title>{domain?.name} isn't verified yet</Empty.Title>
          <Empty.Description>
            Verify your domain at Resend before enabling receiving. DNS records
            are available in the setup flow or the Resend dashboard.
          </Empty.Description>
        </Empty.Header>
        <Empty.Content>
          <Button
            variant="outline"
            onclick={() =>
              openUrl("https://resend.com/domains").catch(console.error)}
          >
            Open Resend Domains
          </Button>
        </Empty.Content>
      </Empty.Root>
    </div>
  {:else if rxState === "receiving-off"}
    <div class="w-full max-w-sm">
      <Empty.Root class={emptyCls}>
        <Empty.Header>
          <Empty.Media variant="icon">
            <Inbox />
          </Empty.Media>
          <Empty.Title>Receiving is off for {domain?.name}</Empty.Title>
          <Empty.Description>
            Enable receiving to get an MX record, then add it at your DNS
            provider to start getting email into OnlySend.
          </Empty.Description>
        </Empty.Header>
        <Empty.Content>
          <Button onclick={handleEnable} disabled={isEnabling}>
            {isEnabling ? "Enabling..." : "Enable receiving"}
          </Button>
        </Empty.Content>
      </Empty.Root>
    </div>
  {:else if rxState === "mx-pending" && detail}
    <div class="w-full max-w-md" in:fly={{ y: 12, duration: 250, easing: cubicInOut }}>
      <Empty.Root>
        <Empty.Header>
          <Empty.Media variant="icon">
            <Inbox />
          </Empty.Media>
          <Empty.Title>One step left</Empty.Title>
          <Empty.Description>
            Add this record at your DNS provider for
            <span class="font-medium text-foreground">{detail.name}</span>.
            Click a name or value to copy it.
          </Empty.Description>
        </Empty.Header>
        <Empty.Content>
          <DnsRecordsCard
            detail={detail}
            bordered={false}
            isVerifying={isVerifying}
            onVerify={handleVerify}
          />
        </Empty.Content>
      </Empty.Root>
    </div>
  {:else}
    <!-- READY: standard placeholder -->
    <div class="flex flex-col items-center gap-4 text-center">
      <div class="rounded-full bg-muted p-6">
        <Inbox class="h-12 w-12 text-muted-foreground" />
      </div>
      <div class="space-y-2">
        <h2 class="text-xl font-semibold">No email selected</h2>
        <p class="text-sm text-muted-foreground max-w-sm">
          Select an email from the sidebar to view its contents.
        </p>
      </div>
    </div>
  {/if}
</div>
