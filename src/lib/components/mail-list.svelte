<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import { RefreshCw, Search, Inbox, LoaderCircle } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import { Button } from "$lib/components/ui/button";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import * as InputGroup from "$lib/components/ui/input-group";
  import * as Sidebar from "$lib/components/ui/sidebar";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import EmailListItem from "$lib/features/inbox/components/email-list-item.svelte";
  import { createEmailList } from "$lib/shared/email-list.svelte";
  import { loadMailList, getMailArchiveStatus, refreshMailbox, type Mailbox, type MailArchiveStatus, type MailListItem } from "$lib/shared/local-mail";
  import type { ArchiveFooterData } from "$lib/shared/archive-status";
  import { authErrorToast, isAuthError } from "$lib/shared/services/auth-toast.svelte";
  import { errorMessage } from "$lib/shared/utils/errors";
  import { cn } from "$lib/utils";

  let { mailbox, domain, active = true, class: className, onDomainsChange, onSelectMail, onArchiveStatusChange }: {
    mailbox: Mailbox;
    domain: string | null;
    active?: boolean;
    class?: string;
    onDomainsChange: (domains: string[]) => void;
    onSelectMail: (mail: MailListItem, mailbox: Mailbox) => void;
    onArchiveStatusChange: (accountId: string, mailbox: Mailbox, data: ArchiveFooterData) => void;
  } = $props();
  const accountId = useAccountId();
  let search = $state("");
  let searchTerm = $state("");
  let manualRefreshing = $state(false);
  let archiveStatus = $state<MailArchiveStatus | null>(null);
  let statusError = $state<string | null>(null);
  let disposed = false;
  let lastKey = "";
  let lastMailbox: Mailbox | null = null;
  let catalogKey = "";

  async function fetchPage(limit: number, after: string | null, refresh: boolean) {
    const owner = mailbox;
    const result = await loadMailList(accountId, { mailbox: owner, limit, after, domain, search: searchTerm || null, refresh });
    const key = result.domains.join("\0");
    if (!disposed && owner === mailbox && key !== catalogKey) {
      catalogKey = key;
      onDomainsChange(result.domains);
    }
    return result;
  }

  const list = createEmailList<MailListItem>(
    (limit, after) => fetchPage(limit, after, after === null),
    50,
    async (limit, after) => {
      const result = await fetchPage(limit, after, false);
      return result.cache ? result : null;
    },
  );
  const threads = $derived.by(() => {
    const grouped = new Map<string, { mail: MailListItem; count: number; unread: boolean; ids: Set<string> }>();
    for (const mail of list.items) {
      const key = mail.threadId ?? `message:${mail.id}`;
      const thread = grouped.get(key);
      if (thread) {
        thread.count += 1;
        thread.unread ||= mailbox === "inbox" && !mail.isRead;
        thread.ids.add(mail.id);
      } else {
        grouped.set(key, { mail, count: mail.threadCount ?? 1, unread: mailbox === "inbox" && !mail.isRead, ids: new Set([mail.id]) });
      }
    }
    return [...grouped.values()];
  });

  function report(error: unknown) {
    if (disposed) return;
    if (isAuthError(error)) authErrorToast(error);
    else toast.error(errorMessage(error, "Could not load mail"));
  }

  async function refreshStatus() {
    const owner = mailbox;
    try {
      const result = await getMailArchiveStatus(accountId, owner);
      if (disposed || owner !== mailbox) return;
      const changed = archiveStatus !== null && (
        archiveStatus.downloadedMessages !== result.downloadedMessages ||
        archiveStatus.downloadedBodies !== result.downloadedBodies
      );
      archiveStatus = result;
      statusError = null;
      if (changed) await list.refreshCached();
    } catch (error) {
      if (!disposed && owner === mailbox) statusError = errorMessage(error, "Could not read download status");
    }
  }

  $effect(() => {
    const owner = mailbox;
    untrack(() => { search = ""; searchTerm = ""; });
  });

  $effect(() => {
    const value = search.trim();
    const timer = setTimeout(() => { searchTerm = value; }, 300);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    const key = JSON.stringify([mailbox, domain, searchTerm]);
    const owner = mailbox;
    if (!active) { lastKey = ""; return; }
    untrack(() => {
      if (lastKey === key) return;
      const revalidate = lastMailbox !== owner || lastKey === "";
      if (lastMailbox !== owner) {
        archiveStatus = null;
        statusError = null;
        void refreshStatus();
      }
      lastMailbox = owner;
      lastKey = key;
      list.clearItems();
      void (revalidate ? list.refresh() : list.refreshCached()).catch(report);
    });
  });

  $effect(() => {
    const data: ArchiveFooterData = {
      status: archiveStatus, statusError, refreshError: list.error,
      downloadedAt: list.cacheInfo?.downloadedAt ?? null,
    };
    const owner = mailbox;
    untrack(() => onArchiveStatusChange(accountId, owner, data));
  });

  function openMail(id: string, threadCount: number) {
    const mail = list.items.find((item) => item.id === id);
    if (mail) onSelectMail({ ...mail, threadCount }, mailbox);
    void goto(mailUrl(accountId, mailbox, id));
  }

  async function refresh() {
    if (manualRefreshing || list.isRefreshing) return;
    const owner = mailbox;
    manualRefreshing = true;
    try {
      await refreshMailbox(accountId, owner);
      if (disposed || owner !== mailbox) return;
      await list.refreshCached();
      await refreshStatus();
      toast.success("Mail refreshed");
    } catch (error) { report(error); }
    finally { manualRefreshing = false; }
  }

  onMount(() => {
    const cleanups: (() => void)[] = [];
    function subscribe<T>(name: string, handler: (payload: T) => void) {
      void listen<T>(name, ({ payload }) => { if (!disposed) handler(payload); })
        .then((cleanup) => { if (disposed) cleanup(); else cleanups.push(cleanup); })
        .catch(console.error);
    }
    subscribe<{ accountId: string; mailbox: Mailbox }>("mail-archive-updated", (event) => {
      if (event.accountId === accountId && event.mailbox === mailbox && active) void refreshStatus();
    });
    subscribe<{ accountId: string }>("inbox-updated", (event) => {
      if (event.accountId === accountId && mailbox === "inbox" && active && !manualRefreshing) {
        void list.refreshCached().catch(report);
        void refreshStatus();
      }
    });
    subscribe<{ accountId: string; emailId: string }>("inbox-read", (event) => {
      if (event.accountId !== accountId || mailbox !== "inbox") return;
      const item = list.items.find((mail) => mail.id === event.emailId);
      if (item) item.isRead = true;
    });
    const visible = () => { if (!document.hidden && active) void refreshStatus(); };
    document.addEventListener("visibilitychange", visible);
    return () => { cleanups.forEach((cleanup) => cleanup()); document.removeEventListener("visibilitychange", visible); };
  });

  onDestroy(() => { disposed = true; list.clearItems(); });
</script>

<section class={cn("flex min-h-0 min-w-0 flex-col border-e bg-background", className)} aria-label={`${mailbox === "inbox" ? "Inbox" : "Sent"} messages`}>
  <header class="flex h-16 shrink-0 items-center gap-2 border-b px-4">
    <Sidebar.Trigger aria-label="Toggle mailboxes" />
    <div class="min-w-0 flex-1">
      <h1 class="text-base font-semibold tracking-tight">{mailbox === "inbox" ? "Inbox" : "Sent"}</h1>
      {#if domain}<p class="truncate text-xs text-muted-foreground">{domain}</p>{/if}
    </div>
    <Button variant="ghost" size="icon" onclick={refresh} disabled={list.isRefreshing || manualRefreshing} aria-label="Refresh mail" title="Refresh mail">
      <RefreshCw class={cn((list.isRefreshing || manualRefreshing) && "animate-spin motion-reduce:animate-none")} />
    </Button>
  </header>
  <div class="shrink-0 border-b px-4 py-3">
    <InputGroup.Root>
      <InputGroup.Input type="search" placeholder="Search mail" aria-label="Search downloaded mail" maxlength={512} bind:value={search} />
      <InputGroup.Addon><Search /></InputGroup.Addon>
    </InputGroup.Root>
  </div>
  <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
    {#if list.error}
      <div class="border-b px-4 py-3 text-sm text-destructive" role="alert">
        <p>{list.items.length ? "Couldn't refresh. Showing saved mail." : "Couldn't load mail."}</p>
        <p class="mt-1 text-xs">{list.error}</p>
      </div>
    {/if}
    {#if list.isLoading}
      <div class="flex flex-col gap-6 p-4" aria-label="Loading mail" role="status">
        {#each Array(6) as _}<div class="flex flex-col gap-2"><Skeleton class="h-4 w-2/3" /><Skeleton class="h-3 w-5/6" /><Skeleton class="h-3 w-3/4" /></div>{/each}
      </div>
    {:else if list.items.length === 0}
      <div class="flex h-full min-h-48 flex-col items-center justify-center gap-3 p-6 text-center">
        <Inbox class="size-7 text-muted-foreground" aria-hidden="true" />
        <p class="text-sm font-medium">{searchTerm ? "No matches" : list.cacheInfo ? "No mail yet" : "No saved mail"}</p>
        <p class="text-xs text-muted-foreground">{searchTerm ? "Try another search." : list.cacheInfo ? "Your messages will appear here." : "Refresh to check for messages."}</p>
        {#if list.error || !list.cacheInfo}<Button variant="outline" size="sm" onclick={refresh}>{list.error ? "Try again" : "Check for mail"}</Button>{/if}
      </div>
    {:else}
      {#each threads as thread (thread.mail.threadId ?? thread.mail.id)}
        {@const mail = thread.mail}
        <EmailListItem {mail} mode={mailbox} threadCount={mail.threadCount ?? thread.count} isActive={thread.ids.has(page.params.id ?? "")} isUnread={thread.unread} onclick={(id) => openMail(id, mail.threadCount ?? thread.count)} />
      {/each}
      {#if list.hasMore}
        <div class="p-3"><Button class="w-full" variant="ghost" disabled={list.isLoadingMore || list.isRefreshing} onclick={() => { void list.loadMore().catch(report); }}>
          {#if list.isLoadingMore}<LoaderCircle class="animate-spin motion-reduce:animate-none" />{/if} Load more
        </Button></div>
      {/if}
    {/if}
  </div>
</section>
