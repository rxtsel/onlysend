<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Editor } from "@tiptap/core";
  import StarterKit from "@tiptap/starter-kit";
  import Placeholder from "@tiptap/extension-placeholder";
  import {
    Link as LinkIcon,
    Italic as ItalicIcon,
    List as ListIcon,
    ListOrdered as ListOrderedIcon,
    Quote,
    Eraser,
    ClipboardPaste,
    Undo2,
    Redo2,
  } from "@lucide/svelte";
  import Button from "../ui/button/button.svelte";
  import * as Popover from "$lib/components/ui/popover";

  import { cn } from "@/lib/utils";
  import Input from "../ui/input/input.svelte";
  import { toast } from "svelte-sonner";

  let element = $state<HTMLDivElement | null>(null);
  let editor = $state<Editor | null>(null);

  let {
    content = $bindable(""),
    placeholder = "Write your email...",
    editable = true,
    disabled = false,
    onchange,
    "aria-invalid": ariaInvalid,
  } = $props();

  export function getText(): string {
    return editor?.getText() ?? "";
  }

  export function focus(): boolean {
    const writer = element?.querySelector<HTMLElement>('[contenteditable="true"]');
    if (!writer) return false;
    writer.focus();
    return document.activeElement === writer;
  }

  $effect(() => { editor?.setEditable(editable && !disabled, false); });

  // States reactive for buttons
  let isBold = $state(false);
  let isItalic = $state(false);
  let isBulletList = $state(false);
  let isOrderedList = $state(false);
  let isLink = $state(false);
  let isBlockquote = $state(false);
  let canUndo = $state(false);
  let canRedo = $state(false);
  let pasting = $state(false);

  // Popover link
  let linkPopoverOpen = $state(false);
  let linkUrl = $state("");

  function updateActiveStates() {
    if (!editor) return;
    isBold = editor.isActive("bold");
    isItalic = editor.isActive("italic");
    isBulletList = editor.isActive("bulletList");
    isOrderedList = editor.isActive("orderedList");
    isLink = editor.isActive("link");
    isBlockquote = editor.isActive("blockquote");
    canUndo = editor.can().undo();
    canRedo = editor.can().redo();
  }

  async function pastePlainText() {
    const current = editor;
    if (!current || disabled || !editable || pasting) return;
    pasting = true;
    try {
      const text = await navigator.clipboard.readText();
      if (!text || current.isDestroyed || disabled || !editable) return;
      const paragraphs = text.split(/\r\n|\r|\n/).map((line) => ({
        type: "paragraph",
        content: line ? [{ type: "text", text: line }] : [],
      }));
      current.chain().focus().insertContent(paragraphs).run();
    } catch {
      if (!current.isDestroyed) toast.error("Clipboard access is unavailable. Use Ctrl+Shift+V or ⌘+Shift+V to paste as plain text.");
    } finally { pasting = false; }
  }

  function handleLinkPopoverChange(open: boolean) {
    if (open) linkUrl = editor?.getAttributes("link").href || "";
  }

  function handleSetLink() {
    if (!linkUrl) {
      // If the URL is empty, remove the link
      editor?.chain().focus().extendMarkRange("link").unsetLink().run();
    } else {
      // Add or update the link
      editor
        ?.chain()
        .focus()
        .extendMarkRange("link")
        .setLink({ href: linkUrl })
        .run();
    }

    linkPopoverOpen = false;
    linkUrl = "";
  }

  function handleRemoveLink() {
    editor?.chain().focus().unsetLink().run();
    linkPopoverOpen = false;
    linkUrl = "";
  }

  onMount(() => {
    if (!element) return;

    const inst = new Editor({
      element,
      extensions: [
        StarterKit.configure({
          link: {
            openOnClick: false,
            HTMLAttributes: { class: "text-blue-400 underline cursor-pointer" },
          },
        }),
        Placeholder.configure({ placeholder }),
      ],
      content,
      editable,
      editorProps: {
        attributes: {
          class:
            "prose prose-neutral dark:prose-invert prose-sm max-w-none min-h-full w-full sm:prose-base px-2.5 py-3 focus:outline-none",
        },
      },
      onTransaction: () => {
        updateActiveStates();
      },
      onUpdate: ({ editor }) => {
        content = editor.getHTML();
        onchange?.();
      },
    });

    editor = inst;
    updateActiveStates();
  });

  onDestroy(() => {
    editor?.destroy();
  });
</script>

<div
  inert={disabled}
  aria-disabled={disabled || undefined}
  class={cn("flex h-full min-h-0 flex-col", {
    "ring-1 ring-destructive": ariaInvalid,
    "border-0 -ml-4 max-w-full w-full": !editable,
  })}
>
  {#if editor && editable}
    <!-- Toolbar -->
    <div class="flex shrink-0 items-start justify-between gap-3 py-2">
      <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1" role="group" aria-label="Text formatting">
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class={cn({
          "bg-accent": isBold,
        })}
        aria-label="Bold"
        title="Bold"
        aria-pressed={isBold}
        onclick={() => {
          editor?.chain().focus().toggleBold().run();
        }}
      >
        <strong>B</strong>
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class={cn({
          "bg-accent": isItalic,
        })}
        aria-label="Italic"
        title="Italic"
        aria-pressed={isItalic}
        onclick={() => editor?.chain().focus().toggleItalic().run()}
      >
        <ItalicIcon class="size-4" />
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class={cn({
          "bg-accent": isBulletList,
        })}
        aria-label="Bullet list"
        title="Bullet list"
        aria-pressed={isBulletList}
        onclick={() => editor?.chain().focus().toggleBulletList().run()}
      >
        <ListIcon class="size-4" />
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class={cn({
          "bg-accent": isOrderedList,
        })}
        aria-label="Numbered list"
        title="Numbered list"
        aria-pressed={isOrderedList}
        onclick={() => editor?.chain().focus().toggleOrderedList().run()}
      >
        <ListOrderedIcon class="size-4" />
      </Button>
      <Popover.Root bind:open={linkPopoverOpen} onOpenChange={handleLinkPopoverChange}>
        <Popover.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label="Insert link"
              title="Insert link"
              aria-pressed={isLink}
              class={cn({
                "bg-accent": isLink,
              })}
            >
              <LinkIcon class="size-4" />
            </Button>
          {/snippet}
        </Popover.Trigger>
        <Popover.Content class="w-80">
          <div class="space-y-4">
            <div class="space-y-2">
              <h4 class="font-medium text-sm">Insert Link</h4>
              <p class="text-sm text-muted-foreground">
                {isLink ? "Edit or remove the link" : "Enter the URL below"}
              </p>
            </div>
            <div class="space-y-2">
              <Input
                type="url"
                placeholder="https://example.com"
                bind:value={linkUrl}
                onkeydown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleSetLink();
                  }
                }}
              />
            </div>
            <div class="flex gap-1">
              <Button
                type="button"
                size="sm"
                class="flex-1"
                onclick={handleSetLink}
              >
                {isLink ? "Update" : "Insert"}
              </Button>
              {#if isLink}
                <Button
                  type="button"
                  variant="destructive"
                  size="sm"
                  onclick={handleRemoveLink}
                >
                  Remove
                </Button>
              {/if}
            </div>
          </div>
        </Popover.Content>
      </Popover.Root>
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Quote" title="Quote" aria-pressed={isBlockquote} class={cn(isBlockquote && "bg-accent")} onclick={() => editor?.chain().focus().toggleBlockquote().run()}>
        <Quote class="size-4" />
      </Button>
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Clear formatting" title="Clear formatting" onclick={() => editor?.chain().focus().unsetAllMarks().clearNodes().run()}>
        <Eraser class="size-4" />
      </Button>
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Paste as plain text" title="Paste as plain text" disabled={pasting} onclick={pastePlainText}>
        <ClipboardPaste class="size-4" />
      </Button>
      </div>
      <div class="flex shrink-0 items-center gap-1" role="group" aria-label="Edit history">
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Undo" title="Undo" disabled={!canUndo} onclick={() => editor?.chain().focus().undo().run()}>
          <Undo2 class="size-4" />
        </Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Redo" title="Redo" disabled={!canRedo} onclick={() => editor?.chain().focus().redo().run()}>
          <Redo2 class="size-4" />
        </Button>
      </div>
    </div>
  {/if}

  <!-- Editor -->
  <div class="min-h-0 flex-1 overflow-y-auto" bind:this={element}></div>
</div>

<style>
  :global(.ProseMirror) {
    min-height: 100%;
    min-width: 100%;
    height: 100%;
    width: 100%;
  }
  :global(.ProseMirror p.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    float: left;
    color: var(--muted-foreground);
    pointer-events: none;
    height: 0;
  }
</style>
