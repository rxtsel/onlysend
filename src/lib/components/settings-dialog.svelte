<script lang="ts">
  import * as Breadcrumb from "@/lib/components/ui/breadcrumb";
  import * as Dialog from "@/lib/components/ui/dialog";
  import * as Sidebar from "@/lib/components/ui/sidebar";
  import { Mail, Settings, Plug, Globe, Info, Bell } from "@lucide/svelte";
  import * as Select from "$lib/components/ui/select";
  const panels = {
    "Sender options": () => import("$lib/features/sending/components/settings-sender-options.svelte"),
    "Domains & receiving": () => import("$lib/features/domains/components/settings-domains.svelte"),
    Notifications: () => import("$lib/components/settings-notifications.svelte"),
    About: () => import("$lib/components/settings-about.svelte"),
  };

  const nav = [
    { name: "Sender options", icon: Mail },
    { name: "Connection", icon: Plug },
    { name: "Domains & receiving", icon: Globe },
    { name: "Notifications", icon: Bell },
    { name: "About", icon: Info },
  ];

  let open = $state(false);
  let activeItem = $state(nav[0].name);

  // Other components can request the dialog programmatically:
  //   window.dispatchEvent(new CustomEvent("open-settings", { detail: { section: "Connection" } }))
  function handleOpenSettings(e: Event) {
    const requested = (e as CustomEvent<{ section?: string }>).detail?.section;
    const section = requested === "Domains" ? "Domains & receiving" : requested;
    if (section && nav.some((item) => item.name === section)) {
      activeItem = section;
    }
    open = true;
  }

  $effect(() => {
    const handler = handleOpenSettings;
    window.addEventListener("open-settings", handler as EventListener);
    return () =>
      window.removeEventListener("open-settings", handler as EventListener);
  });
</script>

<Dialog.Root bind:open>
  <Dialog.Trigger>
    {#snippet child({ props })}
      <Sidebar.MenuButton {...props} class="h-10 gap-4" aria-label="Settings" tooltipContent="Settings">
        <Settings /><span class="group-data-[collapsible=icon]:hidden">Settings</span>
      </Sidebar.MenuButton>
    {/snippet}
  </Dialog.Trigger>
  <Dialog.Content
    class="overflow-hidden p-0 md:max-h-[500px] md:max-w-[700px] lg:max-w-[800px]"
    trapFocus={false}
  >
    <Dialog.Title class="sr-only">Settings</Dialog.Title>
    <Dialog.Description class="sr-only"
      >Customize your settings here.</Dialog.Description
    >
    <Sidebar.Provider class="items-start">
      <Sidebar.Root
        collapsible="none"
        class="hidden md:flex"
        style="--sidebar-width: 11rem"
      >
        <Sidebar.Content>
          <Sidebar.Group>
            <Sidebar.GroupContent>
              <Sidebar.Menu>
                {#each nav as item (item.name)}
                  <Sidebar.MenuItem>
                    <Sidebar.MenuButton isActive={item.name === activeItem}>
                      {#snippet child({ props })}
                        <button
                          {...props}
                          onclick={() => (activeItem = item.name)}
                        >
                          <item.icon />
                          <span>{item.name}</span>
                        </button>
                      {/snippet}
                    </Sidebar.MenuButton>
                  </Sidebar.MenuItem>
                {/each}
              </Sidebar.Menu>
            </Sidebar.GroupContent>
          </Sidebar.Group>
        </Sidebar.Content>
      </Sidebar.Root>
      <main class="flex h-[480px] flex-1 flex-col overflow-hidden">
        <header
          class="flex h-16 shrink-0 items-center gap-2 transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-12"
        >
          <div class="flex min-w-0 flex-1 items-center gap-2 px-4">
            <div class="hidden md:block">
            <Breadcrumb.Root>
              <Breadcrumb.List>
                <Breadcrumb.Item class="hidden md:block">
                  <Breadcrumb.Page>Settings</Breadcrumb.Page>
                </Breadcrumb.Item>
                <Breadcrumb.Separator class="hidden md:block" />
                <Breadcrumb.Item>
                  <Breadcrumb.Page>{activeItem}</Breadcrumb.Page>
                </Breadcrumb.Item>
              </Breadcrumb.List>
            </Breadcrumb.Root>
            </div>
            <div class="min-w-0 w-full md:hidden">
              <Select.Root type="single" bind:value={activeItem}>
                <Select.Trigger aria-label="Settings section">{activeItem}</Select.Trigger>
                <Select.Content>
                  <Select.Group>
                    {#each nav as item (item.name)}<Select.Item value={item.name}>{item.name}</Select.Item>{/each}
                  </Select.Group>
                </Select.Content>
              </Select.Root>
            </div>
          </div>
        </header>
        <div class="flex flex-1 flex-col gap-4 overflow-y-auto p-4 pt-0">
          {#if open}
            {#if activeItem === "Connection"}
              {#await import("$lib/features/auth/components/settings-connection.svelte")}
                <p class="text-sm text-muted-foreground" role="status">Loading…</p>
              {:then { default: Connection }}
                <Connection onRequestClose={() => (open = false)} />
              {:catch}
                <p class="text-sm text-destructive" role="alert">Couldn't load settings. Close and reopen to try again.</p>
              {/await}
            {:else}
              {#await panels[activeItem as keyof typeof panels]()}
                <p class="text-sm text-muted-foreground" role="status">Loading…</p>
              {:then { default: Panel }}
                <Panel />
              {:catch}
                <p class="text-sm text-destructive" role="alert">Couldn't load settings. Close and reopen to try again.</p>
              {/await}
            {/if}
          {/if}
        </div>
      </main>
    </Sidebar.Provider>
  </Dialog.Content>
</Dialog.Root>

