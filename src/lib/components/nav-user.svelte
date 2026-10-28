<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  const accountId = useAccountId();
  import {
    CheckIcon,
    PlusIcon,
    HeartHandshakeIcon,
    SettingsIcon,
    UnplugIcon,
  } from "@lucide/svelte";
  import AccountAvatar from "./account-avatar.svelte";

  import * as DropdownMenu from "@/lib/components/ui/dropdown-menu/index.js";
  import * as Sidebar from "@/lib/components/ui/sidebar/index.js";
  import { useSidebar } from "@/lib/components/ui/sidebar/index.js";
  import {
    listAccounts,
    type ConnectionMethod,
    type AccountMeta,
  } from "@/lib/shared/api/auth";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { switchMailAccount } from "@/lib/features/auth/account-switch.svelte";
  import { toast } from "svelte-sonner";
  import { openUrl } from "$lib/shared/api/external";
  import { copyToClipboard } from "$lib/shared/services/clipboard.svelte";
  import { ConnectionStore } from "@/lib/features/auth/connection-store.svelte";

  import * as AlertDialog from "@/lib/components/ui/alert-dialog";

  const sidebar = useSidebar();

  const connection = new ConnectionStore(accountId);
  let method = $state<ConnectionMethod>(null);
  let isDisconnecting = $state(false);
  let showDisconnectConfirm = $state(false);
  let accounts = $state<AccountMeta[]>([]);
  let activeAccountId = $state<string | null>(null);
  let accountButton = $state<HTMLButtonElement | null>(null);
  let openingSettings = false;

  const METHOD_LABELS: Record<Exclude<ConnectionMethod, null>, string> = {
    oauth: "Connected via OAuth",
    api_key: "Connected with API key",
  };

  const connectionLabel = $derived(
    method ? METHOD_LABELS[method] : "Not connected",
  );

  async function loadData() {
    try {
      accounts = await listAccounts();
      const storedMethod = accounts.find((account) => account.id === accountId)?.method;
      method = storedMethod === "oauth" || storedMethod === "api_key" ? storedMethod : null;
      activeAccountId = accountId;
    } catch (err) {
      console.error("Error loading accounts:", err);
    }
  }

  async function handleSwitchAccount(accountId: string) {
    if (accountId === activeAccountId) return;
    try {
      await switchMailAccount(accountId, page.url.pathname);
    } catch (err) {
      console.error("Error switching account:", err);
      toast.error("Failed to switch account");
    }
  }

  async function donate() {
    const url = "https://donate.stripe.com/00wdR8dOd0YF7Ipce0a7C04";
    try { await openUrl(url); }
    catch {
      toast.error("Could not open the default browser", {
        action: { label: "Copy link", onClick: () => { void copyToClipboard(url, "Link copied"); } },
      });
    }
  }

  async function handleDisconnect() {
    if (isDisconnecting) return;

    isDisconnecting = true;
    const result = await connection.disconnect();
    isDisconnecting = false;

    if (result.ok && !result.cancelled) {
      toast.success("Disconnected from Resend");
      showDisconnectConfirm = false;
    } else if (!result.ok) {
      toast.error(result.error);
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
            bind:ref={accountButton}
            aria-label="Account"
            tooltipContent="Account"
            class="h-10 p-0 group-data-[collapsible=icon]:p-0! data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
          >
            <AccountAvatar name={accountId} size={32} />
            <span class="group-data-[collapsible=icon]:hidden">Account</span>
          </Sidebar.MenuButton>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        class="w-(--bits-dropdown-menu-anchor-width) min-w-56 rounded-lg"
        side={sidebar.isMobile ? "bottom" : "right"}
        align="end"
        sideOffset={4}
        onCloseAutoFocus={(event) => {
          if (!openingSettings) return;
          openingSettings = false;
          event.preventDefault();
          accountButton?.focus();
          window.dispatchEvent(new CustomEvent("open-settings"));
        }}
      >
        <DropdownMenu.Label class="p-0 font-normal">
          <div class="flex items-center gap-2 px-1 py-1.5 text-start text-sm">
            <AccountAvatar name={accountId} size={32} />
            <div class="grid flex-1 text-start text-sm leading-tight">
              <span class="truncate font-medium">
                {accounts.find((account) => account.id === accountId)?.label ?? "Account"}
              </span>
              <span class="truncate text-xs text-muted-foreground">
                {connectionLabel}
              </span>
            </div>
          </div>
        </DropdownMenu.Label>
        <DropdownMenu.Separator />
        {#if accounts.length > 1}
          <DropdownMenu.Group>
            <DropdownMenu.Label class="text-[10px] uppercase tracking-wide text-muted-foreground px-2">
              Accounts
            </DropdownMenu.Label>
            {#each accounts as account (account.id)}
              <DropdownMenu.Item
                onclick={() => handleSwitchAccount(account.id)}
                class={account.id === activeAccountId ? "bg-accent" : ""}
              >
                <AccountAvatar name={account.id} size={20} />
                <span class="truncate">{account.label}</span>
                {#if account.id === activeAccountId}
                  <CheckIcon class="ms-auto size-3 text-primary" />
                {/if}
              </DropdownMenu.Item>
            {/each}
          </DropdownMenu.Group>
        {/if}
        <DropdownMenu.Group>
          <DropdownMenu.Item onclick={() => goto("/setup")}>
            <PlusIcon />
            Add account
          </DropdownMenu.Item>
          <DropdownMenu.Separator />
          <DropdownMenu.Item onSelect={() => { openingSettings = true; }}>
            <SettingsIcon />
            Settings
          </DropdownMenu.Item>
          <DropdownMenu.Item onclick={donate}>
            <HeartHandshakeIcon />
            Support OnlySend
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
  </Sidebar.MenuItem>
</Sidebar.Menu>
