<script lang="ts">
  import { onDestroy } from "svelte";
  import { X } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { canPreviewImage } from "$lib/shared/attachments";
  import AttachmentCard from "./attachment-card.svelte";
  import AttachmentPreview from "./attachment-preview.svelte";
  let { files = $bindable<File[]>([]) }: { files?: File[] } = $props();
  const urls = new Map<File, string>();
  let images = $state<Map<File, string>>(new Map());
  let selected = $state<File | null>(null);
  let open = $state(false);
  $effect(() => {
    for (const [file, url] of urls) {
      if (!files.includes(file)) { URL.revokeObjectURL(url); urls.delete(file); }
    }
    for (const file of files) {
      if (canPreviewImage(file.type) && file.size <= 5 * 1024 * 1024 && !urls.has(file)) urls.set(file, URL.createObjectURL(file));
    }
    images = new Map(urls);
    if (selected && !files.includes(selected)) open = false;
  });
  onDestroy(() => { for (const url of urls.values()) URL.revokeObjectURL(url); urls.clear(); });
</script>

{#if files.length}
  <section class="max-h-48 space-y-2 overflow-y-auto rounded-b-md border border-t-0 p-3" aria-label="Selected attachments">
    <ul class="grid grid-cols-1 gap-2 lg:grid-cols-2">
      {#each files as file (file)}
        <li class="min-w-0">
          <AttachmentCard name={file.name} contentType={file.type} size={file.size} src={images.get(file)}
            onactivate={images.has(file) ? () => { selected = file; open = true; } : undefined} actionLabel={`Preview ${file.name}`}>
            {#snippet actions()}
              <Button type="button" variant="ghost" size="icon-sm" aria-label={`Remove ${file.name}`} onclick={() => { files = files.filter((item) => item !== file); }}><X class="size-4" /></Button>
            {/snippet}
          </AttachmentCard>
        </li>
      {/each}
    </ul>
    <p class="text-[11px] text-muted-foreground">Up to 25 MB total · Image previews up to 5 MB</p>
  </section>
{/if}
<AttachmentPreview bind:open name={selected?.name || "Image"} src={selected ? images.get(selected) || null : null} />
