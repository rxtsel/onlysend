<script lang="ts">
  import * as Breadcrumb from "@/lib/components/ui/breadcrumb";
  import { Button, buttonVariants } from "@/lib/components/ui/button";
  import * as Dialog from "@/lib/components/ui/dialog";
  import * as Sidebar from "@/lib/components/ui/sidebar";
  import { Mail, User, Settings, SquarePen, Trash, Plug } from "@lucide/svelte";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import type { FromEmail, Profile } from "../types";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    createFromEmail,
    updateFromEmail,
    deleteFromEmail,
    formatFromEmail,
    listFromEmails,
  } from "../commom/from-emails";
  import { getProfile, saveProfile } from "../commom/profile";
  import { toast } from "svelte-sonner";
  import type { ZodError } from "zod/v4";
  import { profileSchema } from "../schemas/profile.schema";
  import { emailOptionSchema } from "../schemas/email-option.schema";
  import {
    connectResend,
    disconnectResend,
    getConnectionStatus,
    type ConnectionStatus,
  } from "../commom/store";

  const data = {
    nav: [
      { name: "Profile", icon: User },
      { name: "Email sender options", icon: Mail },
      { name: "Connection", icon: Plug },
    ],
  };

  let open = $state(false);
  let activeItem = $state("Profile");
  let profileErrors = $state<Record<string, string>>({});
  let emailErrors = $state<Record<string, string>>({});
  let isSaving = $state(false);
  let isDeleteDialogOpen = $state(false);

  // From Email form state
  let fromEmailForm = $state({
    id: "",
    label: "",
    address: "",
    isDefault: false,
  });
  let isEditingFromEmail = $state(false);

  let fromEmails = $state<FromEmail[]>([]);
  let profile = $state<Profile>({
    firstName: "",
    lastName: "",
    username: "",
    domain: "",
  });
  let connection = $state<ConnectionStatus>({ method: null });
  let isConnecting = $state(false);
  let isDisconnecting = $state(false);

  function handleZodError(error: ZodError): Record<string, string> {
    const errors: Record<string, string> = {};
    error.issues.forEach((err) => {
      const path = err.path.join(".");
      errors[path] = err.message;
    });
    return errors;
  }

  async function loadData() {
    try {
      const [profileData, fromEmailsData, connectionData] = await Promise.all([
        getProfile(),
        listFromEmails(),
        getConnectionStatus(),
      ]);
      if (profileData) {
        profile = profileData;
      }
      fromEmails = fromEmailsData;
      connection = connectionData;
    } catch (error) {
      console.error("Error loading data:", error);
      toast.error("Failed to load data");
    }
  }

  // Connection handlers
  async function handleConnectResend(e: Event) {
    e.preventDefault();
    try {
      isConnecting = true;
      await connectResend();
    } catch (error) {
      isConnecting = false;
      console.error("Error connecting with Resend:", error);
      toast.error("Failed to start the connection");
    }
  }

  async function handleDisconnect(e: Event) {
    e.preventDefault();
    try {
      isDisconnecting = true;
      await disconnectResend();
      toast.success("Disconnected from Resend");
      await loadData();
    } catch (error) {
      console.error("Error disconnecting:", error);
      toast.error("Failed to disconnect");
    } finally {
      isDisconnecting = false;
    }
  }

  async function refreshConnection() {
    try {
      connection = await getConnectionStatus();
      isConnecting = false;

      if (connection.method === "oauth") {
        toast.success("Connected with Resend");
      }
    } catch (error) {
      console.error("Error refreshing connection status:", error);
    }
  }

  // Profile handlers
  async function handleSaveProfile(e: Event) {
    e.preventDefault();
    profileErrors = {};
    isSaving = true;

    try {
      // Validate with Zod
      const validatedData = profileSchema.parse(profile);

      await saveProfile(validatedData);
      toast.success("Profile saved successfully");
    } catch (error) {
      if (error instanceof Error && error.name === "ZodError") {
        profileErrors = handleZodError(error as ZodError);
      } else {
        console.error("Error saving profile:", error);
        toast.error("Failed to save profile");
      }
    } finally {
      isSaving = false;
    }
  }

  // From Email handlers
  function resetFromEmailForm() {
    fromEmailForm = {
      id: "",
      label: "",
      address: "",
      isDefault: false,
    };
    isEditingFromEmail = false;
    emailErrors = {};
  }

  async function handleSubmitFromEmail(e: Event) {
    e.preventDefault();
    emailErrors = {};
    isSaving = true;

    try {
      // Validate with Zod
      const validatedData = emailOptionSchema.parse({
        label: fromEmailForm.label,
        address: fromEmailForm.address,
      });

      if (isEditingFromEmail && fromEmailForm.id) {
        // Update existing
        await updateFromEmail({
          id: fromEmailForm.id,
          label: validatedData.label,
          address: validatedData.address,
          isDefault: fromEmailForm.isDefault,
        });
        toast.success("Email option updated successfully");
      } else {
        // Create new
        await createFromEmail({
          label: validatedData.label,
          address: validatedData.address,
          isDefault: fromEmailForm.isDefault,
        });
        toast.success("Email option created successfully");
      }

      // Reload data and reset form
      await loadData();
      resetFromEmailForm();
    } catch (error) {
      if (error instanceof Error && error.name === "ZodError") {
        emailErrors = handleZodError(error as ZodError);
      } else {
        console.error("Error saving from email:", error);
        toast.error("Failed to save email option");
      }
    } finally {
      isSaving = false;
    }
  }

  function handleEditFromEmail(email: FromEmail) {
    fromEmailForm = {
      id: email.id,
      label: email.label,
      address: email.address,
      isDefault: email.isDefault,
    };
    isEditingFromEmail = true;
    emailErrors = {};
  }

  async function handleDeleteFromEmail(id: string) {
    try {
      await deleteFromEmail(id);
      toast.success("Email option deleted successfully");
      await loadData();
    } catch (error) {
      console.error("Error deleting from email:", error);
      toast.error("Failed to delete email option");
    }
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;

    loadData();

    listen<{ success: boolean; error?: string }>(
      "oauth://done",
      (event) => {
        if (event.payload.success) {
          refreshConnection();
        } else {
          isConnecting = false;
          toast.error(event.payload.error ?? "Failed to connect with Resend");
        }
      },
    ).then((fn) => (unlisten = fn));

    return () => unlisten?.();
  });
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
      <Sidebar.Root collapsible="none" class="hidden md:flex">
        <Sidebar.Content>
          <Sidebar.Group>
            <Sidebar.GroupContent>
              <Sidebar.Menu>
                {#each data.nav as item (item.name)}
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
          {#if activeItem === "Profile"}
            {@render profileComponent()}
          {:else if activeItem === "Email sender options"}
            {@render emailSenderOptions()}
          {:else if activeItem === "Connection"}
            {@render connectionSection()}
          {/if}
        </div>
      </main>
    </Sidebar.Provider>
  </Dialog.Content>
</Dialog.Root>

{#snippet profileComponent()}
  <form class="w-full" onsubmit={handleSaveProfile}>
    <Field.Group>
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-x-2">
        <Field.Field>
          <Field.Label for="firstName">First Name</Field.Label>
          <Input
            id="firstName"
            name="firstName"
            bind:value={profile.firstName}
            placeholder="John"
            required
            aria-invalid={!!profileErrors.firstName}
          />
          {#if profileErrors.firstName}
            <Field.Error>{profileErrors.firstName}</Field.Error>
          {/if}
        </Field.Field>

        <Field.Field>
          <Field.Label for="lastName">Last Name</Field.Label>
          <Input
            id="lastName"
            name="lastName"
            bind:value={profile.lastName}
            placeholder="Doe"
            required
            aria-invalid={!!profileErrors.lastName}
          />
          {#if profileErrors.lastName}
            <Field.Error>{profileErrors.lastName}</Field.Error>
          {/if}
        </Field.Field>
      </div>

      <Field.Field>
        <Field.Label for="username">Username</Field.Label>
        <Input
          id="username"
          name="username"
          bind:value={profile.username}
          placeholder="john_doe"
          required
          aria-invalid={!!profileErrors.username}
        />
        {#if profileErrors.username}
          <Field.Error>{profileErrors.username}</Field.Error>
        {/if}
        <Field.Description>
          Only letters, numbers, and underscores (3-30 characters)
        </Field.Description>
      </Field.Field>

      <Field.Field>
        <Field.Label for="domain">Domain</Field.Label>
        <Input
          id="domain"
          name="domain"
          bind:value={profile.domain}
          placeholder="example.com"
          required
          aria-invalid={!!profileErrors.domain}
        />
        {#if profileErrors.domain}
          <Field.Error>{profileErrors.domain}</Field.Error>
        {/if}
        <Field.Description>
          Without protocol (e.g., example.com)
        </Field.Description>
      </Field.Field>

      <Field.Field>
        <Button type="submit" disabled={isSaving} class="w-full">
          {isSaving ? "Saving..." : "Save Changes"}
        </Button>
      </Field.Field>
    </Field.Group>
  </form>
{/snippet}

{#snippet emailSenderOptions()}
  <form class="w-full" onsubmit={handleSubmitFromEmail}>
    <Field.Group>
      <div class="flex justify-between items-center mb-2">
        <h3 class="text-sm font-medium">
          {isEditingFromEmail ? "Edit Email Option" : "Add New Email Option"}
        </h3>
        {#if isEditingFromEmail}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onclick={resetFromEmailForm}
          >
            Cancel Edit
          </Button>
        {/if}
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 gap-x-2">
        <Field.Field>
          <Field.Label for="label">Label</Field.Label>
          <Input
            id="label"
            name="label"
            bind:value={fromEmailForm.label}
            placeholder="Contact"
            required
            aria-invalid={!!emailErrors.label}
          />
          {#if emailErrors.label}
            <Field.Error>{emailErrors.label}</Field.Error>
          {/if}
        </Field.Field>

        <Field.Field>
          <Field.Label for="address">Address</Field.Label>
          <Input
            id="address"
            name="address"
            type="email"
            bind:value={fromEmailForm.address}
            placeholder="contact@domain.com"
            required
            aria-invalid={!!emailErrors.address}
          />
          {#if emailErrors.address}
            <Field.Error>{emailErrors.address}</Field.Error>
          {/if}
        </Field.Field>
      </div>

      <Field.Field>
        <label class="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            bind:checked={fromEmailForm.isDefault}
            class="rounded border-gray-300"
          />
          Mark as default
        </label>
      </Field.Field>

      <Field.Field>
        <Button type="submit" disabled={isSaving} class="w-full">
          {isSaving ? "Saving..." : isEditingFromEmail ? "Update" : "Create"}
        </Button>
      </Field.Field>
    </Field.Group>
  </form>

  <div class="mt-6">
    <h3 class="text-sm font-medium mb-2">Saved Email Options</h3>
    {#if fromEmails.length === 0}
      <p class="text-sm text-muted-foreground text-center py-4">
        No email options configured yet
      </p>
    {:else}
      <table
        class="w-full table-auto border-collapse border border-border rounded-md"
      >
        <thead>
          <tr class="bg-muted/50">
            <th class="border border-border p-2 text-xs text-left"
              >From Email</th
            >
            <th class="border border-border p-2 text-xs text-left w-24"
              >Actions</th
            >
          </tr>
        </thead>
        <tbody>
          {#each fromEmails as fromEmail (fromEmail.id)}
            <tr class="hover:bg-muted/30">
              <td class="border border-border px-4 py-2">
                <div class="flex items-center gap-2">
                  <span class="text-xs">{formatFromEmail(fromEmail)}</span>
                  {#if fromEmail.isDefault}
                    <span
                      class="text-[10px] bg-primary/10 text-primary px-2 py-0.5 rounded"
                    >
                      Default
                    </span>
                  {/if}
                </div>
              </td>
              <td class="border border-border px-4 py-2">
                <div class="flex justify-end gap-1">
                  <Button
                    variant="outline"
                    size="icon-sm"
                    onclick={() => handleEditFromEmail(fromEmail)}
                    title="Edit"
                  >
                    <SquarePen />
                  </Button>
                  {@render confirmDelete(fromEmail.id)}
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
{/snippet}

{#snippet connectionSection()}
  <div class="w-full max-w-md">
    <h3 class="text-sm font-medium mb-2">Resend account</h3>
    {#if connection.method === "oauth"}
      <div class="flex items-center justify-between border rounded-md px-4 py-3">
        <div>
          <p class="text-sm">Connected with Resend</p>
          <p class="text-xs text-muted-foreground">
            Authorized via OAuth. Tokens refresh automatically.
          </p>
        </div>
        <Button
          type="button"
          variant="destructive"
          size="sm"
          disabled={isDisconnecting}
          onclick={handleDisconnect}
        >
          {isDisconnecting ? "Disconnecting..." : "Disconnect"}
        </Button>
      </div>
    {:else if connection.method === "api_key"}
      <div class="border rounded-md px-4 py-3">
        <p class="text-sm">Connected with an API key</p>
        <p class="text-xs text-muted-foreground mb-3">
          You can also connect your account securely via OAuth.
        </p>
        <Button
          type="button"
          size="sm"
          disabled={isConnecting}
          onclick={handleConnectResend}
        >
          {isConnecting ? "Waiting for authorization..." : "Connect with Resend"}
        </Button>
      </div>
    {:else}
      <div class="border rounded-md px-4 py-3">
        <p class="text-sm text-muted-foreground mb-3">Not connected.</p>
        <Button
          type="button"
          size="sm"
          disabled={isConnecting}
          onclick={handleConnectResend}
        >
          {isConnecting ? "Waiting for authorization..." : "Connect with Resend"}
        </Button>
      </div>
    {/if}
  </div>
{/snippet}

{#snippet confirmDelete(fromEmailId: string)}
  <AlertDialog.Root>
    <AlertDialog.Trigger>
      <Button
        variant="destructive"
        size="icon-sm"
        onclick={() => (isDeleteDialogOpen = true)}
        title="Delete"
      >
        <Trash />
      </Button>
    </AlertDialog.Trigger>
    <AlertDialog.Content>
      <AlertDialog.Header>
        <AlertDialog.Title>Are you absolutely sure?</AlertDialog.Title>
        <AlertDialog.Description>
          This action cannot be undone. This will permanently delete item from
          storage.
        </AlertDialog.Description>
      </AlertDialog.Header>
      <AlertDialog.Footer>
        <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
        <AlertDialog.Action
          class={buttonVariants({ variant: "destructive" })}
          onclick={() => handleDeleteFromEmail(fromEmailId)}
        >
          Continue
        </AlertDialog.Action>
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>
{/snippet}
