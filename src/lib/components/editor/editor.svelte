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
  } from "@lucide/svelte";
  import Button from "../ui/button/button.svelte";
  import * as Popover from "$lib/components/ui/popover";

  import { cn } from "@/lib/utils";
  import Input from "../ui/input/input.svelte";

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
    <div class="flex shrink-0 flex-wrap items-center gap-1 py-2" role="group" aria-label="Text formatting">
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
      <Popover.Root bind:open={linkPopoverOpen} onOpenChange={handleLinkPopoverChange}>
        <Popover.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label="Insert link"
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
