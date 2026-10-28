<script lang="ts">
  import type { Snippet } from "svelte";
  import * as Dialog from "$lib/components/ui/dialog";
  let { open = $bindable(false), name, src, actions, restoreFocus, onclose }: {
    open?: boolean;
    name: string;
    src: string | null;
    actions?: Snippet;
    restoreFocus?: HTMLElement | null;
    onclose?: () => void;
  } = $props();
  let failed = $state(false);
  $effect(() => { src; failed = false; });
</script>

<Dialog.Root bind:open onOpenChangeComplete={(isOpen) => { if (!isOpen) onclose?.(); }}>
  <Dialog.Content class="sm:max-w-3xl" onCloseAutoFocus={(event) => {
    if (restoreFocus?.isConnected) { event.preventDefault(); restoreFocus.focus(); }
  }}>
    <Dialog.Header>
      <Dialog.Title class="truncate pr-6" title={name}>{name}</Dialog.Title>
      <Dialog.Description class="sr-only">Image attachment preview</Dialog.Description>
    </Dialog.Header>
    {#if failed}<p class="py-8 text-center text-sm text-muted-foreground" role="status">This image could not be displayed. You can still save the file.</p>
    {:else if src}<img {src} alt={name} onerror={() => { failed = true; }} class="mx-auto max-h-[65svh] max-w-full rounded-md object-contain" />{/if}
    {#if actions}<Dialog.Footer>{@render actions()}</Dialog.Footer>{/if}
  </Dialog.Content>
</Dialog.Root>
