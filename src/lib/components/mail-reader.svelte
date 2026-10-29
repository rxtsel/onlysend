<script lang="ts">
  import { onMount, onDestroy, getContext } from "svelte";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import { toast } from "svelte-sonner";
  import { LoaderCircle } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import MailAvatar from "./mail-avatar.svelte";
  import MailThreadMessage from "./mail-thread-message.svelte";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { getInboundEmail } from "$lib/shared/inbound";
  import { getSentEmail } from "$lib/shared/sent";
  import { getMailThread, type MailListItem, type MailThreadMessage as ThreadMessage, type Mailbox } from "$lib/shared/local-mail";
  import { formatEmailDate, formatFullEmailDate } from "$lib/shared/utils/dates";

  let { mailbox, emailId }: { mailbox: Mailbox; emailId: string | undefined } = $props();
  const accountId = useAccountId();
  const setMailSubject = getContext<((subject: string) => void) | undefined>("mail-subject");
  const setMailThreadStatus = getContext<((threaded: boolean) => void) | undefined>("mail-thread-status");
  const getListPreview = getContext<(() => { accountId: string; mailbox: Mailbox; mail: MailListItem } | null) | undefined>("mail-list-preview");
  const listSelection = $derived.by(() => {
    const selection = getListPreview?.();
    return selection?.accountId === accountId && selection.mailbox === mailbox && selection.mail.id === emailId ? selection : null;
  });
  const previewMail = $derived(listSelection?.mail ?? null);
  const previewAddress = $derived(previewMail?.from ?? "");
  const previewName = $derived(previewAddress.split("<")[0].replaceAll('"', "").trim() || previewAddress || "Unknown sender");
  const previewIsThread = $derived((previewMail?.threadCount ?? 1) > 1);
  type Message = { subject: string; from: string; to: string[]; createdAt: string; html: string | null; text?: string | null; cc?: string[]; bcc?: string[]; replyTo?: string | string[] | null; attachments?: import("$lib/shared/attachments").EmailAttachment[] };
  let email = $state<Message | null>(null);
  let thread = $state<ThreadMessage[]>([]);
  let loading = $state(true);
  let failed = $state(false);
  let disposed = false;
  let navigatingAfterDelete = false;

  function removeMessage(id: string, box: Mailbox) {
    if (disposed || navigatingAfterDelete) return;
    const selected = id === emailId && box === mailbox;
    if (!selected && !thread.some((item) => item.id === id && item.mailbox === box)) return;
    thread = thread.filter((item) => item.id !== id || item.mailbox !== box);
    setMailThreadStatus?.(thread.length > 1);
    if (selected || !thread.length) {
      navigatingAfterDelete = true;
      void goto(mailUrl(accountId, mailbox)).catch(() => {
        navigatingAfterDelete = false;
        toast.error("Could not return to the mail list");
      });
    }
  }

  async function load() {
    loading = true;
    failed = false;
    if (!listSelection) setMailSubject?.("");
    try {
      if (!emailId) throw new Error("Missing message ID");
      const result = mailbox === "inbox" ? await getInboundEmail(accountId, emailId) : await getSentEmail(accountId, emailId);
      if (!disposed) setMailSubject?.(result.subject || "(No subject)");
      let messages: ThreadMessage[] = [];
      try { messages = await getMailThread(accountId, mailbox, emailId); }
      catch { /* The selected message remains readable if thread lookup fails. */ }
      const selected: ThreadMessage = {
        id: emailId, mailbox, from: result.from, to: result.to, subject: result.subject,
        createdAt: result.createdAt, preview: result.text?.replace(/\s+/g, " ").trim() || "",
      };
      if (!messages.some((message) => message.id === emailId && message.mailbox === mailbox)) messages.push(selected);
      messages.sort((a, b) => Date.parse(a.createdAt) - Date.parse(b.createdAt) || a.id.localeCompare(b.id));
      if (!disposed && !navigatingAfterDelete) {
        email = result;
        thread = messages;
        setMailThreadStatus?.(messages.length > 1);
      }
    } catch {
      if (!disposed) {
        failed = true;
        setMailSubject?.("Message unavailable");
      }
    }
    finally { if (!disposed) loading = false; }
  }
  onMount(() => {
    void load();
    let cleanup: (() => void) | undefined;
    void listen<{ accountId: string; mailbox: Mailbox; emailId: string; deleted: boolean }>("mail-deletion-updated", ({ payload }) => {
      if (disposed || payload.accountId !== accountId || !emailId) return;
      if (payload.deleted) {
        removeMessage(payload.emailId, payload.mailbox);
        return;
      }
      void getMailThread(accountId, mailbox, emailId).then((messages) => {
        if (!disposed && messages.length) {
          thread = messages;
          setMailThreadStatus?.(messages.length > 1);
        }
      }).catch(() => toast.error("Could not refresh the conversation"));
    }).then((unlisten) => { if (disposed) unlisten(); else cleanup = unlisten; }).catch(console.error);
    return () => cleanup?.();
  });
  onDestroy(() => { disposed = true; });
</script>

{#if loading}
  {#if previewMail}
    {#if previewIsThread}
      <article class="border-b" aria-busy="true">
        <div class="flex items-center gap-3 px-5 py-4">
          <MailAvatar address={previewAddress} name={previewName} />
          <span class="min-w-0 flex-1">
            <span class="flex items-baseline justify-between gap-3">
              <span class="truncate text-sm font-semibold">{previewName}</span>
              <time class="shrink-0 whitespace-nowrap text-xs text-muted-foreground" title={formatFullEmailDate(previewMail.createdAt)}>{formatEmailDate(previewMail.createdAt)}</time>
            </span>
            <span class="mt-1 block truncate text-xs text-muted-foreground">To {previewMail.to.join(", ") || "undisclosed recipients"}</span>
            {#if previewMail.preview}<span class="mt-1 block truncate text-xs leading-5 text-muted-foreground">{previewMail.preview}</span>{/if}
          </span>
        </div>
        <div class="flex flex-col gap-2 px-5 pb-5" role="status" aria-label="Loading full message">
          <Skeleton class="h-4 w-full" />
          <Skeleton class="h-4 w-5/6" />
          <Skeleton class="h-4 w-2/3" />
        </div>
      </article>
    {:else}
      <section class="py-5" aria-busy="true">
        <header class="flex items-start gap-3 px-5 pb-3">
          <MailAvatar address={previewAddress} name={previewName} size={40} />
          <div class="min-w-0 flex-1">
            <div class="flex flex-nowrap items-baseline justify-between gap-x-4">
              <span class="min-w-0 flex-1 break-words text-sm font-semibold">{previewName}</span>
              <time class="shrink-0 whitespace-nowrap text-xs text-muted-foreground" title={formatFullEmailDate(previewMail.createdAt)}>{formatEmailDate(previewMail.createdAt)}</time>
            </div>
            <p class="mb-2 text-xs leading-5 text-muted-foreground">To {previewMail.to.join(", ") || "undisclosed recipients"}</p>
          </div>
        </header>
        <div class="px-5 pb-5">
          {#if previewMail.preview}<p class="text-xs leading-5 text-muted-foreground">{previewMail.preview}</p>{/if}
          <div class="mt-5 flex flex-col gap-2" role="status" aria-label="Loading full message">
            <Skeleton class="h-4 w-full" />
            <Skeleton class="h-4 w-5/6" />
            <Skeleton class="h-4 w-2/3" />
          </div>
        </div>
      </section>
    {/if}
  {:else}
    <div class="flex h-full items-center justify-center" role="status" aria-label="Loading message"><LoaderCircle class="size-5 animate-spin text-muted-foreground motion-reduce:animate-none" /></div>
  {/if}
{:else if failed || !email || !emailId}
  <div class="flex h-full flex-col items-center justify-center gap-4 p-6 text-center">
    <p class="text-sm text-muted-foreground">Couldn't open this message.</p>
    <Button variant="outline" onclick={load}>Try again</Button>
  </div>
{:else}
  <article class="flex h-full min-h-0 flex-col">
    <div class="min-h-0 flex-1 overflow-auto overscroll-contain">
      <ol aria-label="Conversation messages" class="divide-y">
        {#each thread as message (message.mailbox + message.id)}
          <li>
            <MailThreadMessage {message} onDeleted={() => removeMessage(message.id, message.mailbox)} selectedId={thread.at(-1)?.id ?? emailId} threaded={thread.length > 1} initialDetail={message.id === emailId && message.mailbox === mailbox ? email : null} />
          </li>
        {/each}
      </ol>
    </div>
  </article>
{/if}
