<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Editor } from "@tiptap/core";
  import StarterKit from "@tiptap/starter-kit";
  import Placeholder from "@tiptap/extension-placeholder";
  import Link from "@tiptap/extension-link";
  import {
    Link as LinkIcon,
    Italic as ItalicIcon,
    List as ListIcon,
    ListOrdered as ListOrderedIcon,
    Paperclip,
  } from "@lucide/svelte";
  import Button, { buttonVariants } from "../ui/button/button.svelte";
  import * as Popover from "$lib/components/ui/popover";

  import { cn } from "@/lib/utils";
  import Input from "../ui/input/input.svelte";
  import type { AttachmentPayload } from "@/lib/types";

  let element = $state<HTMLDivElement | null>(null);
  let editor = $state<Editor | null>(null);

  let {
    content = $bindable(""),
    placeholder = "Write your email...",
    editable = true,
    files = $bindable<File[]>([]),
    attachments = $bindable<AttachmentPayload[]>([]),
    "aria-invalid": ariaInvalid,
  } = $props();

  export function focus(): boolean {
    const writer = element?.querySelector<HTMLElement>('[contenteditable="true"]');
    if (!writer) return false;
    writer.focus();
    return document.activeElement === writer;
  }

  // States reactive for buttons
  let isBold = $state(false);
  let isItalic = $state(false);
  let isBulletList = $state(false);
  let isOrderedList = $state(false);
  let isLink = $state(false);

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
  }

  function handleOpenLinkPopover() {
    // If there's already a link, prefill the input
    const previousUrl = editor?.getAttributes("link").href || "";
    linkUrl = previousUrl;
    linkPopoverOpen = true;
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
        StarterKit,
        Placeholder.configure({ placeholder }),
        Link.configure({
          openOnClick: false,
          HTMLAttributes: {
            class: "text-blue-400 underline cursor-pointer",
          },
        }),
      ],
      content,
      editable,
      editorProps: {
        attributes: {
          class:
            "prose prose-sm max-h-full w-full h-full min-h-full sm:prose-base p-5 focus:outline-none",
        },
      },
      onTransaction: () => {
        updateActiveStates();
        editor = editor;
      },
      onUpdate: ({ editor }) => {
        content = editor.getHTML();
        updateActiveStates();
      },
      onSelectionUpdate: () => {
        updateActiveStates();
      },
    });

    editor = inst;
  });

  onDestroy(() => {
    editor?.destroy();
  });
</script>

<div
  class={cn("border rounded-t-md h-full shrink", {
    "border-red-500": ariaInvalid,
    "border-0 -ml-4 max-w-full w-full": !editable,
  })}
>
  {#if editor && editable}
    <!-- Toolbar -->
    <div class="border-b p-2 flex gap-1">
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class={cn({
          "bg-accent": isBold,
        })}
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
        onclick={() => editor?.chain().focus().toggleOrderedList().run()}
      >
        <ListOrderedIcon class="size-4" />
      </Button>
      <Popover.Root open={linkPopoverOpen}>
        <Popover.Trigger>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            class={cn({
              "bg-accent": isLink,
            })}
            onclick={handleOpenLinkPopover}
          >
            <LinkIcon class="size-4" />
          </Button>
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
      <label
        class={buttonVariants({
          variant: "ghost",
          size: "icon-sm",
        })}
      >
        <input
          type="file"
          name="files"
          class="hidden"
          multiple
          accept="image/*,application/pdf,.pdf,
            application/msword,
            application/vnd.openxmlformats-officedocument.wordprocessingml.document,
            application/vnd.ms-excel,
            application/vnd.openxmlformats-officedocument.spreadsheetml.sheet,
            application/vnd.ms-powerpoint,
            application/vnd.openxmlformats-officedocument.presentationml.presentation"
          onchange={async (e) => {
            const input = e.currentTarget as HTMLInputElement;
            if (!input.files) return;

            const selectedFiles = Array.from(input.files);
            files = selectedFiles;

            // Convert to base64 to send it to Rust
            const newAttachments: AttachmentPayload[] = [];

            for (const file of selectedFiles) {
              const buffer = await file.arrayBuffer();
              const bytes = new Uint8Array(buffer);

              // ArrayBuffer -> base64
              let binary = "";
              for (let i = 0; i < bytes.length; i++) {
                binary += String.fromCharCode(bytes[i]);
              }
              const base64 = btoa(binary);

              newAttachments.push({
                filename: file.name,
                content: base64,
                mimeType: file.type || "application/octet-stream",
              });
            }

            attachments = newAttachments;
          }}
        />
        <Paperclip class="size-4" />
      </label>
    </div>
  {/if}

  <!-- Editor -->
  <div class="h-full min-h-full max-h-full shrink" bind:this={element}></div>
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
    color: hsl(var(--muted-foreground));
    pointer-events: none;
    height: 0;
    opacity: 0.5;
  }
</style>
