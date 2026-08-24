<script lang="ts">
  
  import { copyToClipboard } from "@/lib/shared/services/clipboard.svelte";
  import { Button } from "@/lib/components/ui/button";
  import { Switch } from "@/lib/components/ui/switch";
  import type {
    DomainDetail,
  } from "@/lib/shared/store";
  import {
    ArrowLeft,
    CheckCircle2,
    Copy,
    Info,
    Loader,
    XCircle,
  } from "@lucide/svelte";

  let {
    detail,
    isVerifying = false,
    isTogglingReceiving = false,
    bordered = true,
    showReceivingToggle = true,
    onVerify,
    onToggleReceiving,
    onBack,
    onContinue,
  }: {
    detail: DomainDetail;
    isVerifying?: boolean;
    isTogglingReceiving?: boolean;
    /** Standalone contexts render the card inside a dashed frame. */
    bordered?: boolean;
    showReceivingToggle?: boolean;
    onVerify?: () => void;
    onToggleReceiving?: (enabled: boolean) => void;
    onBack?: () => void;
    onContinue?: () => void;
  } = $props();

  const sendingRecords = $derived(
    detail.records.filter((r) => r.group !== "Receiving MX"),
  );
  const receivingRecords = $derived(
    detail.records.filter((r) => r.group === "Receiving MX"),
  );
  const receivingEnabled = $derived(
    detail.capabilities.receiving === "enabled",
  );

  /** Verify only while something is still pending on any listed record. */
  const allGreen = $derived(
    detail.records.length > 0 &&
      detail.records.every((r) => r.status === "verified"),
  );

  async function copyRecord(value: string) {
    await copyToClipboard(value);
  }
</script>

<div class={`w-full min-w-0 ${bordered ? "border rounded-md p-3" : ""}`}>
  <!-- SENDING RECORDS -->
  <div class="flex flex-col gap-2">
    {#each sendingRecords as record, i (i)}
      {@const isOk = record.status === "verified"}
      {@const isFailed = record.status === "failed"}
      <div class="border rounded-md px-3 py-2 min-w-0 text-left">
        <div class="group/name flex items-center gap-2 mb-1 min-w-0">
          {#if isOk}
            <CheckCircle2 class="size-4 text-green-600 shrink-0" />
          {:else if isFailed}
            <XCircle class="size-4 text-red-600 shrink-0" />
          {:else}
            <Loader
              class="size-4 animate-spin shrink-0 text-muted-foreground"
            />
          {/if}
          <span class="text-xs font-medium shrink-0 w-9 text-center">{record.recordType}</span>
          <button
            type="button"
            class="text-xs text-left text-muted-foreground truncate max-w-fit w-full min-w-0 flex-1 cursor-pointer hover:bg-muted/60 rounded px-1 -mx-1"
            title="Name"
            onclick={() => copyRecord(record.name)}
          >
            {record.name}
          </button>
          <button
            type="button"
            class="shrink-0 opacity-0 group-hover/name:opacity-100 transition-opacity cursor-pointer text-muted-foreground hover:text-foreground"
            title="Name"
            tabindex="-1"
            onclick={() => copyRecord(record.name)}
          >
            <Copy class="size-3" />
          </button>
        </div>
        <div class="group/value flex items-center gap-x-1 pl-6 min-w-0">
          <button
            type="button"
            class="font-mono text-[11px] text-muted-foreground truncate min-w-0 cursor-pointer hover:bg-muted/60 rounded px-1 -mx-1"
            title="Content"
            onclick={() => copyRecord(record.value)}
          >
            {record.value}
          </button>
          <button
            type="button"
            class="shrink-0 opacity-0 group-hover/value:opacity-100 transition-opacity cursor-pointer text-muted-foreground hover:text-foreground"
            title="Content"
            tabindex="-1"
            onclick={() => copyRecord(record.value)}
          >
            <Copy class="size-3" />
          </button>
        </div>
        {#if record.ttl || record.priority != null}
          <div class="pl-6 mt-1 text-[10px] text-muted-foreground flex gap-3">
            {#if record.ttl}
              <span>TTL {record.ttl}</span>
            {/if}
            {#if record.priority != null}
              <span>Priority {record.priority}</span>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>

    <!-- RECEIVING SECTION: header + switch always visible; rows once enabled -->
    {#if showReceivingToggle}
      <div class="flex items-center gap-3 my-4 pt-3 border-t">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">Receiving (inbound)</p>
          <p class="text-xs text-muted-foreground">
            {#if detail.status !== "verified"}
              Verify the domain first to enable receiving.
            {:else if !receivingEnabled}
              Off. Turn it on to get email into OnlySend.
            {:else if !allGreen}
              Add the MX record below at your DNS provider.
            {:else}
              Receiving is working.
            {/if}
          </p>
        </div>
        <Switch
          checked={receivingEnabled}
          disabled={isTogglingReceiving ||
            (!receivingEnabled && detail.status !== "verified")}
          onCheckedChange={(checked: boolean) => onToggleReceiving?.(checked)}
        />
      </div>
    {/if}

    {#if receivingEnabled && receivingRecords.length > 0}
      <div class="flex flex-col gap-2 mt-2">
            {#each receivingRecords as record, i (i)}
          {@const isOk = record.status === "verified"}
          {@const isFailed = record.status === "failed"}
          <div class="border rounded-md px-3 py-2 min-w-0 text-left">
            <div class="group/name flex items-center gap-2 mb-1 min-w-0">
              {#if isOk}
                <CheckCircle2 class="size-4 text-green-600 shrink-0" />
              {:else if isFailed}
                <XCircle class="size-4 text-red-600 shrink-0" />
              {:else}
                <Loader
                  class="size-4 animate-spin shrink-0 text-muted-foreground"
                />
              {/if}
              <span class="text-xs font-medium shrink-0 w-9 text-center"
                >{record.recordType}</span
              >
              <button
                type="button"
                class="text-xs text-muted-foreground truncate min-w-0 flex-1 cursor-pointer hover:bg-muted/60 rounded px-1 -mx-1"
                title="Name"
                onclick={() => copyRecord(record.name)}
              >
                {record.name}
              </button>
              <button
                type="button"
                class="shrink-0 opacity-0 group-hover/name:opacity-100 transition-opacity cursor-pointer text-muted-foreground hover:text-foreground"
                title="Name"
                tabindex="-1"
                onclick={() => copyRecord(record.name)}
              >
                <Copy class="size-3" />
              </button>
            </div>
            <div class="group/value flex items-center gap-x-1 pl-6 min-w-0">
              <button
                type="button"
                class="font-mono text-[11px] text-muted-foreground truncate min-w-0 cursor-pointer hover:bg-muted/60 rounded px-1 -mx-1"
                title="Content"
                onclick={() => copyRecord(record.value)}
              >
                {record.value}
              </button>
              <button
                type="button"
                class="shrink-0 opacity-0 group-hover/value:opacity-100 transition-opacity cursor-pointer text-muted-foreground hover:text-foreground"
                title="Content"
                tabindex="-1"
                onclick={() => copyRecord(record.value)}
              >
                <Copy class="size-3" />
              </button>
            </div>
            {#if record.ttl || record.priority != null}
          <div class="pl-6 mt-1 text-[10px] text-muted-foreground flex gap-3">
            {#if record.ttl}
              <span>TTL {record.ttl}</span>
            {/if}
            {#if record.priority != null}
              <span>Priority {record.priority}</span>
            {/if}
          </div>
        {/if}
          </div>
        {/each}
      </div>
    {/if}

  <!-- ACTIONS -->
  {#if onVerify && !allGreen}
    <Button
      type="button"
      class="w-full mt-4"
      onclick={onVerify}
      disabled={isVerifying}
    >
      Verify domain
    </Button>
  {/if}

  {#if onBack && onContinue}
    <div class="grid grid-cols-2 gap-2 mt-2">
      <Button type="button" variant="outline" onclick={onBack}>
        <ArrowLeft /> Back
      </Button>
      <Button
        type="button"
        onclick={onContinue}
        disabled={!allGreen}
        title={allGreen
          ? undefined
          : "Verify all records to continue"}
      >
        Continue
      </Button>
    </div>
    {#if onVerify && !allGreen}
      <p class="text-xs text-muted-foreground text-center mt-1">
        All records must be verified to continue.
      </p>
    {/if}
  {:else if onBack}
    <Button
      type="button"
      variant="outline"
      size="sm"
      class="w-full mt-2"
      onclick={onBack}
    >
      <ArrowLeft /> Back
    </Button>
  {:else if onContinue}
    <Button type="button" class="w-full mt-2" onclick={onContinue}>
      Continue to email options
    </Button>
  {/if}
</div>
