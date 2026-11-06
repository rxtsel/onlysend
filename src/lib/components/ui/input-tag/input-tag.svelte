<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import { z } from "zod/v4";
  import { cn } from "@/lib/utils.js";
  import CloseIcon from "@lucide/svelte/icons/x";

  export type Tag = {
    id: string;
    text: string;
  };

  type Props = {
    value?: Tag[];
    inputValue?: string;
    type?: "email";
    list?: string[];
    class?: string;
    placeholder?: string;
    name?: string;
    onDuplicateAttempt?: (text: string) => void;
    onchange?: () => void;
  } & Omit<HTMLInputAttributes, "value" | "type" | "list" | "name" | "onchange">;

  let {
    value = $bindable<Tag[]>([]),
    inputValue = $bindable(""),
    type = "email",
    list = [] as string[],
    class: className,
    placeholder = "",
    name,
    onDuplicateAttempt,
    onchange,
    ...restProps
  }: Props = $props();

  let inputRef = $state<HTMLInputElement | null>(null);
  let activeTagIndex = $state<number | null>(null);
  let duplicateTag = $state<string | null>(null);

  const emailSchema = z.email();

  function validateTags(next: Tag[]): Tag[] {
    if (type !== "email") return next;
    return next.filter((tag) => emailSchema.safeParse(tag.text).success);
  }

  function commitTagFromValue(raw: string) {
    const text = raw.trim();
    if (!text || (type === "email" && !emailSchema.safeParse(text).success)) return;

    // Normalize for case-insensitive comparison
    const normalizedText = text.toLowerCase();
    const existingTag = value.find(
      (tag) => tag.text.toLowerCase() === normalizedText,
    );

    if (existingTag) {
      // If exists, temporary highlight and callback
      duplicateTag = existingTag.id;
      onDuplicateAttempt?.(text);

      // Remove highlight after 1 second
      setTimeout(() => {
        duplicateTag = null;
      }, 1000);

      // Clean input
      inputValue = "";
      return;
    }

    let next = [...value, { id: crypto.randomUUID(), text }];
    next = validateTags(next);
    value = next;
    inputValue = "";
    onchange?.();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      commitTagFromValue(inputValue);
      return;
    }
    if (event.key === "Backspace" && inputValue === "" && value.length > 0) {
      value = value.slice(0, -1);
      activeTagIndex = value.length - 1;
      onchange?.();
      return;
    }
    activeTagIndex = null;
  }

  function handleBlur() {
    commitTagFromValue(inputValue);
  }

  function handlePaste(event: ClipboardEvent) {
    const text = event.clipboardData?.getData("text");
    if (!text) return;
    event.preventDefault();

    const pieces = text
      .split(/[,;\n]/)
      .map((p) => p.trim())
      .filter(Boolean);

    // Filter out duplicates
    const normalizedExisting = value.map((tag) => tag.text.toLowerCase());
    const uniquePieces = pieces.filter(
      (p) => !normalizedExisting.includes(p.toLowerCase()),
    );

    let next = [
      ...value,
      ...uniquePieces.map((p) => ({ id: crypto.randomUUID(), text: p })),
    ];
    next = validateTags(next);
    value = next;
    inputValue = "";
    onchange?.();
  }

  function removeTag(id: string) {
    value = value.filter((tag) => tag.id !== id);
    activeTagIndex = null;
    onchange?.();
  }

  function chooseSuggestion(text: string) {
    commitTagFromValue(text);
  }
</script>

<!-- Hidden inputs para FormData -->
{#if name}
  {#each value as tag (tag.id)}
    <input type="hidden" {name} value={tag.text} />
  {/each}
{/if}

<div
  class={cn(
    "border-input border rounded-md bg-background shadow-xs transition-[color,box-shadow] focus-within:border-ring outline-none focus-within:ring-[3px] focus-within:ring-ring/50 p-1 gap-1 flex flex-wrap items-center",
    className,
    {
      "border-red-500 focus-within:ring-red-400/50": restProps["aria-invalid"],
    },
  )}
>
  {#each value as tag, index (tag.id)}
    <span
      class={cn(
        "h-7 relative border rounded-md font-medium text-xs ps-2 pe-1 flex items-center gap-1 transition-colors",
        tag.id === duplicateTag
          ? "bg-yellow-100 dark:bg-yellow-900/30 border-yellow-400 dark:border-yellow-600 animate-pulse"
          : "bg-background border-input hover:bg-muted",
      )}
      data-active={index === activeTagIndex}
    >
      {tag.text}
      <button
        type="button"
        class="absolute -top-1 -right-1 p-0 rounded-e-md flex size-fit transition-[color,box-shadow] outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] text-muted-foreground/80 hover:text-foreground"
        onclick={() => removeTag(tag.id)}
      >
        <CloseIcon class="size-3" aria-hidden="true" />
      </button>
    </span>
  {/each}
  <input
    bind:this={inputRef}
    class="w-full min-w-20 shadow-none px-2 h-7 outline-none bg-transparent flex-1"
    bind:value={inputValue}
    {placeholder}
    onkeydown={handleKeydown}
    onblur={handleBlur}
    onpaste={handlePaste}
    {...restProps}
  />
</div>

{#if list.length}
  <ul class="mt-1 flex flex-wrap gap-1 text-xs text-muted-foreground">
    {#each list as suggestion}
      <li>
        <button
          type="button"
          class="py-1 px-2 rounded border border-dashed border-input hover:border-solid hover:border-ring"
          onclick={() => chooseSuggestion(suggestion)}
        >
          {suggestion}
        </button>
      </li>
    {/each}
  </ul>
{/if}
