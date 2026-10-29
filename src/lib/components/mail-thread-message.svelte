<script lang="ts">
  import { onDestroy } from "svelte";
  import { ChevronDown, Reply, Trash2 } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import ConfirmDeleteMail from "./confirm-delete-mail.svelte";
  import { deleteLocalMail } from "$lib/shared/services/delete-mail";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import * as Collapsible from "$lib/components/ui/collapsible";
  import { useAccountId } from "$lib/features/auth/account-context";
  import MailAvatar from "./mail-avatar.svelte";
  import EmailBody from "./email-body.svelte";
  import { getInboundEmail } from "$lib/shared/inbound";
  import { getSentEmail } from "$lib/shared/sent";
  import { prepareEmailHtml } from "$lib/shared/email-html";
  import { formatEmailDate, formatFullEmailDate } from "$lib/shared/utils/dates";
  import type { MailThreadMessage } from "$lib/shared/local-mail";
  import { openUrl } from "$lib/shared/api/external";
  import { copyToClipboard } from "$lib/shared/services/clipboard.svelte";
  import { toast } from "svelte-sonner";
  import { cn } from "$lib/utils";
  import type { EmailAttachment } from "$lib/shared/attachments";

  type Detail = {
    from: string; to: string[]; subject: string; createdAt: string;
    html: string | null; text?: string | null; attachments?: EmailAttachment[];
    cc?: string[]; bcc?: string[]; replyTo?: string | string[] | null;
  };
  let { message, selectedId, threaded, initialDetail = null, previewLoading = false, onDeleted }: {
    message: MailThreadMessage; selectedId: string; threaded: boolean; initialDetail?: Detail | null;
    previewLoading?: boolean;
    onDeleted: () => void;
  } = $props();
  const accountId = useAccountId();
  let open = $state(message.id === selectedId);
  let detail = $state<Detail | null>(initialDetail);
  let loading = $state(false);
  let deleting = $state(false);
  let confirmDelete = $state(false);
  async function deleteMessage() {
    if (deleting) return;
    deleting = true;
    try {
      if (await deleteLocalMail(accountId, message.mailbox, message.id)) onDeleted();
    } finally { deleting = false; }
  }
  let loadError = $state(false);
  let htmlError = $state(false);
  let emailHtml = $state<string | null>(null);
  let preparationId = 0;
  let disposed = false;
  const name = $derived(detail?.from.split("<")[0].replaceAll('"', "").trim() || message.from.split("<")[0].replaceAll('"', "").trim() || message.from || "Unknown sender");

  async function load() {
    loading = true;
    loadError = false;
    try {
      const result = message.mailbox === "inbox"
        ? await getInboundEmail(accountId, message.id)
        : await getSentEmail(accountId, message.id);
      if (!disposed) detail = result;
    } catch {
      if (!disposed) loadError = true;
    } finally { if (!disposed) loading = false; }
  }
  $effect(() => {
    if (!previewLoading && open && !detail && !loading && !loadError) void load();
  });
  $effect(() => {
    const html = detail?.html;
    if (!html) { emailHtml = null; htmlError = false; preparationId++; return; }
    const current = ++preparationId;
    htmlError = false;
    void prepareEmailHtml(html).then((prepared) => {
      if (!disposed && current === preparationId) emailHtml = prepared;
    }).catch(() => {
      if (!disposed && current === preparationId) htmlError = true;
    });
  });
  onDestroy(() => { disposed = true; preparationId++; });

  async function openEmailLink(url: string) {
    try { await openUrl(url); }
    catch { toast.error("Could not open this link in the default browser", { action: { label: "Copy link", onClick: () => { void copyToClipboard(url, "Link copied"); } } }); }
  }

  async function displayInlineImages(images: Record<string, string>) {
    if (disposed || !detail?.html || !Object.keys(images).length) return;
    const current = ++preparationId;
    try {
      const prepared = await prepareEmailHtml(detail.html, images);
      if (!disposed && current === preparationId) emailHtml = prepared;
    } catch {
      if (!disposed && current === preparationId) htmlError = true;
    }
  }
</script>

{#snippet recipientDetails()}
  {#if detail || previewLoading}
    <details class="mb-2 text-xs text-muted-foreground">
      <summary class="cursor-pointer leading-5">To {(detail?.to ?? message.to).join(", ") || "undisclosed recipients"}</summary>
      {#if detail}
      <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 leading-5">
        <dt>From</dt><dd class="break-all text-foreground">{detail.from}</dd>
        <dt>To</dt><dd class="break-all text-foreground">{detail.to.join(", ")}</dd>
        {#if detail.cc?.length}<dt>Cc</dt><dd class="break-all text-foreground">{detail.cc.join(", ")}</dd>{/if}
        {#if detail.bcc?.length}<dt>Bcc</dt><dd class="break-all text-foreground">{detail.bcc.join(", ")}</dd>{/if}
        {#if detail.replyTo?.length}<dt>Reply to</dt><dd class="break-all text-foreground">{Array.isArray(detail.replyTo) ? detail.replyTo.join(", ") : detail.replyTo}</dd>{/if}
        <dt>Date</dt><dd class="text-foreground">{formatFullEmailDate(detail.createdAt)}</dd>
      </dl>
      {/if}
    </details>
  {/if}
{/snippet}

{#snippet bodySkeleton()}
  <div class="py-4 text-sm leading-7" role="status" aria-label="Loading full message">
    <div class="flex h-7 items-center"><Skeleton class="h-3.5 w-full" /></div>
    <div class="flex h-7 items-center"><Skeleton class="h-3.5 w-5/6" /></div>
    <div class="flex h-7 items-center"><Skeleton class="h-3.5 w-2/3" /></div>
  </div>
{/snippet}

{#snippet messageBody()}
  {#if previewLoading || loading}
    <div class="px-5 pb-5">{@render bodySkeleton()}</div>
  {:else if loadError}
    <div class="flex items-center gap-3 px-5 py-6"><p class="text-sm text-muted-foreground">Couldn't load this message.</p><button type="button" class="text-sm font-medium text-primary underline-offset-4 hover:underline" onclick={load}>Try again</button></div>
  {:else if detail}
    <div class="px-5 pb-5">
      {#if emailHtml}
        <EmailBody html={emailHtml} label={`Message from ${name}`} onopenlink={openEmailLink} />
      {:else if detail.text}
        <pre class="whitespace-pre-wrap break-words py-4 font-sans text-sm leading-7">{detail.text}</pre>
      {:else if htmlError}
        <p class="py-6 text-sm text-muted-foreground" role="alert">Couldn't display this message.</p>
      {:else if detail.html}
        {@render bodySkeleton()}
      {:else}
        <p class="py-6 text-sm text-muted-foreground">No content available.</p>
      {/if}
      {#if detail.attachments?.length}
        {#await import("./message-attachments.svelte")}
          <p class="mt-4 text-xs text-muted-foreground" role="status">Loading attachments…</p>
        {:then { default: MessageAttachments }}
          <MessageAttachments context={{ accountId, mailbox: message.mailbox, emailId: message.id }} initialItems={detail.attachments} html={detail.html} oninlineimages={displayInlineImages} />
        {:catch}
          <p class="mt-4 text-xs text-destructive" role="alert">Couldn't load attachment controls. Reopen this message to retry.</p>
        {/await}
      {/if}
      {#if threaded && message.mailbox === "inbox"}
        <div class="mt-4 flex justify-end">
          <Button href={`${mailUrl(accountId, "composer")}?reply=${encodeURIComponent(message.id)}`} variant="outline" size="sm" aria-label={`Reply to message from ${name}`}><Reply class="size-4" />Reply</Button>
        </div>
      {/if}
    </div>
  {/if}
{/snippet}

{#if threaded}
  <Collapsible.Root bind:open class="border-b last:border-b-0">
    <div class="relative">
      <Collapsible.Trigger disabled={previewLoading} class="group flex w-full items-center gap-3 px-5 py-4 pr-12 text-left outline-none transition-colors hover:bg-muted/30 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring">
        <MailAvatar address={message.from} {name} />
        <span class="min-w-0 flex-1">
          <span class="flex items-baseline justify-between gap-3">
            <span class="truncate text-sm font-semibold">{name}</span>
            <time class="shrink-0 whitespace-nowrap text-xs text-muted-foreground" title={formatFullEmailDate(message.createdAt)}>{formatEmailDate(message.createdAt)}</time>
          </span>
          <span class="mt-1 block truncate text-xs text-muted-foreground">To {message.to.join(", ") || "undisclosed recipients"}</span>
          {#if message.preview}<p class="mt-1 line-clamp-1 text-xs leading-5 text-muted-foreground">{message.preview}</p>{/if}
        </span>
        <ChevronDown class={cn("absolute right-5 bottom-4 size-4 text-muted-foreground transition-transform", open && "rotate-180")} aria-hidden="true" />
      </Collapsible.Trigger>
      <Tooltip.Provider delayDuration={150}>
        <Tooltip.Root>
          <Tooltip.Trigger>
            {#snippet child({ props })}
              <Button {...props} variant="ghost" size="icon-xs" class="absolute right-4 top-4" disabled={deleting || previewLoading} onclick={(event) => { event.stopPropagation(); confirmDelete = true; }} aria-label="Delete from this device">
                <Trash2 class="size-4" aria-hidden="true" />
              </Button>
            {/snippet}
          </Tooltip.Trigger>
          <Tooltip.Content side="bottom">Delete from this device</Tooltip.Content>
        </Tooltip.Root>
      </Tooltip.Provider>
    </div>
    <Collapsible.Content>{@render messageBody()}</Collapsible.Content>
  </Collapsible.Root>
{:else}
  <section class="py-5">
    <header class="flex items-start gap-3 px-5 pb-3">
      <MailAvatar address={message.from} {name} size={40} />
      <div class="min-w-0 flex-1">
        <div class="flex flex-nowrap items-baseline justify-between gap-x-4">
          <span class="min-w-0 flex-1 break-words text-sm font-semibold">{name}</span>
          <time class="shrink-0 whitespace-nowrap text-xs text-muted-foreground" title={formatFullEmailDate(message.createdAt)}>{formatEmailDate(message.createdAt)}</time>
        </div>
        {@render recipientDetails()}
      </div>
    </header>
    {@render messageBody()}
  </section>
{/if}
<ConfirmDeleteMail bind:open={confirmDelete} subject={message.subject} onConfirm={() => { void deleteMessage(); }} />
