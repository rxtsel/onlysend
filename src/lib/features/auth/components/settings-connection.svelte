<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  const accountId = useAccountId();
  const inboundStatus = getInboundStatus(accountId);
  import { toast } from "svelte-sonner";

  import { Button } from "@/lib/components/ui/button";
  import * as AlertDialog from "@/lib/components/ui/alert-dialog";
  import ReceivingSetup from "@/lib/features/setup/components/receiving-setup.svelte";
  import { getInboundStatus } from "@/lib/shared/inbound-status.svelte";
  import { logoutMailAccount } from "@/lib/features/auth/account-switch.svelte";
  import { ConnectionStore } from "@/lib/features/auth/connection-store.svelte";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";

  /** Called after a successful disconnect so the shell can close + navigate. */
  let {
    onRequestClose,
  }: {
    onRequestClose?: () => void;
  } = $props();

  // Connection state
  const connectionStore = new ConnectionStore(accountId);
  let isConnecting = $state(false);
  let isDisconnecting = $state(false);
  let inboxEnabled = $state(false);
  let showInboxSetup = $state(false);
  let showDisconnectConfirm = $state(false);

  const connection = $derived(connectionStore.status);

  // The embedded setup flow flips the shared readiness signal.
  $effect(() => {
    if (inboundStatus.ready && !inboxEnabled) {
      inboxEnabled = true;
      showInboxSetup = false;
    }
  });

  async function loadData() {
    try {
      await connectionStore.load();
      inboxEnabled = connectionStore.inboxEnabled;
    } catch (error) {
      console.error("Error loading connection data:", error);
      toast.error("Failed to load connection");
    }
  }

  async function handleConnectResend(e: Event) {
    e.preventDefault();
    const result = await connectionStore.connect();
    if (!result.ok) toast.error(result.error);
  }

  async function refreshConnection() {
    await loadData();
    isConnecting = false;

    if (connection.method === "oauth") {
      toast.success("Connected with Resend");
    }
  }

  async function handleDisconnect() {
    if (isDisconnecting) return;
    try {
      isDisconnecting = true;
      if (!await logoutMailAccount(accountId)) return;
      toast.success("Disconnected from Resend");
      showDisconnectConfirm = false;
      onRequestClose?.();
    } catch (error) {
      console.error("Error disconnecting:", error);
      toast.error("Failed to disconnect");
    } finally {
      isDisconnecting = false;
    }
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;

    loadData();

    listen<{ success: boolean; warning?: string; error?: string }>(
      "oauth://done",
      (event) => {
        if (event.payload.success) {
          refreshConnection();
        } else {
          isConnecting = false;
          toast.error(event.payload.error ?? "Failed to connect with Resend");
        }
      },
    ).then((fn) => (unlisten = fn));

    return () => unlisten?.();
  });
</script>

<div class="w-full max-w-md">
  <h3 class="text-sm font-medium mb-2">Resend account</h3>
  {#if connection.method === "oauth"}
    <div class="flex items-center justify-between border rounded-md px-4 py-3">
      <div>
        <p class="text-sm">Connected with Resend</p>
        <p class="text-xs text-muted-foreground">
          Authorized via OAuth. Tokens refresh automatically.
        </p>
      </div>
      <Button
        type="button"
        variant="destructive"
        size="sm"
        disabled={isDisconnecting}
        onclick={() => (showDisconnectConfirm = true)}
      >
        {isDisconnecting ? "Disconnecting..." : "Disconnect"}
      </Button>
    </div>
  {:else if connection.method === "api_key"}
    <div class="border rounded-md px-4 py-3">
      <p class="text-sm">Connected with an API key</p>
      <p class="text-xs text-muted-foreground mb-3">
        You can also connect your account securely via OAuth.
      </p>
      <Button
        type="button"
        size="sm"
        disabled={isConnecting}
        onclick={handleConnectResend}
      >
        {isConnecting ? "Waiting for authorization..." : "Connect with Resend"}
      </Button>
    </div>
  {:else}
    <div class="border rounded-md px-4 py-3">
      <p class="text-sm text-muted-foreground mb-3">Not connected.</p>
      <Button
        type="button"
        size="sm"
        disabled={isConnecting}
        onclick={handleConnectResend}
      >
        {isConnecting ? "Waiting for authorization..." : "Connect with Resend"}
      </Button>
    </div>
  {/if}

  <!-- INBOX SETUP -->
  {#if connection.method}
    <h3 class="text-sm font-medium mt-6 mb-2">Inbox</h3>
    {#if inboxEnabled && !showInboxSetup}
      <div class="border rounded-md px-4 py-3 flex items-center justify-between">
        <p class="text-sm">Inbox enabled for your active domain.</p>
      </div>
    {:else}
      {#if !showInboxSetup}
        <div class="border rounded-md px-4 py-3">
          <p class="text-sm text-muted-foreground mb-3">
            Receiving is not configured yet. Set it up to read incoming email
            in OnlySend.
          </p>
          <Button type="button" size="sm" onclick={() => (showInboxSetup = true)}>
            Set up inbox
          </Button>
        </div>
      {:else}
        <div
          class="border rounded-md p-4 max-h-80 overflow-y-auto overflow-x-hidden min-w-0 w-full"
        >
          <ReceivingSetup bordered={false} />
        </div>
      {/if}
    {/if}
  {/if}

  {#snippet disconnectConfirm()}
    <AlertDialog.Root bind:open={showDisconnectConfirm}>
      <AlertDialog.Content>
        <AlertDialog.Header>
          <AlertDialog.Title>Disconnect from Resend?</AlertDialog.Title>
          <AlertDialog.Description>
            This account and its saved credentials will be removed from OnlySend.
            Other connected accounts and your domains and emails in Resend will not be deleted.
          </AlertDialog.Description>
        </AlertDialog.Header>
        <AlertDialog.Footer>
          <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
          <AlertDialog.Action
            class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            disabled={isDisconnecting}
            onclick={handleDisconnect}
          >
            {isDisconnecting ? "Disconnecting..." : "Disconnect"}
          </AlertDialog.Action>
        </AlertDialog.Footer>
      </AlertDialog.Content>
    </AlertDialog.Root>
  {/snippet}
</div>

