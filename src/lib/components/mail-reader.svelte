<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { LoaderCircle, Paperclip } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import MailAvatar from "./mail-avatar.svelte";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { getInboundEmail, type InboundAttachment } from "$lib/shared/inbound";
  import { getSentEmail } from "$lib/shared/sent";
  import { formatEmailDate, formatFullEmailDate } from "$lib/shared/utils/dates";
  import type { Mailbox } from "$lib/shared/local-mail";

  let { mailbox, emailId }: { mailbox: Mailbox; emailId: string | undefined } = $props();
  const accountId = useAccountId();
  type Message = { subject: string; from: string; to: string[]; createdAt: string; html: string | null; text?: string | null; cc?: string[]; replyTo?: string | null; attachments?: InboundAttachment[] };
  let email = $state<Message | null>(null);
  let loading = $state(true);
  let failed = $state(false);
  let disposed = false;
  const name = $derived(email?.from.split("<")[0].replaceAll('"', "").trim() || email?.from || "Unknown sender");

  async function load() {
    loading = true;
    failed = false;
    try {
      if (!emailId) throw new Error("Missing message ID");
      const result = mailbox === "inbox" ? await getInboundEmail(accountId, emailId) : await getSentEmail(accountId, emailId);
      if (!disposed) email = result;
    } catch { if (!disposed) failed = true; }
    finally { if (!disposed) loading = false; }
  }
  onMount(() => { void load(); });
  onDestroy(() => { disposed = true; });

  function sizeLabel(bytes: number | null): string {
    if (bytes == null) return "";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

{#if loading}
  <div class="flex h-full items-center justify-center" role="status" aria-label="Loading message"><LoaderCircle class="size-5 animate-spin text-muted-foreground motion-reduce:animate-none" /></div>
{:else if failed || !email}
  <div class="flex h-full flex-col items-center justify-center gap-4 p-6 text-center">
    <p class="text-sm text-muted-foreground">Couldn't open this message.</p>
    <Button variant="outline" onclick={load}>Try again</Button>
  </div>
{:else}
  <article class="flex h-full min-h-0 flex-col">
    <header class="max-h-[45%] shrink-0 overflow-y-auto border-b px-6 py-7 lg:px-8">
      <h1 class="break-words text-2xl font-semibold tracking-tight">{email.subject || "(No subject)"}</h1>
      <div class="mt-6 flex items-start gap-3">
        <MailAvatar address={email.from} {name} size={40} />
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
            <span class="break-words text-sm font-semibold">{name}</span>
            <time class="text-xs text-muted-foreground" title={formatFullEmailDate(email.createdAt)}>{formatEmailDate(email.createdAt)}</time>
          </div>
          <details class="mt-1 text-xs text-muted-foreground">
            <summary class="cursor-pointer break-words leading-5">To {email.to.join(", ") || "undisclosed recipients"}</summary>
            <dl class="mt-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-2 leading-5">
              <dt>From</dt><dd class="break-all text-foreground">{email.from}</dd>
              <dt>To</dt><dd class="break-all text-foreground">{email.to.join(", ")}</dd>
              {#if email.cc?.length}<dt>CC</dt><dd class="break-all text-foreground">{email.cc.join(", ")}</dd>{/if}
              {#if email.replyTo}<dt>Reply to</dt><dd class="break-all text-foreground">{email.replyTo}</dd>{/if}
              <dt>Date</dt><dd class="text-foreground">{formatFullEmailDate(email.createdAt)}</dd>
            </dl>
          </details>
        </div>
      </div>
      {#if email.attachments?.length}
        <ul class="mt-5 flex flex-wrap gap-2" aria-label="Attachments">
          {#each email.attachments as attachment (attachment.id)}
            <li class="flex max-w-full items-center gap-2 rounded-lg border px-3 py-2 text-xs" title={attachment.contentType}>
              <Paperclip class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
              <span class="truncate">{attachment.filename || "Attachment"}</span>
              <span class="shrink-0 text-muted-foreground">{sizeLabel(attachment.size)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
      {#if email.html}
        <!-- Mail is untrusted, never grant scripts or same-origin access. -->
        <iframe title="Email content" sandbox="" referrerpolicy="no-referrer" loading="lazy" style:color-scheme="light" class="h-full min-h-80 w-full border-0 bg-white" srcdoc={email.html}></iframe>
      {:else if email.text}
        <pre class="whitespace-pre-wrap break-words p-6 font-sans text-sm leading-7 lg:p-8">{email.text}</pre>
      {:else}
        <p class="p-8 text-sm text-muted-foreground">No content available.</p>
      {/if}
    </div>
  </article>
{/if}
