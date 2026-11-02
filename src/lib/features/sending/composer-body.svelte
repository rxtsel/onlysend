<script lang="ts">
  import { Editor } from "@/lib/components/editor";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import Button from "$lib/components/ui/button/button.svelte";
  import { Paperclip } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import { validateAttachmentFiles } from "$lib/shared/attachments";
  import { errorMessage } from "$lib/shared/utils/errors";
  import HtmlEditor from "./html-editor.svelte";
  import EmailTemplates from "./email-templates.svelte";

  let { accountId, content = $bindable(""), subject = $bindable(""), files = $bindable<File[]>([]), isReply = false, disabled = false, "aria-invalid": ariaInvalid }: { accountId: string; content?: string; subject?: string; files?: File[]; isReply?: boolean; disabled?: boolean; "aria-invalid"?: boolean } = $props();
  let mode = $state("rich");
  let templateRevision = $state(0);
  let writer = $state<{ focus: () => boolean } | null>(null);
  let picker = $state<HTMLInputElement | null>(null);

  export function focus(): boolean { return writer?.focus() ?? false; }
  function switchMode(next: string) {
    if (!next || next === mode || disabled) return;
    if (next === "rich" && content.trim() && !window.confirm("Switch to rich text? Unsupported HTML, styles and layout will be removed. Stay in Custom HTML to preserve your design.")) return;
    mode = next;
  }
  function attach(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    try {
      const next = [...files, ...Array.from(input.files ?? [])];
      validateAttachmentFiles(next);
      files = next;
    } catch (cause) { toast.error(errorMessage(cause, "Could not attach files")); }
    finally { input.value = ""; }
  }
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
  <div class="flex shrink-0 flex-wrap items-center justify-between gap-2">
    <ToggleGroup.Root type="single" value={mode} onValueChange={switchMode} variant="outline" size="sm" aria-label="Email content format" {disabled}>
      <ToggleGroup.Item value="rich">Rich text</ToggleGroup.Item>
      <ToggleGroup.Item value="html">Custom HTML</ToggleGroup.Item>
    </ToggleGroup.Root>
    <EmailTemplates {accountId} {content} {subject} {disabled} onapply={(template) => {
      content = template.html;
      if (!isReply) subject = template.subject;
      mode = "html";
      templateRevision++;
    }} />
  </div>
  <div class="min-h-0 flex-1">
    {#if mode === "rich"}
      <Editor bind:this={writer} bind:content bind:files {disabled} aria-invalid={ariaInvalid} />
    {:else}
      {#key templateRevision}
        <HtmlEditor bind:this={writer} bind:content {disabled} aria-invalid={ariaInvalid} />
      {/key}
    {/if}
  </div>
  {#if mode === "html"}
    <div class="shrink-0"><Button type="button" variant="ghost" size="sm" {disabled} onclick={() => picker?.click()}><Paperclip />Attach files</Button><input bind:this={picker} type="file" multiple class="hidden" aria-label="Attach files" {disabled} onchange={attach} /></div>
  {/if}
</div>
