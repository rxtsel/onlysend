<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  const accountId = useAccountId();
  import { toast } from "svelte-sonner";

  import { Button, buttonVariants } from "@/lib/components/ui/button";
  import * as Field from "$lib/components/ui/field";
  import * as Item from "$lib/components/ui/item";
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

<section class="w-full" aria-label="Resend connection">
  <Field.Group>
    <h3 class="mb-2 text-base font-medium">Resend account</h3>
    <Item.Root variant="outline" size="sm">
      <Item.Content class="min-w-48">
        <Item.Title>
          {connection.method === "oauth" ? "Connected with Resend" : connection.method === "api_key" ? "Connected with an API key" : "Not connected"}
        </Item.Title>
        {#if connection.method === "oauth"}
          <Item.Description class="line-clamp-none">Authorized via OAuth. Tokens refresh automatically.</Item.Description>
        {:else if connection.method === "api_key"}
          <Item.Description class="line-clamp-none">You can add a separate OAuth connection. This does not replace the current account.</Item.Description>
        {/if}
      </Item.Content>
      <Item.Actions>
        {#if connection.method === "oauth"}
          <Button type="button" variant="destructive" disabled={isDisconnecting} onclick={() => (showDisconnectConfirm = true)}>
            {isDisconnecting ? "Disconnecting..." : "Disconnect"}
          </Button>
        {:else if connection.method === "api_key"}
          <Button type="button" onclick={() => goto("/setup")}>Add OAuth connection</Button>
        {:else}
          <Button type="button" onclick={() => goto("/setup")}>Add connection</Button>
        {/if}
      </Item.Actions>
    </Item.Root>
  </Field.Group>

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
            class={buttonVariants({ variant: "destructive" })}
            disabled={isDisconnecting}
            onclick={handleDisconnect}
          >
            {isDisconnecting ? "Disconnecting..." : "Disconnect"}
          </AlertDialog.Action>
        </AlertDialog.Footer>
      </AlertDialog.Content>
    </AlertDialog.Root>
  {/snippet}
</section>

