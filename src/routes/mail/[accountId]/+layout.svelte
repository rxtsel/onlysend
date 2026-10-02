<script lang="ts">
    import { page } from "$app/state";
    import AppSidebar from "@/lib/components/app-sidebar.svelte";
    import * as Breadcrumb from "@/lib/components/ui/breadcrumb";
    import { Separator } from "@/lib/components/ui/separator";
    import * as Sidebar from "@/lib/components/ui/sidebar";
    import { Send, Loader } from "@lucide/svelte";
    import { accountSwitch } from "@/lib/features/auth/account-switch.svelte";
    import * as AlertDialog from "$lib/components/ui/alert-dialog";
    import { buttonVariants } from "$lib/components/ui/button";

    import { provideAccount } from "$lib/features/auth/account-context";
    import { mailUrl } from "$lib/features/auth/mail-routes";

    let { children, data } = $props();
    const accountId = $derived(data.accountId);
    provideAccount(() => accountId);
    const sentHref = $derived(mailUrl(accountId, "sent"));
    const inboxHref = $derived(mailUrl(accountId, "inbox"));

    let isConfirmDialogOpen = $state(false);

    const pathname = $derived(page.url.pathname);

    // Route helpers
    const isSentRoot = $derived(pathname === sentHref);
    const isSentDetail = $derived(pathname.startsWith(`${sentHref}/`));
    const isComposer = $derived(pathname === mailUrl(accountId, "composer"));
    const isInboxRoot = $derived(pathname === inboxHref);
    const isInboxDetail = $derived(pathname.startsWith(`${inboxHref}/`));
    const isMailView = $derived(
      isSentRoot || isSentDetail || isInboxRoot || isInboxDetail,
    );

    // Detail IDs are always interpreted within the explicit route account.
    const emailId = $derived(
      isSentDetail
        ? pathname.slice(sentHref.length + 1)
        : isInboxDetail
          ? pathname.slice(inboxHref.length + 1)
          : "",
    );
    const listTitle = $derived(isInboxRoot || isInboxDetail ? "Inbox" : "All sent");
    const listHref = $derived(isInboxRoot || isInboxDetail ? inboxHref : sentHref);
</script>

{#if accountSwitch.busy}
    <div class="flex h-screen items-center justify-center gap-2" role="status" aria-live="polite">
        <Loader class="animate-spin" aria-hidden="true" /> Switching account…
    </div>
{:else}
{#key `${accountId}:${accountSwitch.generation}`}
<Sidebar.Provider style="--sidebar-width: 450px;" open={!isComposer}>
    <AppSidebar />
    <Sidebar.Inset>
        <header
            class="bg-background z-10 sticky top-0 flex shrink-0 items-center gap-2 border-b px-4 py-4.5 max-h-[65px] h-[65px]"
        >
            {#if isMailView}
                <Sidebar.Trigger class="-ms-1" />

                <Separator
                    orientation="vertical"
                    class="me-2 data-[orientation=vertical]:h-4"
                />

                <Breadcrumb.Root>
                    <Breadcrumb.List>
                        <Breadcrumb.Item class="hidden md:block">
                            {#if isInboxRoot || isSentRoot}
                                <!-- On the list root show it as current page -->
                                <Breadcrumb.Page>{listTitle}</Breadcrumb.Page>
                            {:else}
                                <!-- On detail use it as link -->
                                <Breadcrumb.Link href={listHref}
                                    >{listTitle}</Breadcrumb.Link
                                >
                            {/if}
                        </Breadcrumb.Item>

                        {#if emailId}
                            <Breadcrumb.Separator class="hidden md:block" />
                            <Breadcrumb.Item>
                                <!-- Only show the id, nothing else -->
                                <Breadcrumb.Page>
                                    ...{emailId.slice(-4)}
                                </Breadcrumb.Page>
                            </Breadcrumb.Item>
                        {/if}
                    </Breadcrumb.List>
                </Breadcrumb.Root>
            {:else if isComposer}
                <div
                    class="flex flex-1 min-w-0 justify-between items-center gap-x-2"
                >
                    <h2 class="text-lg font-semibold">New Email</h2>

                    {@render ConfirmDialog()}
                </div>
            {/if}
        </header>

        <div class="flex flex-1 flex-col gap-4 p-4">
            {#key page.params.id ?? "root"}
                {@render children()}
            {/key}
        </div>
    </Sidebar.Inset>
</Sidebar.Provider>
{/key}
{/if}

{#snippet ConfirmDialog()}
    <AlertDialog.Root bind:open={isConfirmDialogOpen}>
        <AlertDialog.Trigger class={buttonVariants({ variant: "default" })}>
            <Send />
            Send
        </AlertDialog.Trigger>
        <AlertDialog.Content>
            <AlertDialog.Header>
                <AlertDialog.Title>Are you absolutely sure?</AlertDialog.Title>
                <AlertDialog.Description>
                    This action cannot be undone. This will permanently send
                    your email.
                </AlertDialog.Description>
            </AlertDialog.Header>
            <AlertDialog.Footer>
                <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
                <AlertDialog.Action
                    form="compose-email-form"
                    type="submit"
                    onclick={() => (isConfirmDialogOpen = false)}
                >
                    Continue
                </AlertDialog.Action>
            </AlertDialog.Footer>
        </AlertDialog.Content>
    </AlertDialog.Root>
{/snippet}
