<script lang="ts">
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { ArrowLeft, Send, LoaderCircle } from "@lucide/svelte";
  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import MailList from "$lib/components/mail-list.svelte";
  import MailStatusFooter from "$lib/components/mail-status-footer.svelte";
  import { selectArchiveFooter, type ArchiveFooterData, type ArchiveFooterSnapshot } from "$lib/shared/archive-status";
  import type { Mailbox } from "$lib/shared/local-mail";
  import * as Sidebar from "$lib/components/ui/sidebar";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { Button, buttonVariants } from "$lib/components/ui/button";
  import { accountSwitch } from "$lib/features/auth/account-switch.svelte";
  import { provideAccount } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import { cn } from "$lib/utils";

  let { children, data } = $props();
  const accountId = $derived(data.accountId);
  provideAccount(() => accountId);
  const mailbox = $derived<Mailbox>(page.url.pathname.startsWith(mailUrl(accountId, "inbox")) ? "inbox" : "sent");
  const isComposer = $derived(page.url.pathname === mailUrl(accountId, "composer"));
  const isDetail = $derived(Boolean(page.params.id));
  let domain = $state<string | null>(null);
  let domains = $state<string[]>([]);
  $effect.pre(() => {
    const owner = accountId;
    untrack(() => { domain = null; domains = []; });
  });
  let confirmSend = $state(false);
  let archiveFooter = $state<ArchiveFooterSnapshot | null>(null);
  const footerData = $derived(selectArchiveFooter(archiveFooter, accountId, mailbox));
  function updateArchiveStatus(owner: string, mailbox: Mailbox, data: ArchiveFooterData) {
    if (owner === accountId) archiveFooter = { accountId: owner, mailbox, data };
  }
</script>

{#if accountSwitch.busy}
  <div class="fixed inset-x-0 top-0 z-50 flex items-center justify-center gap-2 bg-background p-3" role="status" aria-live="polite">
    <LoaderCircle class="size-4 animate-spin motion-reduce:animate-none" aria-hidden="true" /> Switching account…
  </div>
{/if}
{#key accountId}
  <Sidebar.Provider
    style="--sidebar-width: 16.5rem;"
    class="h-[calc(100svh-2rem)] min-h-0 [&_[data-slot=sidebar-container]]:bottom-8 [&_[data-slot=sidebar-container]]:h-[calc(100svh-2rem)]"
  >
    <AppSidebar {mailbox} {domains} bind:domain />
    <div class="flex min-h-0 min-w-0 flex-1">
      <MailList
        {mailbox} {domain} active={!isComposer}
        class={cn("w-full shrink-0 lg:w-[340px] xl:w-[360px]", isComposer ? "hidden" : isDetail ? "hidden lg:flex" : "flex")}
        onDomainsChange={(value) => { domains = value; }}
        onArchiveStatusChange={updateArchiveStatus}
      />
      <main class={cn("min-h-0 min-w-0 flex-1 bg-background", isComposer || isDetail ? "flex flex-col" : "hidden lg:flex lg:flex-col")} aria-label={isComposer ? "Compose message" : "Message preview"}>
        <header class="flex h-16 shrink-0 items-center gap-3 border-b px-5">
          {#if isComposer}
            <Sidebar.Trigger aria-label="Toggle mailboxes" />
            <h1 class="flex-1 text-base font-semibold tracking-tight">New message</h1>
            <AlertDialog.Root bind:open={confirmSend}>
              <AlertDialog.Trigger class={buttonVariants({ variant: "default" })}><Send /> Send</AlertDialog.Trigger>
              <AlertDialog.Content>
                <AlertDialog.Header>
                  <AlertDialog.Title>Send this message?</AlertDialog.Title>
                  <AlertDialog.Description>Once sent, this message cannot be recalled.</AlertDialog.Description>
                </AlertDialog.Header>
                <AlertDialog.Footer>
                  <AlertDialog.Cancel>Keep editing</AlertDialog.Cancel>
                  <AlertDialog.Action form="compose-email-form" type="submit" onclick={() => { confirmSend = false; }}>Send message</AlertDialog.Action>
                </AlertDialog.Footer>
              </AlertDialog.Content>
            </AlertDialog.Root>
          {:else}
            <Button href={mailUrl(accountId, mailbox)} variant="ghost" size="icon" class="lg:hidden" aria-label="Back to messages"><ArrowLeft /></Button>
            <span class="text-sm font-medium text-muted-foreground">{isDetail ? "Message" : "Preview"}</span>
          {/if}
        </header>
        <div class={cn("min-h-0 flex-1", isComposer ? "overflow-y-auto p-5 md:p-8" : "overflow-hidden")}>
          {#key page.params.id ?? (isComposer ? "composer" : mailbox)}
            {@render children()}
          {/key}
        </div>
      </main>
    </div>
  </Sidebar.Provider>
  <MailStatusFooter data={footerData} mailbox={mailbox} />
{/key}
