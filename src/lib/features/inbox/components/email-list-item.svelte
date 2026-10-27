<script lang="ts">
  import { formatEmailDate, formatFullEmailDate } from "$lib/shared/utils/dates";
  import type { Mailbox, MailListItem } from "$lib/shared/local-mail";
  import { cn } from "$lib/utils";
  import MailAvatar from "$lib/components/mail-avatar.svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { sentFailureLabel } from "$lib/shared/sent-status";
  import { CircleAlert } from "@lucide/svelte";

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
  class={cn("group/mail flex w-full gap-3 border-b border-b-border/60 bg-[var(--mail-row-bg)] px-4 py-4 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring", isActive ? "[--mail-row-bg:var(--accent)]" : "[--mail-row-bg:var(--background)] hover:[--mail-row-bg:color-mix(in_oklab,var(--muted)_60%,var(--background))]")}
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
        <span class="relative size-4 shrink-0">
          <span class="sr-only">{failureLabel}</span>
          <span class="absolute right-0 top-0 flex w-max rounded-full bg-[var(--mail-row-bg)]">
            <Badge
              variant="destructive"
              aria-hidden="true"
              class={cn(
                "group/failure h-4 gap-0 px-0.5 py-0 text-[10px] leading-4 [&_svg]:size-2.5",
                mail.lastEvent === "suppressed" && "border-amber-600/20 bg-amber-500/10 text-amber-800 dark:border-amber-400/20 dark:text-amber-300",
                mail.lastEvent === "canceled" && "border-border bg-muted/60 text-muted-foreground",
              )}
            >
              <CircleAlert />
              <span class="grid grid-cols-[0fr] transition-[grid-template-columns] duration-200 ease-out delay-0 group-hover/failure:grid-cols-[1fr] group-hover/failure:delay-[125ms] group-focus-visible/mail:grid-cols-[1fr] group-focus-visible/mail:delay-0 motion-reduce:transition-none">
                <span class="min-w-0 overflow-hidden"><span class="pl-1.5 pr-1">{failureLabel}</span></span>
              </span>
            </Badge>
          </span>
        </span>
      {/if}
    </div>
    {#if mail.preview}<p class="mt-1 line-clamp-1 text-xs leading-5 text-muted-foreground">{mail.preview}</p>{/if}
  </div>
</button>
