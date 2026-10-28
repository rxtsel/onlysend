<script lang="ts">
  import { Inbox, Send, SquarePen, Bug, Globe } from "@lucide/svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { openUrl } from "$lib/shared/api/external";
  import { toast } from "svelte-sonner";
  import * as Sidebar from "$lib/components/ui/sidebar";
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

<Sidebar.Root collapsible="icon">
  <Sidebar.Header class="h-16 shrink-0 justify-center px-4 py-0 group-data-[collapsible=icon]:px-2">
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <Sidebar.MenuButton variant="primary" tooltipContent="New message" class="h-10 justify-center rounded-full">
          {#snippet child({ props })}
            <a {...props} href={mailUrl(accountId, "composer")} aria-label="New message" onclick={closeMobile} aria-current={composing ? "page" : undefined}>
              <SquarePen /><span class="group-data-[collapsible=icon]:hidden">New message</span>
            </a>
          {/snippet}
        </Sidebar.MenuButton>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
  </Sidebar.Header>
  <Sidebar.Content class="overflow-x-hidden pt-4">
    <nav aria-label="Mailboxes">
      <Sidebar.Group class="px-3 group-data-[collapsible=icon]:px-2">
        <Sidebar.GroupLabel>Mailboxes</Sidebar.GroupLabel>
        <Sidebar.GroupContent>
          <Sidebar.Menu>
            {#each folders as folder (folder.value)}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton isActive={mailbox === folder.value && domain === null && !composing} tooltipContent={folder.label} class="h-10 px-3">
                  {#snippet child({ props })}
                    <a {...props} href={mailUrl(accountId, folder.value)} aria-label={folder.label} aria-current={mailbox === folder.value && domain === null && !composing ? "page" : undefined} onclick={() => { domain = null; closeMobile(); }}>
                      <folder.icon /><span class="group-data-[collapsible=icon]:hidden">{folder.label}</span>
                    </a>
                  {/snippet}
                </Sidebar.MenuButton>
              </Sidebar.MenuItem>
            {/each}
          </Sidebar.Menu>
        </Sidebar.GroupContent>
      </Sidebar.Group>
      {#if domains.length > 0}
        <Sidebar.Group class="px-3 group-data-[collapsible=icon]:px-2">
          <Sidebar.GroupLabel>Domains</Sidebar.GroupLabel>
          <Sidebar.GroupContent>
            <Sidebar.Menu>
              {#each domains as name (name)}
                <Sidebar.MenuItem>
                  <Sidebar.MenuButton isActive={domain === name} tooltipContent={name} aria-label={name} class="h-10 px-3" onclick={() => { domain = name; closeMobile(); void goto(mailUrl(accountId, mailbox)); }}>
                    <Globe /><span class="group-data-[collapsible=icon]:hidden" title={name}>{name}</span>
                  </Sidebar.MenuButton>
                </Sidebar.MenuItem>
              {/each}
            </Sidebar.Menu>
          </Sidebar.GroupContent>
        </Sidebar.Group>
      {/if}
    </nav>
  </Sidebar.Content>
  <Sidebar.Footer class="gap-1 p-3 group-data-[collapsible=icon]:p-2" role="group" aria-label="Account and settings">
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <Sidebar.MenuButton class="h-10 gap-4" onclick={feedback} aria-label="Feedback / report a bug" tooltipContent="Feedback / report a bug">
          <Bug /><span class="group-data-[collapsible=icon]:hidden">Feedback &amp; bugs</span>
        </Sidebar.MenuButton>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
    <NavUser />
  </Sidebar.Footer>
</Sidebar.Root>

<SettingsDialog />
