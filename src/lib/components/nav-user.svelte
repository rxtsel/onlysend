<script lang="ts">
  import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
  import SparklesIcon from "@lucide/svelte/icons/sparkles";
  import UnplugIcon from "@lucide/svelte/icons/unplug";
  import { Blobatar } from "@blobatar/svelte";

  import * as DropdownMenu from "@/lib/components/ui/dropdown-menu/index.js";
  import * as Sidebar from "@/lib/components/ui/sidebar/index.js";
  import { useSidebar } from "@/lib/components/ui/sidebar/index.js";
  import {
    disconnectResend,
    getConnectionStatus,
    getActiveDomain,
    type ConnectionMethod,
  } from "../shared/store";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { openUrl } from "@tauri-apps/plugin-opener";

  import * as AlertDialog from "@/lib/components/ui/alert-dialog";

  const sidebar = useSidebar();

  let activeDomain = $state<string | null>(null);
  let method = $state<ConnectionMethod>(null);
  let isDisconnecting = $state(false);
  let showDisconnectConfirm = $state(false);

  const METHOD_LABELS: Record<Exclude<ConnectionMethod, null>, string> = {
    oauth: "Connected via OAuth",
    api_key: "Connected with API key",
  };

  const connectionLabel = $derived(
    method ? METHOD_LABELS[method] : "Not connected",
  );

  async function loadData() {
    try {
      const [domain, status] = await Promise.all([
        getActiveDomain(),
        getConnectionStatus(),
      ]);
      activeDomain = domain;
      method = status.method;
    } catch (err) {
      console.error(err);
    }
  }

  async function handleDisconnect() {
    if (isDisconnecting) return;

    try {
      isDisconnecting = true;
      await disconnectResend();
      toast.success("Disconnected from Resend");
      showDisconnectConfirm = false;
      goto("/");
    } catch (err) {
      console.error(err);
      toast.error("Failed to disconnect");
    } finally {
      isDisconnecting = false;
    }
  }

  onMount(() => {
    loadData();
  });
</script>

<Sidebar.Menu>
  <Sidebar.MenuItem>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Sidebar.MenuButton
            {...props}
            size="lg"
            class="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground md:h-8 md:p-0"
          >
            <Blobatar
              name={activeDomain ?? "onlysend"}
              size={32}
              class="rounded-lg"
            />
            <div class="grid flex-1 text-start text-sm leading-tight">
              <span class="truncate font-medium">
                {activeDomain ?? "OnlySend"}
              </span>
              <span class="truncate text-xs text-muted-foreground">
                {connectionLabel}
              </span>
            </div>
            <ChevronsUpDownIcon class="ms-auto size-4" />
          </Sidebar.MenuButton>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        class="w-(--bits-dropdown-menu-anchor-width) min-w-56 rounded-lg"
        side={sidebar.isMobile ? "bottom" : "right"}
        align="end"
        sideOffset={4}
      >
        <DropdownMenu.Label class="p-0 font-normal">
          <div class="flex items-center gap-2 px-1 py-1.5 text-start text-sm">
            <Blobatar
              name={activeDomain ?? "onlysend"}
              size={32}
              class="rounded-lg"
            />
            <div class="grid flex-1 text-start text-sm leading-tight">
              <span class="truncate font-medium">
                {activeDomain ?? "OnlySend"}
              </span>
              <span class="truncate text-xs text-muted-foreground">
                {connectionLabel}
              </span>
            </div>
          </div>
        </DropdownMenu.Label>
        <DropdownMenu.Separator />
        <DropdownMenu.Group>
          <DropdownMenu.Item onclick={() => openUrl("https://donate.stripe.com/00wdR8dOd0YF7Ipce0a7C04")}>
            <SparklesIcon />
            Support us
          </DropdownMenu.Item>
          {#if method}
            <DropdownMenu.Item variant="destructive" onclick={() => (showDisconnectConfirm = true)}>
              <UnplugIcon />
              Disconnect
            </DropdownMenu.Item>
          {/if}
        </DropdownMenu.Group>
      </DropdownMenu.Content>
    </DropdownMenu.Root>

    <!-- DISCONNECT CONFIRM -->
    <AlertDialog.Root bind:open={showDisconnectConfirm}>
      <AlertDialog.Content>
        <AlertDialog.Header>
          <AlertDialog.Title>Disconnect from Resend?</AlertDialog.Title>
          <AlertDialog.Description>
            Your OAuth grant will be revoked and any stored API key removed.
            You'll be signed out and returned to setup. Your local data
            (identities, history) stays and revives when you reconnect.
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
  </Sidebar.MenuItem>
</Sidebar.Menu>
