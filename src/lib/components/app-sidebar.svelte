<script lang="ts" module>
    import InboxIcon from "@lucide/svelte/icons/inbox";
    import SendIcon from "@lucide/svelte/icons/send";
    import PlusIcon from "@lucide/svelte/icons/plus";
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
    import SettingsDialog from "./settings-dialog.svelte";
</script>

<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  const accountId = useAccountId();
    import NavUser from "./nav-user.svelte";
    import { useSidebar } from "@/lib/components/ui/sidebar/context.svelte.js";
    import * as Sidebar from "@/lib/components/ui/sidebar/index.js";
    import { Button } from "@/lib/components/ui/button/index.js";
    import type { ComponentProps } from "svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/state";
    import { onMount, onDestroy, untrack } from "svelte";
    import { fly } from "svelte/transition";
    import { toast } from "svelte-sonner";
    import { Loader } from "@lucide/svelte";
    import type { InboundEmail } from "../shared/inbound";
    import type { SentEmail } from "../types";
    import { listSentEmails } from "../shared/sent";
    import {
        getReadInboundIds,
        listInboundEmails,
        markInboundRead,
    } from "../shared/inbound";
    import EmailListItem from "@/lib/features/inbox/components/email-list-item.svelte";
import { authErrorToast, isAuthError } from "@/lib/shared/services/auth-toast.svelte";
import { errorMessage } from "@/lib/shared/utils/errors";
    import { createEmailList } from "../shared/email-list.svelte";
    import { mailDomains } from "$lib/shared/mail-domains";

    let {
        ref = $bindable(null),
        ...restProps
    }: ComponentProps<typeof Sidebar.Root> = $props();

    /* ---------------------------------------------------------
     * MODE: which list the sidebar shows, driven by the route
     * --------------------------------------------------------- */
    type Mode = "inbox" | "sent";

    const mode = $derived(
        page.url.pathname.startsWith(mailUrl(accountId, "inbox")) ? ("inbox" as const) : ("sent" as const),
    );

    const data = {
        navMain: [
            { title: "Inbox", url: mailUrl(accountId, "inbox"), icon: InboxIcon, mode: "inbox" as const },
            { title: "All Sent", url: mailUrl(accountId, "sent"), icon: SendIcon, mode: "sent" as const },
        ],
    };

    const activeItem = $derived(
        data.navMain.find((item) => item.mode === mode) ?? data.navMain[0],
    );

    const sidebar = useSidebar();
    const PAGE_SIZE = 16;
    const POLL_INTERVAL_MS = 60_000;

    /* ---------------------------------------------------------
     * DOMAIN FILTER (unified views, client-side filtering)
     * --------------------------------------------------------- */
    let domainFilter = $state<"all" | string>("all");

    function dateOf(mail: InboundEmail | SentEmail): string {
        return mail.createdAt;
    }

    function domainOf(mail: InboundEmail | SentEmail): string {
        return mailDomains(mail).join(", ");
    }
    const sentList = createEmailList<SentEmail>((limit, after) =>
        listSentEmails(accountId, limit, after, true),
    );

    const inboxList = createEmailList<InboundEmail>((limit, after) =>
        listInboundEmails(accountId, limit, after),
    );

    const activeList = $derived(mode === "inbox" ? inboxList : sentList);

    /* ---------------------------------------------------------
     * DOMAIN FILTER (unified views)
     * --------------------------------------------------------- */
    const activeDomains = $derived.by(() => {
        const set = new Set<string>();
        for (const mail of activeList.items as (InboundEmail | SentEmail)[]) {
            for (const domain of mailDomains(mail)) set.add(domain);
        }
        return [...set].sort();
    });

    const visibleItems = $derived(
        domainFilter === "all"
            ? (activeList.items as (InboundEmail | SentEmail)[])
            : (activeList.items as (InboundEmail | SentEmail)[]).filter(
                  (mail) => mailDomains(mail).includes(domainFilter),
              ),
    );

    // History belongs to the account. DNS readiness only controls new
    // delivery, never access to previously received messages.
    let lastLoadContext = "";
    let disposed = false;

    onDestroy(() => {
        disposed = true;
        sentList.clearItems();
        inboxList.clearItems();
    });

    // Only mode drives loading, not items or flags (empty history is valid).
    $effect(() => {
        const currentMode = mode;
        untrack(() => {
            if (currentMode === lastLoadContext) return;
            lastLoadContext = currentMode;
            domainFilter = "all";
            const list = currentMode === "inbox" ? inboxList : sentList;
            void list.refresh().catch((err) => {
                if (disposed) return;
                if (isAuthError(err)) authErrorToast(err);
                else toast.error(errorMessage(err, "Failed to load emails"));
            });
        });
    });

    async function handleRefresh() {
        const currentMode = mode;
        const list = activeList;
        try {
            await list.refresh();
            if (disposed || mode !== currentMode) return;
            toast.success("Emails refreshed");
        } catch (err) {
            if (disposed || mode !== currentMode) return;
            if (isAuthError(err)) {
                authErrorToast(err);
            } else {
                toast.error(errorMessage(err, "Failed to load emails"));
            }
        }
    }

    function handleLoadMore() {
        void activeList.loadMore().catch((err) => {
            if (!disposed) toast.error(errorMessage(err, "Failed to load emails"));
        });
    }

    /* ---------------------------------------------------------
     * INBOX READ MARKERS
     * --------------------------------------------------------- */
    let readIds = $state<Set<string>>(new Set());

    onMount(async () => {
        try {
            const ids = await getReadInboundIds(accountId);
            if (disposed) return;
            readIds = new Set(ids);
        } catch (err) {
            console.error("Error loading inbox markers:", err);
        }
    });

    // Keep Inbox reachable even before receiving is configured for this
    // account; the page owns setup/readiness, not a global navigation flag.
    const navItems = data.navMain;

    async function handleEmailClick(mailId: string) {
        if (mode === "inbox" && !readIds.has(mailId)) {
            readIds = new Set([...readIds, mailId]);
            markInboundRead(accountId, mailId).catch(console.error);
        }
        goto(mailUrl(accountId, mode, mailId));
    }

    /* ---------------------------------------------------------
     * INBOX POLLING (only while the inbox is visible)
     * --------------------------------------------------------- */
    onMount(() => {
        const interval = setInterval(async () => {
            if (
                mode !== "inbox" ||
                document.hidden ||
                disposed
            )
                return;

            try {
                const previousNewest = inboxList.items[0]?.id;
                await inboxList.refreshSilent();
                if (disposed) return;

                const items = inboxList.items;
                const oldIndex = previousNewest
                    ? items.findIndex((m) => m.id === previousNewest)
                    : -1;

                const newCount =
                    oldIndex === -1
                        ? Math.max(items.length - 1, 0)
                        : oldIndex;

                if (newCount > 0) {
                    toast.success(
                        `${newCount} new email${newCount > 1 ? "s" : ""}`,
                    );
                }
            } catch (err) {
                console.error("Inbox polling failed:", err);
            }
        }, POLL_INTERVAL_MS);

        return () => clearInterval(interval);
    });
</script>

<Sidebar.Root
    bind:ref
    collapsible="icon"
    class="overflow-hidden *:data-[sidebar=sidebar]:flex-row"
    {...restProps}
>
    <!-- This is the first sidebar -->
    <Sidebar.Root
        collapsible="none"
        class="w-[calc(var(--sidebar-width-icon)+1px)]! border-e"
    >
        <Sidebar.Header>
            <Sidebar.Menu>
                <Sidebar.MenuItem>
                    <Sidebar.MenuButton
                        variant="primary"
                        size="lg"
                        class="size-8 justify-center rounded-lg transition-colors p-0"
                    >
                        <a
                            href={mailUrl(accountId, "composer")}
                            class="flex h-full w-full items-center justify-center"
                            onclick={() => {
                                if (sidebar.open) sidebar.setOpen(false);
                            }}
                        >
                            <PlusIcon class="text-white size-5" />
                        </a>
                    </Sidebar.MenuButton>
                </Sidebar.MenuItem>
            </Sidebar.Menu>
        </Sidebar.Header>
        <Sidebar.Content>
            <Sidebar.Group>
                <Sidebar.GroupContent class="px-1.5 md:px-0">
                    <Sidebar.Menu>
                        {#each navItems as item (item.title)}
                            <Sidebar.MenuItem>
                                <Sidebar.MenuButton
                                    tooltipContentProps={{
                                        hidden: false,
                                    }}
                                    onclick={() => {
                                        sidebar.setOpen(true);
                                        goto(item.url);
                                    }}
                                    isActive={activeItem.title === item.title}
                                    class="px-2.5 md:px-2"
                                >
                                    {#snippet tooltipContent()}
                                        {item.title}
                                    {/snippet}
                                    <item.icon />
                                    <span>{item.title}</span>
                                </Sidebar.MenuButton>
                            </Sidebar.MenuItem>
                        {/each}
                    </Sidebar.Menu>
                </Sidebar.GroupContent>
            </Sidebar.Group>
        </Sidebar.Content>
        <Sidebar.Footer>
            <SettingsDialog />
            <NavUser />
        </Sidebar.Footer>
    </Sidebar.Root>

    <!-- This is the second sidebar -->
    <Sidebar.Root collapsible="none" class="hidden flex-1 md:flex">
        <Sidebar.Header class="gap-3.5 border-b p-4">
            <div class="flex w-full items-center justify-between">
                <div class="text-foreground text-base font-medium">
                    {activeItem.title}
                </div>
                <Button
                    variant="ghost"
                    size="icon-sm"
                    onclick={handleRefresh}
                    disabled={activeList.isRefreshing}
                    title="Refresh emails"
                >
                    <RefreshCwIcon
                        class={activeList.isRefreshing ? "animate-spin" : ""}
                    />
                </Button>
            </div>

            {#if activeDomains.length > 1}
                <!-- Temporary filter over loaded pages, not setup inclusion. -->
                <p class="text-xs text-muted-foreground">Filter loaded messages; load more to include older history.</p>
                <div class="flex flex-wrap gap-1.5">
                    <button
                        onclick={() => (domainFilter = "all")}
                        class="text-xs px-2 py-0.5 rounded-full border transition-colors {domainFilter === 'all'
                            ? 'bg-primary text-primary-foreground border-primary'
                            : 'hover:bg-sidebar-accent'}"
                    >
                        All
                    </button>
                    {#each activeDomains as d (d)}
                        <button
                            onclick={() => (domainFilter = d)}
                            class="text-xs px-2 py-0.5 rounded-full border transition-colors {domainFilter === d
                                ? 'bg-primary text-primary-foreground border-primary'
                                : 'hover:bg-sidebar-accent'}"
                        >
                            {d}
                        </button>
                    {/each}
                </div>
            {/if}
        </Sidebar.Header>
        <Sidebar.Content
            class="overflow-y-auto overflow-x-hidden max-w-[400px]"
        >
            <Sidebar.Group class="px-0 pt-0">
                <Sidebar.GroupContent>
                    {#if activeList.isLoading}
                        {@const skeletons = Array.from({ length: PAGE_SIZE })}
                        {#each skeletons}
                            <div
                                class="animate-pulse p-4 border-b last:border-b-0"
                            >
                                <div
                                    class="h-4 bg-muted w-3/4 mb-2 rounded"
                                ></div>
                                <div class="h-3 bg-muted w-1/2 rounded"></div>
                            </div>
                        {/each}
                    {:else if activeList.error && activeList.items.length === 0}
                        <div class="p-8 text-center text-sm text-muted-foreground" role="alert">
                            {activeList.error}
                            <Button variant="outline" size="sm" onclick={handleRefresh}>
                                Retry
                            </Button>
                        </div>
                    {:else if activeList.items.length === 0 && !activeList.isRefreshing}
                        <div
                            class="p-8 text-center text-sm text-muted-foreground"
                        >
                            {#if mode === "inbox"}
                                No received emails yet
                            {:else}
                                No sent emails yet
                            {/if}
                        </div>
                    {:else}
                        {#if visibleItems.length === 0}
                            <p class="p-4 text-sm text-muted-foreground">No matching messages in the loaded pages.</p>
                        {/if}
                        {#each visibleItems as mail (mail.id)}
                            <EmailListItem
                                mail={mail}
                                {mode}
                                isActive={page.params.id === mail.id}
                                isUnread={mode === "inbox" && !readIds.has(mail.id)}
                                domainLabel={activeDomains.length > 1 ? domainOf(mail) : ""}
                                onclick={handleEmailClick}
                            />
                        {/each}

                        {#if activeList.hasMore}
                            <button
                                onclick={handleLoadMore}
                                disabled={activeList.isLoadingMore}
                                class="w-full p-4 text-sm text-center text-muted-foreground hover:bg-sidebar-accent transition-colors"
                            >
                                {#if activeList.isLoadingMore}
                                    <Loader
                                        class="size-4 animate-spin mx-auto"
                                    />
                                {:else}
                                    <span>Load more</span>
                                {/if}
                            </button>
                        {/if}
                    {/if}
                </Sidebar.GroupContent>
            </Sidebar.Group>
        </Sidebar.Content>
    </Sidebar.Root>
</Sidebar.Root>
