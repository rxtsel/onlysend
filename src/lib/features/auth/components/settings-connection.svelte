<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  const accountId = useAccountId();
  import { toast } from "svelte-sonner";

  import { Button } from "@/lib/components/ui/button";
  import * as AlertDialog from "@/lib/components/ui/alert-dialog";
  import { logoutMailAccount } from "@/lib/features/auth/account-switch.svelte";
  import { ConnectionStore } from "@/lib/features/auth/connection-store.svelte";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";

  /** Called after a successful disconnect so the shell can close + navigate. */
  let {
    onRequestClose,
  }: {
    onRequestClose?: () => void;
  } = $props();

  // Connection state
  const connectionStore = new ConnectionStore(accountId);
  let isDisconnecting = $state(false);
  let showDisconnectConfirm = $state(false);

  const connection = $derived(connectionStore.status);

  async function loadData() {
    try {
      await connectionStore.load();
    } catch (error) {
      console.error("Error loading connection data:", error);
      toast.error("Failed to load connection");
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

  onMount(() => { void loadData(); });
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
        You can add a separate OAuth connection. This does not replace the current account.
      </p>
      <Button type="button" size="sm" onclick={() => goto("/setup")}>
        Add OAuth connection
      </Button>
    </div>
  {:else}
    <div class="border rounded-md px-4 py-3">
      <p class="text-sm text-muted-foreground mb-3">Not connected.</p>
      <Button type="button" size="sm" onclick={() => goto("/setup")}>
        Add connection
      </Button>
    </div>
  {/if}

  {@render disconnectConfirm()}

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

