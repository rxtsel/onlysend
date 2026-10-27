<script lang="ts">
  import { Inbox, Send, SquarePen, Bug, Globe } from "@lucide/svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { toast } from "svelte-sonner";
  import * as Sidebar from "$lib/components/ui/sidebar";
  import { Button } from "$lib/components/ui/button";
  import { useSidebar } from "$lib/components/ui/sidebar";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import type { Mailbox } from "$lib/shared/local-mail";
  import { errorMessage } from "$lib/shared/utils/errors";
  import SettingsDialog from "./settings-dialog.svelte";
  import NavUser from "./nav-user.svelte";

  let { mailbox, domains, domain = $bindable(null) }: {
    mailbox: Mailbox;
    domains: string[];
    domain?: string | null;
  } = $props();
  const accountId = useAccountId();
  const sidebar = useSidebar();
  const composing = $derived(page.url.pathname === mailUrl(accountId, "composer"));
  const folders = [
    { label: "Inbox", value: "inbox" as const, icon: Inbox },
    { label: "Sent", value: "sent" as const, icon: Send },
  ];

  function closeMobile() {
    if (sidebar.isMobile) sidebar.setOpenMobile(false);
  }

  async function feedback() {
    try { await openUrl("https://github.com/rxtsel/onlysend/issues/new/choose"); }
    catch (error) { toast.error(errorMessage(error, "Could not open GitHub")); }
  }
</script>

<Sidebar.Root collapsible="offcanvas">
  <div class="flex h-full min-h-0 w-full">
  <aside class="flex w-14 shrink-0 flex-col items-center border-e bg-sidebar py-4" aria-label="Account and settings">
    <a href={mailUrl(accountId, "inbox")} aria-label="OnlySend" class="flex size-8 items-center justify-center rounded-lg text-foreground focus-visible:outline focus-visible:outline-ring" onclick={closeMobile}>
      <Send class="size-5" aria-hidden="true" />
    </a>
    <div class="mt-auto flex w-8 flex-col items-center gap-3">
      <Button size="icon" variant="ghost" onclick={feedback} aria-label="Feedback / report a bug" title="Feedback / report a bug"><Bug /></Button>
      <SettingsDialog />
      <NavUser />
    </div>
  </aside>
  <nav class="flex min-w-0 flex-1 flex-col" aria-label="Mailboxes">
    <Sidebar.Header class="h-16 shrink-0 justify-center px-4 py-0">
      <Button href={mailUrl(accountId, "composer")} class="h-10 w-full justify-center gap-2 rounded-full" onclick={closeMobile} aria-current={composing ? "page" : undefined}>
        <SquarePen data-icon="inline-start" /> New message
      </Button>
    </Sidebar.Header>
    <Sidebar.Content class="pt-4">
      <Sidebar.Group class="px-3">
        <Sidebar.GroupLabel>Mailboxes</Sidebar.GroupLabel>
        <Sidebar.GroupContent>
          <Sidebar.Menu>
            {#each folders as folder (folder.value)}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton isActive={mailbox === folder.value && domain === null && !composing} class="h-10 px-3">
                  {#snippet child({ props })}
                    <a {...props} href={mailUrl(accountId, folder.value)} aria-current={mailbox === folder.value && domain === null && !composing ? "page" : undefined} onclick={() => { domain = null; closeMobile(); }}>
                      <folder.icon /><span>{folder.label}</span>
                    </a>
                  {/snippet}
                </Sidebar.MenuButton>
              </Sidebar.MenuItem>
            {/each}
          </Sidebar.Menu>
        </Sidebar.GroupContent>
      </Sidebar.Group>
      {#if domains.length > 0}
        <Sidebar.Group class="px-3">
          <Sidebar.GroupLabel>Domains</Sidebar.GroupLabel>
          <Sidebar.GroupContent>
            <Sidebar.Menu>
              {#each domains as name (name)}
                <Sidebar.MenuItem>
                  <Sidebar.MenuButton isActive={domain === name} class="h-10 px-3" onclick={() => { domain = name; closeMobile(); void goto(mailUrl(accountId, mailbox)); }}>
                    <Globe /><span title={name}>{name}</span>
                  </Sidebar.MenuButton>
                </Sidebar.MenuItem>
              {/each}
            </Sidebar.Menu>
          </Sidebar.GroupContent>
        </Sidebar.Group>
      {/if}
    </Sidebar.Content>
  </nav>
  </div>
</Sidebar.Root>
