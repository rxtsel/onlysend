<script lang="ts">
  import { Editor } from "@/lib/components/editor";
  import Button from "$lib/components/ui/button/button.svelte";
  import Textarea from "$lib/components/ui/textarea/textarea.svelte";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Field from "$lib/components/ui/field";
  import { BookTemplate, ChevronDown, Code, Paperclip, Save } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import { validateAttachmentFiles } from "$lib/shared/attachments";
  import { errorMessage } from "$lib/shared/utils/errors";
  import HtmlEditor from "./html-editor.svelte";
  import EmailTemplates from "./email-templates.svelte";

  let { accountId, content = $bindable(""), subject = $bindable(""), files = $bindable<File[]>([]), isReply = false, disabled = false, "aria-invalid": ariaInvalid }: { accountId: string; content?: string; subject?: string; files?: File[]; isReply?: boolean; disabled?: boolean; "aria-invalid"?: boolean } = $props();
  let imported = $state(false);
  let writer = $state<{ focus: () => boolean; replaceContent?: (html: string) => void } | null>(null);
  let library = $state<{ show: (panel: "library" | "save") => void } | null>(null);
  let picker = $state<HTMLInputElement | null>(null);
  let htmlOpen = $state(false);
  let htmlDraft = $state("");
  let htmlError = $state("");

  export function focus(): boolean { return writer?.focus() ?? false; }

  function openHtml() {
    htmlDraft = content;
    htmlError = "";
    htmlOpen = true;
  }
  function useHtml(html: string) {
    if (imported && writer?.replaceContent) writer.replaceContent(html);
    else { content = html; imported = true; }
  }
  function insertHtml() {
    if (disabled || !htmlDraft.trim()) return;
    if (new TextEncoder().encode(htmlDraft).length > 1024 * 1024) { htmlError = "HTML must be no larger than 1 MB"; return; }
    if (content.trim() && htmlDraft !== content && !window.confirm("Replace the current message design? Recipients, subject and attachments will stay unchanged.")) return;
    useHtml(htmlDraft);
    htmlOpen = false;
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

{#snippet toolbarActions()}
  <div class="flex items-center gap-1">
    {#if imported}
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Attach files" {disabled} onclick={() => picker?.click()}><Paperclip /></Button>
    {/if}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button {...props} type="button" variant="ghost" size="sm" aria-label="HTML options" {disabled}><Code />HTML<ChevronDown /></Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end">
        <DropdownMenu.Group>
          <DropdownMenu.Item {disabled} onSelect={openHtml}><Code />Insert HTML…</DropdownMenu.Item>
          <DropdownMenu.Item {disabled} onSelect={() => library?.show("library")}><BookTemplate />Use template…</DropdownMenu.Item>
        </DropdownMenu.Group>
        <DropdownMenu.Separator />
        <DropdownMenu.Group>
          <DropdownMenu.Item disabled={disabled || !content.trim()} onSelect={() => library?.show("save")}><Save />Save as template…</DropdownMenu.Item>
        </DropdownMenu.Group>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  </div>
{/snippet}

<div class="h-full min-h-0">
  {#if imported}
    <HtmlEditor bind:this={writer} bind:content {toolbarActions} {disabled} aria-invalid={ariaInvalid} />
  {:else}
    <Editor bind:this={writer} bind:content bind:files {toolbarActions} {disabled} aria-invalid={ariaInvalid} />
  {/if}
</div>
<input bind:this={picker} type="file" multiple class="hidden" aria-label="Attach files" {disabled} onchange={attach} />
<EmailTemplates bind:this={library} {accountId} {content} {subject} {disabled} onapply={(template) => {
  useHtml(template.html);
  if (!isReply) subject = template.subject;
}} />
<Dialog.Root bind:open={htmlOpen}>
  <Dialog.Content class="sm:max-w-3xl">
    <Dialog.Header>
      <Dialog.Title>Insert HTML</Dialog.Title>
      <Dialog.Description>Paste a complete email design. It will appear in the message editor, where you can edit its text and links. Recipients, subject and attachments stay unchanged.</Dialog.Description>
    </Dialog.Header>
    <Field.Group>
      <Field.Field data-invalid={!!htmlError}>
        <Field.Label for="email-html-source">Email HTML</Field.Label>
        <Textarea id="email-html-source" bind:value={htmlDraft} aria-invalid={!!htmlError} aria-label="Email HTML source" {disabled} spellcheck="false" placeholder="Paste your email HTML here…" class="h-72 resize-y font-mono text-xs [field-sizing:fixed]" />
        {#if htmlError}<Field.Error>{htmlError}</Field.Error>{/if}
      </Field.Field>
    </Field.Group>
    <Dialog.Footer>
      <Button type="button" variant="outline" onclick={() => htmlOpen = false}>Cancel</Button>
      <Button type="button" disabled={disabled || !htmlDraft.trim()} onclick={insertHtml}>Apply HTML</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
