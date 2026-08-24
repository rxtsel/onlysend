<script lang="ts" module>
    import InboxIcon from "@lucide/svelte/icons/inbox";
    import SendIcon from "@lucide/svelte/icons/send";
    import PlusIcon from "@lucide/svelte/icons/plus";
    import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
    import SettingsDialog from "./settings-dialog.svelte";
</script>

<script lang="ts">
    import NavUser from "./nav-user.svelte";
    import { useSidebar } from "@/lib/components/ui/sidebar/context.svelte.js";
    import * as Sidebar from "@/lib/components/ui/sidebar/index.js";
    import { Button } from "@/lib/components/ui/button/index.js";
    import type { ComponentProps } from "svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/state";
    import { onMount } from "svelte";
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
    import { getOnboardingState } from "@/lib/shared/api/auth";
    import EmailListItem from "@/lib/features/inbox/components/email-list-item.svelte";
import { authErrorToast, isAuthError } from "@/lib/shared/services/auth-toast.svelte";
import { errorMessage } from "@/lib/shared/utils/errors";
    import { createEmailList } from "../shared/email-list.svelte";
    import { inboundStatus } from "../shared/inbound-status.svelte";

    // Live update when receiving becomes verified anywhere in the app.
    $effect(() => {
      if (inboundStatus.ready && !inboxEnabled) {
        inboxEnabled = true;
      }
    });

    let {
        ref = $bindable(null),
        ...restProps
    }: ComponentProps<typeof Sidebar.Root> = $props();

    /* ---------------------------------------------------------
     * MODE: which list the sidebar shows, driven by the route
     * --------------------------------------------------------- */
    type Mode = "inbox" | "sent";

    const mode = $derived(
        page.url.pathname.startsWith("/mail/inbox") ? ("inbox" as const) : ("sent" as const),
    );

    const data = {
        navMain: [
            { title: "Inbox", url: "/mail/inbox", icon: InboxIcon, mode: "inbox" as const },
            { title: "All Sent", url: "/mail/sent", icon: SendIcon, mode: "sent" as const },
        ],
    };

    const activeItem = $derived(
        data.navMain.find((item) => item.mode === mode) ?? data.navMain[0],
    );

    const sidebar = useSidebar();
    const PAGE_SIZE = 16;
    const POLL_INTERVAL_MS = 60_000;

    /* ---------------------------------------------------------
     * INBOX VISIBILITY (persisted flag, updated by observation
     * points in the wizard / inbox setup flow)
     * --------------------------------------------------------- */
    let inboxEnabled = $state(false);

    /* ---------------------------------------------------------
     * DOMAIN FILTER (unified views, client-side filtering)
     * --------------------------------------------------------- */
    let domainFilter = $state<"all" | string>("all");

    function dateOf(mail: InboundEmail | SentEmail): string {
        return mail.createdAt;
    }

    function domainOf(mail: InboundEmail | SentEmail): string {
        if ("domain" in mail) return mail.domain;
        return mail.from.split("@")[1]?.toLowerCase() ?? "";
    }
    const sentList = createEmailList<SentEmail>((limit, offset) =>
        listSentEmails(limit, offset),
    );

    const inboxList = createEmailList<InboundEmail>((limit, offset) =>
        listInboundEmails(limit, offset),
    );

    const activeList = $derived(mode === "inbox" ? inboxList : sentList);

    /* ---------------------------------------------------------
     * DOMAIN FILTER (unified views)
     * --------------------------------------------------------- */
    const activeDomains = $derived.by(() => {
        const set = new Set<string>();
        for (const mail of activeList.items as (InboundEmail | SentEmail)[]) {
            const d = domainOf(mail);
            if (d) set.add(d);
        }
        return [...set].sort();
    });

    const visibleItems = $derived(
        domainFilter === "all"
            ? (activeList.items as (InboundEmail | SentEmail)[])
            : (activeList.items as (InboundEmail | SentEmail)[]).filter(
                  (mail) => domainOf(mail) === domainFilter,
              ),
    );

    // Load each list once when its mode becomes active. The inbox list
    // only fetches once receiving is ready (the root page owns that
    // state); before that it stays empty and error-free.
    let lastLoadedMode = $state("");

    $effect(() => {
      if (mode !== lastLoadedMode) {
        lastLoadedMode = mode;
        domainFilter = "all";

        if (mode === "inbox" && !inboundStatus.ready) {
          inboxList.markLoaded();
          return;
        }

        activeList.refreshSilent().catch((err) => {
          console.error(`Error loading ${mode} emails:`, err);
          if (isAuthError(err)) {
            authErrorToast(err);
          } else {
            toast.error(errorMessage(err, "Failed to load emails"));
          }
          activeList.markLoaded();
        });
      }
    });

    // Re-fetch the inbox as soon as receiving becomes ready.
    $effect(() => {
      if (mode === "inbox" && inboundStatus.ready && lastLoadedMode === "inbox") {
        const alreadyFetched = inboxList.items.length > 0;
        if (!alreadyFetched) {
          inboxList.refreshSilent().catch(console.error);
        }
      }
    });

    async function handleRefresh() {
        if (mode === "inbox" && !inboundStatus.ready) {
            toast.info("Enable receiving first to get email into OnlySend.");
            return;
        }

        try {
            await activeList.refresh();
            toast.success("Emails refreshed");
        } catch (err) {
            if (isAuthError(err)) {
                authErrorToast(err);
            } else {
                toast.error(errorMessage(err, "Failed to load emails"));
            }
        }
    }

    function handleLoadMore() {
        activeList.loadMore();
    }

    /* ---------------------------------------------------------
     * INBOX READ MARKERS
     * --------------------------------------------------------- */
    let readIds = $state<Set<string>>(new Set());

    onMount(async () => {
        try {
            const [ids, state] = await Promise.all([
                getReadInboundIds(),
                getOnboardingState(),
            ]);
            readIds = new Set(ids);
            inboxEnabled = state.inboxEnabled;
        } catch (err) {
            console.error("Error loading inbox markers:", err);
        }
    });

    // Settings can flip this flag (Set up inbox flow); pick it up live.
    window.addEventListener("inbox-enabled-changed", ((e: CustomEvent<boolean>) => {
        inboxEnabled = e.detail;
    }) as EventListener);

    /** Nav items, with Inbox gated behind the receiving flag. */
    const navItems = $derived(
        data.navMain.filter((item) => item.mode !== "inbox" || inboxEnabled),
    );

    async function handleEmailClick(mailId: string) {
        if (mode === "inbox" && !readIds.has(mailId)) {
            readIds = new Set([...readIds, mailId]);
            markInboundRead(mailId).catch(console.error);
        }
        goto(`/mail/${mode}/${mailId}`);
    }

    /* ---------------------------------------------------------
     * INBOX POLLING (only while the inbox is visible)
     * --------------------------------------------------------- */
    onMount(() => {
        const interval = setInterval(async () => {
            if (
                mode !== "inbox" ||
                document.hidden ||
                !inboundStatus.ready ||
                !inboxList.items.length
            )
                return;

            try {
                const previousNewest = inboxList.items[0]?.id;
                await inboxList.refreshSilent();

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
                            href="/mail/composer"
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
                <!-- DOMAIN FILTER CHIPS -->
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
