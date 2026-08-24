<script lang="ts">
  import * as Breadcrumb from "@/lib/components/ui/breadcrumb";
  import { Button, buttonVariants } from "@/lib/components/ui/button";
  import * as Dialog from "@/lib/components/ui/dialog";
  import * as Sidebar from "@/lib/components/ui/sidebar";
  import {
    Mail,
    Settings,
    SquarePen,
    Trash,
    Plug,
    Star,
  } from "@lucide/svelte";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import * as InputGroup from "@/lib/components/ui/input-group";
  import * as Item from "@/lib/components/ui/item";
  import * as Empty from "@/lib/components/ui/empty";
  import EmailPreview from "@/lib/components/setup/email-preview.svelte";
  import type { FromEmail } from "../types";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { stripAt } from "../commom/email";
  import {
    createFromEmail,
    updateFromEmail,
    deleteFromEmail,
    listFromEmails,
  } from "../commom/from-emails";
  import { toast } from "svelte-sonner";
  import type { ZodError } from "zod/v4";
  import { emailOptionSchema } from "../schemas/email-option.schema";
  import {
    connectResend,
    disconnectResend,
    getActiveDomain,
    getConnectionStatus,
    type ConnectionStatus,
  } from "../commom/store";

  const data = {
    nav: [
      { name: "Sender options", icon: Mail },
      { name: "Connection", icon: Plug },
    ],
  };

  let open = $state(false);
  let activeItem = $state("Sender options");
  let emailErrors = $state<Record<string, string>>({});
  let isSaving = $state(false);
  let isDeleteDialogOpen = $state(false);

  // Active sender identity
  let activeDomain = $state<string | null>(null);

  // Connection state
  let connection = $state<ConnectionStatus>({ method: null });
  let isConnecting = $state(false);
  let isDisconnecting = $state(false);

  // From Email form state
  let emailLabel = $state("");
  let emailAddress = $state("");
  let isEditingFromEmail = $state(false);
  let editingId = $state("");

  let fromEmails = $state<FromEmail[]>([]);

  function handleZodError(error: ZodError): Record<string, string> {
    const errors: Record<string, string> = {};
    error.issues.forEach((err) => {
      errors[err.path.join(".")] = err.message;
    });
    return errors;
  }

  async function loadData() {
    try {
      const [emails, domain, status] = await Promise.all([
        listFromEmails(),
        getActiveDomain(),
        getConnectionStatus(),
      ]);
      fromEmails = emails;
      activeDomain = domain;
      connection = status;
    } catch (error) {
      console.error("Error loading data:", error);
      toast.error("Failed to load data");
    }
  }

  /* ---------------------------------------------------------
   * FROM EMAILS
   * --------------------------------------------------------- */
  function resetForm() {
    emailLabel = "";
    emailAddress = "";
    isEditingFromEmail = false;
    editingId = "";
    emailErrors = {};
  }

  function startEdit(email: FromEmail) {
    isEditingFromEmail = true;
    editingId = email.id;
    emailLabel = email.label;

    // Only the local part goes into the input; the domain is the addon.
    emailAddress = activeDomain && email.address.endsWith(`@${activeDomain}`)
      ? email.address.slice(0, -(`@${activeDomain}`).length)
      : email.address.split("@")[0] ?? "";

    emailErrors = {};
  }

  async function handleAddOrUpdate(e: Event) {
    e.preventDefault();
    emailErrors = {};
    isSaving = true;

    try {
      const { local, typedDomain } = stripAt(emailAddress.trim());
      if (local) emailAddress = local;

      if (typedDomain && activeDomain && typedDomain !== activeDomain.toLowerCase()) {
        toast.info(
          `Using your domain ${activeDomain} instead of ${typedDomain}.`,
        );
      }

      const fullAddress = `${local}@${activeDomain}`;
      const validated = emailOptionSchema.parse({
        label: emailLabel,
        address: fullAddress,
      });

      if (isEditingFromEmail && editingId) {
        await updateFromEmail({
          id: editingId,
          label: validated.label,
          address: validated.address,
          isDefault: isDefault(editingId),
        });
        toast.success("Email option updated");
      } else {
        await createFromEmail({
          label: validated.label,
          address: validated.address,
          isDefault: fromEmails.length === 0,
        });
        toast.success("Email option created");
      }

      await loadData();
      resetForm();
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

  function isDefault(id: string): boolean {
    return fromEmails.find((e) => e.id === id)?.isDefault ?? false;
  }

  async function setDefault(id: string) {
    try {
      await updateFromEmail({ id, isDefault: true });
      toast.success("Default sender updated");
      await loadData();
    } catch (error) {
      console.error(error);
      toast.error("Failed to update default");
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteFromEmail(id);
      toast.success("Email option deleted");
      if (editingId === id) resetForm();
      await loadData();
    } catch (error) {
      console.error(error);
      toast.error("Failed to delete email option");
    }
  }

  /* ---------------------------------------------------------
   * CONNECTION
   * --------------------------------------------------------- */
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

  onMount(() => {
    let unlisten: (() => void) | undefined;

    loadData();

    listen<{ success: boolean; warning?: string; error?: string }>(
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
          {#if activeItem === "Sender options"}
            {@render senderOptions()}
          {:else if activeItem === "Connection"}
            {@render connectionSection()}
          {/if}
        </div>
      </main>
    </Sidebar.Provider>
  </Dialog.Content>
</Dialog.Root>

{#snippet senderOptions()}
  <form class="w-full" onsubmit={handleAddOrUpdate}>
    <Field.Group>
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-medium">
          {isEditingFromEmail ? "Edit Email Option" : "Add New Email Option"}
        </h3>
        {#if activeDomain}
          <span
            class="font-mono text-xs px-2 py-0.5 rounded bg-muted text-muted-foreground"
            title="Active domain"
          >
            @{activeDomain}
          </span>
        {/if}
      </div>

      <Field.Field>
        <Field.Label for="label">Your Name</Field.Label>
        <Input
          id="label"
          name="label"
          bind:value={emailLabel}
          placeholder="Your Name"
          required
          aria-invalid={!!emailErrors.label}
        />
        {#if emailErrors.label}
          <Field.Error>{emailErrors.label}</Field.Error>
        {/if}
      </Field.Field>

      <Field.Field>
        <Field.Label for="address">Address</Field.Label>
        <InputGroup.Root>
          <InputGroup.Input
            placeholder="hello"
            bind:value={emailAddress}
            aria-invalid={!!emailErrors.address}
          />
          <InputGroup.Addon align="inline-end">
            <InputGroup.Text>@{activeDomain ?? "domain"}</InputGroup.Text>
          </InputGroup.Addon>
        </InputGroup.Root>
        {#if emailErrors.address}
          <Field.Error>{emailErrors.address}</Field.Error>
        {:else}
          <Field.Description>
            Just the part before the @. Click a name or value below to copy.
          </Field.Description>
        {/if}
      </Field.Field>

      <!-- LIVE PREVIEW -->
      <div class="mb-4">
        <EmailPreview
          label={emailLabel}
          localPart={stripAt(emailAddress).local}
          domain={activeDomain ?? ""}
        />
      </div>

      <div class="flex gap-2 mb-4">
        {#if isEditingFromEmail}
          <Button
            type="button"
            variant="outline"
            class="flex-1"
            onclick={resetForm}
          >
            Cancel Edit
          </Button>
          <Button type="submit" class="flex-1" disabled={isSaving}>
            Update
          </Button>
        {:else}
          <Button type="submit" class="w-full" disabled={isSaving}>
            Add email
          </Button>
        {/if}
      </div>
    </Field.Group>
  </form>

  <!-- SAVED OPTIONS -->
  {#if fromEmails.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon">
          <Mail />
        </Empty.Media>
        <Empty.Title>No sender options yet</Empty.Title>
        <Empty.Description>
          Add your first identity above to start sending emails.
        </Empty.Description>
      </Empty.Header>
    </Empty.Root>
  {:else}
    <Item.Group>
      {#each fromEmails as fromEmail, i (fromEmail.id)}
        <Item.Root variant="outline" size="sm">
          <Item.Content>
            <Item.Title>{fromEmail.label}</Item.Title>
            <Item.Description>{fromEmail.address}</Item.Description>
          </Item.Content>
          <Item.Actions>
            {#if fromEmail.isDefault}
              <span title="Default sender">
                <Star class="size-4 fill-yellow-400 stroke-yellow-400" />
              </span>
            {:else}
              <Button
                variant="ghost"
                size="icon-sm"
                title="Set as default"
                onclick={() => setDefault(fromEmail.id)}
              >
                <Star />
              </Button>
            {/if}
            <Button
              variant="ghost"
              size="icon-sm"
              title="Edit"
              onclick={() => startEdit(fromEmail)}
            >
              <SquarePen />
            </Button>
            {@render confirmDelete(fromEmail.id)}
          </Item.Actions>
        </Item.Root>
        {#if i !== fromEmails.length - 1}
          <Item.Separator />
        {/if}
      {/each}
    </Item.Group>
  {/if}
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
          onclick={() => handleDelete(fromEmailId)}
        >
          Continue
        </AlertDialog.Action>
      </AlertDialog.Footer>
    </AlertDialog.Content>
  </AlertDialog.Root>
{/snippet}
