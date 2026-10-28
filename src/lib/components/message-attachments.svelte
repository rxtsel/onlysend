<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { Download, ExternalLink, X } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import AttachmentCard from "./attachment-card.svelte";
  import AttachmentPreview from "./attachment-preview.svelte";
  import {
    attachmentAction, cancelAttachmentDownload, listEmailAttachments, canPreviewImage,
    contentId, inlineImageIds, primaryAttachmentAction, attachmentStatus, MAX_ATTACHMENT_BYTES,
    type AttachmentContext, type EmailAttachment, type AttachmentAction, type DownloadProgress,
  } from "$lib/shared/attachments";
  import { errorMessage } from "$lib/shared/utils/errors";
  import { toast } from "svelte-sonner";

  let { context, initialItems = [], html, oninlineimages }: {
    context: AttachmentContext;
    initialItems?: EmailAttachment[];
    html?: string | null;
    oninlineimages: (images: Record<string, string>) => Promise<void>;
  } = $props();
  let items = $state<EmailAttachment[]>([]);
  let listing = $state(false);
  let error = $state<string | null>(null);
  let embedded = $state<Record<string, string>>({});
  let pending = $state<Set<string>>(new Set());
  let busy = $state<({ attachmentId: string; requestId: string; action: AttachmentAction } & DownloadProgress) | null>(null);
  let preview = $state<{ attachment: EmailAttachment; src: string } | null>(null);
  let previewOpen = $state(false);
  let previewTrigger = $state<HTMLElement | null>(null);
  let disposed = false;
  const requests = new Set<string>();
  const visible = $derived(items.filter((item) => !item.contentId || !(pending.has(contentId(item.contentId)) || Object.hasOwn(embedded, contentId(item.contentId)))));

  async function load() {
    if (listing) return;
    listing = true;
    error = null;
    let referenced = new Set<string>();
    if (html) referenced = inlineImageIds(html);
    pending = new Set(referenced);
    try {
      const result = await listEmailAttachments(context);
      if (disposed) return;
      items = result;
    } catch (err) {
      if (!disposed) error = errorMessage(err, "Could not refresh attachments. Your downloaded metadata is still available.");
    }
    if (disposed) return;
    const images: Record<string, string> = Object.assign(Object.create(null), embedded);
    let total = Object.values(images).reduce((sum, value) => sum + value.length * 0.75, 0);
    for (const item of items) {
      const cid = item.contentId ? contentId(item.contentId) : null;
      if (!cid || !referenced.has(cid) || Object.hasOwn(images, cid) || !canPreviewImage(item.contentType)) continue;
      if (disposed) return;
      const requestId = crypto.randomUUID();
      requests.add(requestId);
      try {
        const result = await attachmentAction({ ...context, attachmentId: item.id, requestId, action: "preview" }, () => {});
        if (disposed) return;
        if (result.kind === "preview") {
          total += result.data.length * 0.75;
          if (total > MAX_ATTACHMENT_BYTES) throw new Error("Inline images exceed 25 MB. Save the remaining files individually.");
          images[cid] = result.data;
        }
      } catch {
        if (!disposed) error = "Some inline images could not be loaded. You can retry or save them individually.";
      } finally { requests.delete(requestId); }
    }
    if (!disposed) {
      try {
        await oninlineimages(images);
        if (!disposed) embedded = images;
      } catch { if (!disposed) error = "Could not display inline images. Try again."; }
      if (!disposed) { pending = new Set(); listing = false; }
    }
  }

  async function run(item: EmailAttachment, action: AttachmentAction, trigger?: HTMLElement) {
    if (busy) return;
    const requestId = crypto.randomUUID();
    requests.add(requestId);
    busy = { attachmentId: item.id, requestId, action, downloaded: 0, total: item.size };
    try {
      const result = await attachmentAction({ ...context, attachmentId: item.id, requestId, action }, (progress) => {
        if (!disposed && busy?.requestId === requestId) busy = { ...busy, ...progress };
      });
      if (disposed) return;
      if (result.kind === "preview") { previewTrigger = trigger || null; preview = { attachment: item, src: result.data }; previewOpen = true; }
      else if (result.kind === "saved") toast.success("Attachment saved");
    } catch (err) {
      if (!disposed) toast.error(errorMessage(err, "Could not download attachment"));
    } finally {
      requests.delete(requestId);
      if (!disposed) {
        busy = null;
        if (action === "save" && trigger) { await tick(); if (!disposed && trigger.isConnected) trigger.focus(); }
      }
    }
  }
  function releasePreview() {
    preview = null;
    previewTrigger = null;
  }
  async function cancel() {
    if (!busy) return;
    try { await cancelAttachmentDownload(context.accountId, busy.requestId); }
    catch { toast.error("Could not cancel download. Try again."); }
  }
  onMount(() => { items = initialItems; void load(); });
  onDestroy(() => {
    disposed = true;
    releasePreview();
    for (const id of requests) void cancelAttachmentDownload(context.accountId, id).catch(() => {});
  });
</script>

{#if visible.length || error}
  <section class="@container/attachments mt-4 shrink-0 space-y-2 px-0 pt-4" aria-label="Attachments">
    {#if visible.length}
      <ul class="grid grid-cols-1 gap-2 @xl/attachments:grid-cols-2">
        {#each visible as item (item.id)}
          {@const name = item.filename || "Attachment"}
          {@const current = busy?.attachmentId === item.id ? busy : null}
          {@const finishing = current?.total != null && current.downloaded >= current.total}
          <li class="min-w-0">
            <AttachmentCard {name} contentType={item.contentType} size={item.size} disabled={!!busy}
              status={current ? attachmentStatus(current.action, current) : undefined}
              onactivate={(event) => run(item, primaryAttachmentAction(item), event.currentTarget as HTMLElement)}
              actionLabel={primaryAttachmentAction(item) === "preview" ? `Preview ${name}` : primaryAttachmentAction(item) === "open" ? `Open ${name} in the default application` : `Save ${name} as`}>
              {#snippet actions()}
                <Button type="button" variant="ghost" size="icon-sm" aria-label={`Save ${name} as`} disabled={!!busy} onclick={(event) => run(item, "save", event.currentTarget)}><Download class="size-4" /></Button>
                {#if current}<Button type="button" variant="ghost" size="icon-sm" aria-label={`Cancel download of ${name}`} disabled={finishing} onclick={cancel}><X class="size-4" /></Button>{/if}
              {/snippet}
            </AttachmentCard>
          </li>
        {/each}
      </ul>
    {/if}
    {#if error}<div class="flex items-center gap-2 text-xs text-muted-foreground" role="status"><span>{error}</span><Button type="button" variant="ghost" size="sm" disabled={listing} onclick={load}>Retry</Button></div>{/if}
  </section>
{/if}
<AttachmentPreview bind:open={previewOpen} name={preview?.attachment.filename || "Image"} src={preview?.src || null} restoreFocus={previewTrigger} onclose={releasePreview}>
  {#snippet actions()}
    {#if preview}
      <Button type="button" variant="outline" disabled={!!busy} onclick={() => preview && run(preview.attachment, "open")}><ExternalLink class="size-4" />Open externally</Button>
      <Button type="button" disabled={!!busy} onclick={() => preview && run(preview.attachment, "save")}><Download class="size-4" />Save as…</Button>
    {/if}
  {/snippet}
</AttachmentPreview>
