<script lang="ts">
  import { formatEmailDate } from "$lib/shared/utils/dates";

  export type EmailListMode = "inbox" | "sent";

  let {
    mail,
    mode,
    isActive = false,
    isUnread = false,
    domainLabel = "",
    onclick,
  }: {
    mail: Record<string, any>;
    mode: EmailListMode;
    isActive?: boolean;
    isUnread?: boolean;
    /** Shown when the unified view has multiple domains. */
    domainLabel?: string;
    onclick: (id: string) => void;
  } = $props();

  function dateOf(): string {
    // The two list models differ only in their date field naming.
    return "createdAt" in mail ? mail.createdAt : mail.created_at;
  }

  function primaryLine(): string {
    return mode === "inbox" ? (mail.from ?? "") : (mail.to?.[0] ?? "");
  }
</script>

<button
  onclick={() => onclick(mail.id)}
  class="hover:bg-sidebar-accent hover:text-sidebar-accent-foreground flex w-full flex-col items-start gap-2 whitespace-nowrap border-b p-4 text-sm leading-tight last:border-b-0 text-left {isActive
      ? 'bg-sidebar-accent'
      : ''}"
>
  <div class="flex w-full items-center gap-2">
    <span
        class="truncate {mode === 'inbox' && !isUnread
            ? 'font-normal text-muted-foreground'
            : 'font-semibold'}"
    >
        {primaryLine()}
    </span>

    {#if mode === "inbox" && isUnread}
        <span
            class="size-2 rounded-full bg-blue-500 shrink-0"
            title="Unread"
        ></span>
    {/if}

    <span class="ms-auto text-xs shrink-0">
        {formatEmailDate(dateOf())}
    </span>
  </div>
  <div class="flex w-full items-center gap-1.5 min-w-0">
      <span
          class="truncate flex-1 {isUnread && mode === 'inbox'
              ? 'text-foreground'
              : 'text-muted-foreground'}"
          >{mail.subject}</span
      >
      {#if domainLabel}
          <span
              class="text-[10px] shrink-0 px-1.5 py-px rounded bg-muted text-muted-foreground"
          >
              {domainLabel}
          </span>
      {/if}
  </div>
</button>
