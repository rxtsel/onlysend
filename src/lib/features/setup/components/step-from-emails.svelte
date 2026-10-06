<script lang="ts">
  import { toast } from "svelte-sonner";
  import type { DomainSummary } from "$lib/shared/api/domains";
  import { sendingReady } from "$lib/features/domains/domain-setup-store.svelte";
  import * as Select from "$lib/components/ui/select";

  import { emailOptionSchema } from "@/lib/schemas/email-option.schema";
  import { stripAt } from "@/lib/shared/email";

  import { Button } from "@/lib/components/ui/button";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import * as InputGroup from "@/lib/components/ui/input-group";
  import * as Item from "@/lib/components/ui/item";
  import EmailPreview from "@/lib/features/setup/components/email-preview.svelte";
  import { Blobatar } from "@blobatar/svelte";
  import { ArrowRight, Trash } from "@lucide/svelte";

  let {
    domain = "",
    domains = [],
    options = $bindable([]),
    onBack,
    onFinish,
    isSaving = false,
  }: {
    domain?: string;
    domains?: DomainSummary[];
    options?: { label: string; address: string }[];
    onBack: () => void;
    onFinish: () => void;
    isSaving?: boolean;
  } = $props();

  const readyDomains = $derived(domains.filter(sendingReady));
  let senderDomain = $state("");
  $effect(() => {
    if (!readyDomains.some((item) => item.name === senderDomain))
      senderDomain = readyDomains.find((item) => item.name === domain)?.name ?? readyDomains[0]?.name ?? "";
  });
  function supported(address: string): boolean {
    return readyDomains.some((item) => item.name.toLowerCase() === address.split("@").at(-1)?.toLowerCase());
  }

  let emailLabel = $state("");
  let emailAddress = $state("");
  let errors = $state<Record<string, string>>({});

  const previewLocal = $derived(stripAt(emailAddress).local);

  function addEmailOption() {
    errors = {};

    const { local, typedDomain } = stripAt(emailAddress.trim());
    if (local) emailAddress = local;

    if (!senderDomain) return;
    if (typedDomain && typedDomain !== senderDomain.toLowerCase()) {
      toast.info(`Using your selected domain ${senderDomain} instead of ${typedDomain}.`);
    }

    const fullAddress = `${local}@${senderDomain}`;
    if (options.some((option) => option.address.toLowerCase() === fullAddress.toLowerCase())) {
      errors.address = "This sender is already listed";
      return;
    }
    const result = emailOptionSchema.safeParse({
      label: emailLabel,
      address: fullAddress,
    });

    if (!result.success) {
      for (const issue of result.error.issues) {
        errors[issue.path[0]?.toString()] = issue.message;
      }
      return;
    }

    options = [...options, { label: emailLabel, address: fullAddress }];

    emailLabel = "";
    emailAddress = "";
  }

  function removeOption(index: number) {
    options = options.filter((_, i) => i !== index);
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    errors = {};

    if (options.some((option) => !supported(option.address))) {
      errors.fromEmail = "Remove senders from domains that are no longer included or ready";
      return;
    }

    onFinish();
  }
</script>

<form class="w-full max-w-sm" onsubmit={submit}>
  <Field.Group>
    <Field.Field>
      <Field.Label>From Email Options</Field.Label>
      <Field.Description>
        First sender is the account default, not a mailbox filter. Identities are local;
        they do not create remote inboxes. You can finish without a sender and add one later.
      </Field.Description>
      {#if readyDomains.length > 0}
        <Select.Root type="single" bind:value={senderDomain}>
          <Select.Trigger aria-label="Sending domain">{senderDomain}</Select.Trigger>
          <Select.Content>
            <Select.Group>
              {#each readyDomains as item (item.id)}
                <Select.Item value={item.name}>{item.name}</Select.Item>
              {/each}
            </Select.Group>
          </Select.Content>
        </Select.Root>
      {:else}
        <Field.Description>No included domains are ready for sending. Configure DNS later in Settings.</Field.Description>
      {/if}

      <div class="flex flex-col gap-2 mb-3">
        <Input
          placeholder="Your Name"
          bind:value={emailLabel}
          aria-invalid={!!errors.label}
          autofocus
        />
        <InputGroup.Root>
          <InputGroup.Input
            placeholder="hello"
            bind:value={emailAddress}
            aria-invalid={!!errors.address}
          />
          <InputGroup.Addon align="inline-end">
            <InputGroup.Text>@{senderDomain || "domain pending"}</InputGroup.Text>
          </InputGroup.Addon>
        </InputGroup.Root>
      </div>

      {#if errors.label}
        <Field.Error>{errors.label}</Field.Error>
      {/if}
      {#if errors.address}
        <Field.Error>{errors.address}</Field.Error>
      {/if}

      <!-- LIVE PREVIEW -->
      <EmailPreview
        label={emailLabel}
        localPart={previewLocal}
        domain={senderDomain}
      />

      <Button type="button" class="w-full mb-4" disabled={!senderDomain || isSaving} onclick={addEmailOption}>
        Add email
      </Button>

      <!-- SAVED OPTIONS -->
      {#if options.length > 0}
        <Item.Group class="mb-4">
          {#each options as opt, i (opt.address)}
            <Item.Root variant="outline" size="sm">
              <Item.Media>
                <Blobatar
                  name={opt.address}
                  size={28}
                  class="rounded-full shrink-0"
                />
              </Item.Media>
              <Item.Content>
                <Item.Title>{opt.label}</Item.Title>
                <Item.Description>{opt.address}{i === 0 ? " · Default" : ""}</Item.Description>
                {#if !supported(opt.address)}
                  <Item.Description>Domain no longer included or ready; remove this sender or go back.</Item.Description>
                {/if}
              </Item.Content>
              <Item.Actions>
                {#if i > 0}
                  <Button type="button" variant="ghost" size="sm" disabled={isSaving}
                    onclick={() => { options = [opt, ...options.filter((_, index) => index !== i)]; }}>Set default</Button>
                {/if}
                <Button type="button" disabled={isSaving} variant="destructive" size="icon-sm" title="Remove" onclick={() => removeOption(i)}>
                  <Trash />
                </Button>
              </Item.Actions>
            </Item.Root>
            {#if i !== options.length - 1}
              <Item.Separator />
            {/if}
          {/each}
        </Item.Group>
      {/if}

      {#if errors.fromEmail}
        <Field.Error>{errors.fromEmail}</Field.Error>
      {/if}
    </Field.Field>

    <div class="flex gap-2">
      <Button type="button" variant="outline" class="flex-1" disabled={isSaving} onclick={onBack}>
        <ArrowRight class="-rotate-180" /> Back
      </Button>
      <Button type="submit" class="flex-1" disabled={isSaving}>
        {isSaving ? "Saving..." : options.length ? "Finish" : "Finish without sender"}
      </Button>
    </div>
  </Field.Group>
</form>
