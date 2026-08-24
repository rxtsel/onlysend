<script lang="ts">
  import * as Breadcrumb from "@/lib/components/ui/breadcrumb";
  import * as Dialog from "@/lib/components/ui/dialog";
  import * as Sidebar from "@/lib/components/ui/sidebar";
  import { Button } from "@/lib/components/ui/button";
  import { Mail, Settings, Plug } from "@lucide/svelte";
  import SettingsSenderOptions from "@/lib/features/sending/components/settings-sender-options.svelte";
  import SettingsConnection from "@/lib/features/auth/components/settings-connection.svelte";

  const nav = [
    { name: "Sender options", icon: Mail },
    { name: "Connection", icon: Plug },
  ];

  let open = $state(false);
  let activeItem = $state(nav[0].name);
</script>

<Dialog.Root bind:open>
  <Dialog.Trigger>
    {#snippet child({ props })}
      <Button size="icon-sm" variant="ghost" {...props}>
        <Settings />
      </Button>
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
          <div class="flex items-center gap-2 px-4">
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
        </header>
        <div class="flex flex-1 flex-col gap-4 overflow-y-auto p-4 pt-0 mr-7">
          {#if activeItem === "Sender options"}
            <SettingsSenderOptions />
          {:else if activeItem === "Connection"}
            <SettingsConnection onRequestClose={() => (open = false)} />
          {/if}
        </div>
      </main>
    </Sidebar.Provider>
  </Dialog.Content>
</Dialog.Root>

