<script lang="ts">
  import { toast } from "svelte-sonner";
  import type { ZodError } from "zod/v4";

  import { Button, buttonVariants } from "@/lib/components/ui/button";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import * as InputGroup from "@/lib/components/ui/input-group";
  import * as Item from "@/lib/components/ui/item";
  import * as Empty from "@/lib/components/ui/empty";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import EmailPreview from "@/lib/features/setup/components/email-preview.svelte";
  import { Blobatar } from "@blobatar/svelte";
  import {
    Mail,
    SquarePen,
    Star,
    Trash,
  } from "@lucide/svelte";
  import type { FromEmail } from "../../../types";
  import { stripAt } from "../../../shared/email";
  import {
    createFromEmail,
    updateFromEmail,
    deleteFromEmail,
    listFromEmails,
  } from "../../../shared/from-emails";
  import { emailOptionSchema } from "../../../schemas/email-option.schema";
  import { getActiveDomain } from "@/lib/shared/api/domains";
  import { onMount } from "svelte";

  let activeDomain = $state<string | null>(null);
  let fromEmails = $state<FromEmail[]>([]);

  // Form state
  let emailLabel = $state("");
  let emailAddress = $state("");
  let emailErrors = $state<Record<string, string>>({});
  let isSaving = $state(false);
  let isEditingFromEmail = $state(false);
  let editingId = $state("");
  let isDeleteDialogOpen = $state(false);

  function handleZodError(error: ZodError): Record<string, string> {
    const errors: Record<string, string> = {};
    error.issues.forEach((err) => {
      errors[err.path.join(".")] = err.message;
    });
    return errors;
  }

  async function loadData() {
    try {
      const [emails, domain] = await Promise.all([
        listFromEmails(),
        getActiveDomain(),
      ]);
      fromEmails = emails;
      activeDomain = domain;
    } catch (error) {
      console.error("Error loading sender options:", error);
      toast.error("Failed to load sender options");
    }
  }

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
    emailAddress =
      activeDomain && email.address.endsWith(`@${activeDomain}`)
        ? email.address.slice(0, -(`@${activeDomain}`).length)
        : (email.address.split("@")[0] ?? "");

    emailErrors = {};
  }

  function isDefault(id: string): boolean {
    return fromEmails.find((e) => e.id === id)?.isDefault ?? false;
  }

  async function handleAddOrUpdate(e: Event) {
    e.preventDefault();
    emailErrors = {};
    isSaving = true;

    try {
      const { local, typedDomain } = stripAt(emailAddress.trim());
      if (local) emailAddress = local;

      if (
        typedDomain &&
        activeDomain &&
        typedDomain !== activeDomain.toLowerCase()
      ) {
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

  onMount(() => {
    loadData();
  });
</script>

<form class="w-full" onsubmit={handleAddOrUpdate}>
  <Field.Group>
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-base font-medium">
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

    <div class="flex flex-col @min-sm:flex-row gap-2 mb-4">
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
        {/if}
      </Field.Field>
    </div>

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
        <Item.Media>
          <Blobatar
            name={fromEmail.address}
            size={28}
            class="rounded-full shrink-0"
          />
        </Item.Media>
        <Item.Content class="text-left">
          <Item.Title class="text-left">{fromEmail.label}</Item.Title>
          <Item.Description class="text-left"
            >{fromEmail.address}</Item.Description
          >
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

