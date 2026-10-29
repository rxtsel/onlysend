<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { deleteLocalMail } from "$lib/shared/services/delete-mail";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { setContext, untrack } from "svelte";
  import { COMPOSER_STATUS, type ComposerStatus } from "$lib/features/sending/composer-context";
  import { ArrowLeft, Reply, Send, LoaderCircle, Trash2 } from "@lucide/svelte";
  import AppSidebar from "$lib/components/app-sidebar.svelte";
  import MailList from "$lib/components/mail-list.svelte";
  import MailStatusFooter from "$lib/components/mail-status-footer.svelte";
  import { selectArchiveFooter, type ArchiveFooterData, type ArchiveFooterSnapshot } from "$lib/shared/archive-status";
  import type { MailListItem, Mailbox } from "$lib/shared/local-mail";
  import * as Sidebar from "$lib/components/ui/sidebar";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { Button, buttonVariants } from "$lib/components/ui/button";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import { accountSwitch } from "$lib/features/auth/account-switch.svelte";
  import { provideAccount } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import { cn } from "$lib/utils";

  let { children, data } = $props();
  const accountId = $derived(data.accountId);
  provideAccount(() => accountId);
  const mailbox = $derived<Mailbox>(page.url.pathname.startsWith(mailUrl(accountId, "inbox")) ? "inbox" : "sent");
  const isComposer = $derived(page.url.pathname === mailUrl(accountId, "composer"));
  const replyId = $derived(page.url.searchParams.get("reply"));
  const composerKey = $derived(replyId === null ? "composer" : `reply:${replyId}`);
  const composerStatus = $state<ComposerStatus>({ ready: false, sending: false });
  let detailSubject = $state("");
  let deleting = $state(false);
  async function deleteMessage() {
    const owner = accountId;
    const box = mailbox;
    const id = page.params.id;
    if (!id || deleting) return;
    deleting = true;
    try {
      if (await deleteLocalMail(owner, box, id) && owner === accountId && box === mailbox && id === page.params.id) {
        await goto(mailUrl(owner, box));
      }
    } catch {
      toast.error("Could not return to the mail list");
    } finally { deleting = false; }
  }
  let selectedListMail = $state<{ accountId: string; mailbox: Mailbox; mail: MailListItem } | null>(null);
  let detailIsThread = $state<boolean | null>(null);
  setContext(COMPOSER_STATUS, composerStatus);
  setContext("mail-subject", (subject: string) => { detailSubject = subject; });
  setContext("mail-list-preview", () => selectedListMail);
  setContext("mail-thread-status", (threaded: boolean) => { detailIsThread = threaded; });
  $effect.pre(() => {
    const id = page.params.id;
    const owner = accountId;
    const box = mailbox;
    untrack(() => {
      const selection = selectedListMail;
      const matchesSelection = selection?.accountId === owner && selection.mailbox === box && selection.mail.id === id;
      detailSubject = matchesSelection ? selection.mail.subject || "(No subject)" : "";
      detailIsThread = matchesSelection ? (selection.mail.threadCount ?? 1) > 1 : null;
    });
  });
  const isDetail = $derived(Boolean(page.params.id));
  let domain = $state<string | null>(null);
  let domains = $state<string[]>([]);
  $effect.pre(() => {
    const owner = accountId;
    untrack(() => { domain = null; domains = []; });
  });
  let confirmSend = $state(false);
  $effect.pre(() => {
    accountId; isComposer; replyId;
    untrack(() => { confirmSend = false; composerStatus.ready = false; composerStatus.sending = false; });
  });
  let archiveFooter = $state<ArchiveFooterSnapshot | null>(null);
  const footerData = $derived(selectArchiveFooter(archiveFooter, accountId, mailbox));
  function selectMail(mail: MailListItem, selectedMailbox: Mailbox) {
    selectedListMail = { accountId, mailbox: selectedMailbox, mail };
    detailSubject = mail.subject || "(No subject)";
    detailIsThread = (mail.threadCount ?? 1) > 1;
  }
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
        onSelectMail={selectMail}
        onArchiveStatusChange={updateArchiveStatus}
      />
      <main class={cn("min-h-0 min-w-0 flex-1 bg-background", isComposer || isDetail ? "flex flex-col" : "hidden lg:flex lg:flex-col")} aria-label={isComposer ? "Compose message" : "Message preview"}>
        <header class={cn("flex shrink-0 gap-3 border-b px-5", isDetail ? "min-h-16 items-start py-3" : "h-16 items-center")}>
          {#if isComposer}
            {#if replyId !== null}
              <Button href={mailUrl(accountId, "inbox", replyId)} variant="ghost" size="icon" aria-label="Back to original message"><ArrowLeft /></Button>
            {:else}
              <Sidebar.Trigger aria-label="Toggle mailboxes" />
            {/if}
            <h1 class="flex-1 text-base font-semibold tracking-tight">{replyId !== null ? "Reply" : "New message"}</h1>
            <AlertDialog.Root bind:open={confirmSend}>
              <AlertDialog.Trigger disabled={!composerStatus.ready || composerStatus.sending} class={buttonVariants({ variant: "default" })}><Send /> {composerStatus.sending ? "Sending…" : "Send"}</AlertDialog.Trigger>
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
            <h1 class="min-w-0 flex-1 break-words text-base font-semibold leading-tight tracking-tight" aria-busy={isDetail && !detailSubject}>
              {#if isDetail && !detailSubject}
                <span class="sr-only">Loading subject</span><Skeleton aria-hidden="true" class="h-5 w-36" />
              {:else}
                {isDetail ? detailSubject : "Preview"}
              {/if}
            </h1>
            {#if isDetail && detailIsThread === false}
              <div class="flex shrink-0 items-center gap-1 self-start">
                {#if mailbox === "inbox"}
                  <Button href={`${mailUrl(accountId, "composer")}?reply=${encodeURIComponent(page.params.id!)}`} variant="outline" size="sm" aria-label="Reply to this message"><Reply class="size-4" />Reply</Button>
                {/if}
                <Tooltip.Provider delayDuration={150}>
                  <Tooltip.Root>
                    <Tooltip.Trigger>
                      {#snippet child({ props })}
                        <Button {...props} variant="ghost" size="icon-sm" disabled={deleting} onclick={deleteMessage} aria-label="Delete from this device">
                          <Trash2 aria-hidden="true" />
                        </Button>
                      {/snippet}
                    </Tooltip.Trigger>
                    <Tooltip.Content side="bottom">Delete from this device</Tooltip.Content>
                  </Tooltip.Root>
                </Tooltip.Provider>
              </div>
            {/if}
          {/if}
        </header>
        <div class={cn("min-h-0 flex-1", isComposer ? "overflow-y-auto p-5 md:p-8" : "overflow-hidden")}>
          {#key page.params.id ?? (isComposer ? composerKey : mailbox)}
            {@render children()}
          {/key}
        </div>
      </main>
    </div>
  </Sidebar.Provider>
  <MailStatusFooter data={footerData} mailbox={mailbox} />
{/key}
