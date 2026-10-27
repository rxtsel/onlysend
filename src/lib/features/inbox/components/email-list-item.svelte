<script lang="ts">
  import { formatEmailDate, formatFullEmailDate } from "$lib/shared/utils/dates";
  import type { Mailbox, MailListItem } from "$lib/shared/local-mail";
  import { cn } from "$lib/utils";
  import MailAvatar from "$lib/components/mail-avatar.svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { sentFailureLabel } from "$lib/shared/sent-status";

  let { mail, mode, isActive = false, isUnread = false, onclick }: {
    mail: MailListItem;
    mode: Mailbox;
    isActive?: boolean;
    isUnread?: boolean;
    onclick: (id: string) => void;
  } = $props();
  const address = $derived(mode === "inbox" ? mail.from : mail.to.join(", "));
  const name = $derived(address.split("<")[0].replaceAll('"', "").trim() || address);
  const failureLabel = $derived(mode === "sent" ? sentFailureLabel(mail.lastEvent) : null);
</script>

<button
  onclick={() => onclick(mail.id)}
  aria-current={isActive ? "true" : undefined}
  class={cn("flex w-full gap-3 border-b border-b-border/60 px-4 py-4 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring", isActive ? "bg-accent" : "hover:bg-muted/60")}
>
  <div class="mt-0.5 shrink-0"><MailAvatar address={mode === "inbox" ? mail.from : (mail.to[0] ?? "")} {name} /></div>
  <div class="min-w-0 flex-1">
    <div class="flex items-center gap-2">
      <span class={cn("min-w-0 flex-1 truncate text-sm", isUnread ? "font-semibold text-foreground" : "font-medium")}>{name || "Unknown sender"}</span>
      {#if isUnread}<span class="size-1.5 shrink-0 rounded-full bg-primary" title="Unread"><span class="sr-only">Unread</span></span>{/if}
      <time class="shrink-0 text-[11px] tabular-nums text-muted-foreground" title={formatFullEmailDate(mail.createdAt)}>{formatEmailDate(mail.createdAt)}</time>
    </div>
    <div class="mt-1 flex min-w-0 items-center gap-2">
      <p class={cn("min-w-0 flex-1 truncate text-[13px]", isUnread ? "font-medium text-foreground" : "text-foreground/80")}>{mail.subject || "(No subject)"}</p>
      {#if failureLabel}
        <Badge
          variant="destructive"
          class={cn(
            "gap-1.5 px-1.5 py-0 text-[10px] leading-4",
            mail.lastEvent === "suppressed" && "border-amber-600/20 bg-amber-500/10 text-amber-800 dark:border-amber-400/20 dark:text-amber-300",
            mail.lastEvent === "canceled" && "border-border bg-muted/60 text-muted-foreground",
          )}
        >
          <span aria-hidden="true" class="size-1 shrink-0 rounded-full bg-current"></span>
          {failureLabel}
        </Badge>
      {/if}
    </div>
    {#if mail.preview}<p class="mt-1 line-clamp-1 text-xs leading-5 text-muted-foreground">{mail.preview}</p>{/if}
  </div>
</button>
