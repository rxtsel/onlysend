<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { LoaderCircle } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import MailAvatar from "./mail-avatar.svelte";
  import MessageAttachments from "./message-attachments.svelte";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { getInboundEmail, type InboundAttachment } from "$lib/shared/inbound";
  import { getSentEmail } from "$lib/shared/sent";
  import { formatEmailDate, formatFullEmailDate } from "$lib/shared/utils/dates";
  import type { Mailbox } from "$lib/shared/local-mail";
  import { prepareEmailDocument, emailLinkFromMessage, type EmailDocument } from "$lib/shared/email-document";
  import { openUrl } from "$lib/shared/api/external";
  import { copyToClipboard } from "$lib/shared/services/clipboard.svelte";
  import { toast } from "svelte-sonner";

  let { mailbox, emailId }: { mailbox: Mailbox; emailId: string | undefined } = $props();
  const accountId = useAccountId();
  type Message = { subject: string; from: string; to: string[]; createdAt: string; html: string | null; text?: string | null; cc?: string[]; replyTo?: string | null; attachments?: InboundAttachment[] };
  let email = $state<Message | null>(null);
  let document = $state<EmailDocument | null>(null);
  let frame = $state<HTMLIFrameElement | null>(null);
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
      const prepared = result.html ? await prepareEmailDocument(result.html) : null;
      if (!disposed) { email = result; document = prepared; }
    } catch { if (!disposed) failed = true; }
    finally { if (!disposed) loading = false; }
  }
  async function openEmailLink(event: MessageEvent) {
    const url = emailLinkFromMessage(event, frame?.contentWindow ?? null, document?.token);
    if (!url) return;
    try { await openUrl(url); }
    catch {
      toast.error("Could not open this link in the default browser", {
        action: { label: "Copy link", onClick: () => { void copyToClipboard(url, "Link copied"); } },
      });
    }
  }
  onMount(() => {
    void load();
    window.addEventListener("message", openEmailLink);
    return () => window.removeEventListener("message", openEmailLink);
  });
  onDestroy(() => { disposed = true; });

  async function displayInlineImages(images: Record<string, string>) {
    if (disposed || !email?.html || !Object.keys(images).length) return;
    const prepared = await prepareEmailDocument(email.html, images);
    if (!disposed) document = prepared;
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
      {#if emailId && (mailbox === "sent" || email.attachments?.length)}
        <MessageAttachments context={{ accountId, mailbox, emailId }} initialItems={email.attachments || []} html={email.html} oninlineimages={displayInlineImages} />
      {/if}
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
      {#if document}
        <!-- No same-origin: CSP authorizes only the app's link bridge, not email scripts. -->
        <iframe bind:this={frame} title="Email content" sandbox="allow-scripts" referrerpolicy="no-referrer" loading="lazy" style:color-scheme="light" class="h-full min-h-80 w-full border-0 bg-white" srcdoc={document.html}></iframe>
      {:else if email.text}
        <pre class="whitespace-pre-wrap break-words p-6 font-sans text-sm leading-7 lg:p-8">{email.text}</pre>
      {:else}
        <p class="p-8 text-sm text-muted-foreground">No content available.</p>
      {/if}
    </div>
  </article>
{/if}
