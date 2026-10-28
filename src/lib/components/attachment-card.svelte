<script lang="ts">
  import type { Snippet } from "svelte";
  import { FileImage, FileText, LoaderCircle } from "@lucide/svelte";
  import * as Item from "$lib/components/ui/item";
  import { canPreviewImage, formatAttachmentSize } from "$lib/shared/attachments";

  let { name, contentType, size, src, status, onactivate, actionLabel, actions, disabled = false }: {
    name: string;
    contentType: string;
    size: number | null;
    src?: string;
    status?: string;
    onactivate?: (event: MouseEvent) => void;
    actionLabel?: string;
    actions?: Snippet;
    disabled?: boolean;
  } = $props();
</script>

<Item.Root variant="outline" size="sm" class="relative min-w-0 flex-nowrap gap-3 rounded-lg bg-background p-3 transition-colors has-[>button]:hover:bg-muted/40">
  <Item.Media variant={src ? "image" : "icon"} class="size-10 shrink-0 overflow-hidden rounded-md">
    {#if src}<img {src} alt="" class="size-full object-cover" />
    {:else if status}<LoaderCircle class="size-4 animate-spin motion-reduce:animate-none" aria-hidden="true" />
    {:else if canPreviewImage(contentType)}<FileImage class="size-4" aria-hidden="true" />
    {:else}<FileText class="size-4" aria-hidden="true" />{/if}
  </Item.Media>
  <Item.Content class="min-w-0 gap-0.5">
    <Item.Title class="block truncate text-xs font-medium" title={name}>{name}</Item.Title>
    <Item.Description class="line-clamp-none truncate text-xs">
      {#if status}<span role="status">{status}</span>
      {:else}{formatAttachmentSize(size)}{#if size != null && contentType} · {/if}{contentType || "File"}{/if}
    </Item.Description>
  </Item.Content>
  {#if onactivate}
    <button type="button" class="absolute inset-0 z-10 rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2" aria-label={actionLabel || `Open ${name}`} aria-disabled={disabled || undefined} onclick={(event) => { if (!disabled) onactivate?.(event); }}></button>
  {/if}
  {#if actions}<Item.Actions class="relative z-20 shrink-0 gap-1">{@render actions()}</Item.Actions>{/if}
</Item.Root>
