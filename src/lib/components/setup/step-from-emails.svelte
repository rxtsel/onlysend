<script lang="ts">
  import { toast } from "svelte-sonner";

  import { emailOptionSchema } from "@/lib/schemas/email-option.schema";

  import { Button } from "@/lib/components/ui/button";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import * as InputGroup from "@/lib/components/ui/input-group";
  import * as Item from "@/lib/components/ui/item";
  import EmailPreview from "@/lib/components/setup/email-preview.svelte";
  import { ArrowRight, Trash } from "@lucide/svelte";

  let {
    domain = "",
    options = $bindable([]),
    onBack,
    onFinish,
    isSaving = false,
  } = $props();

  let emailLabel = $state("");
  let emailAddress = $state("");
  let errors = $state<Record<string, string>>({});

  /**
   * Pure helper: users often paste a full email. Splits at the first "@"
   * so we can keep only the local part and inspect the typed domain.
   */
  function stripAt(raw: string): { local: string; typedDomain: string } {
    const at = raw.indexOf("@");
    if (at === -1) return { local: raw, typedDomain: "" };
    return {
      local: raw.slice(0, at),
      typedDomain: raw.slice(at + 1).trim().toLowerCase(),
    };
  }

  const previewLocal = $derived(stripAt(emailAddress).local);

  function addEmailOption() {
    errors = {};

    const { local, typedDomain } = stripAt(emailAddress.trim());
    if (local) emailAddress = local;

    if (typedDomain && domain && typedDomain !== domain.toLowerCase()) {
      toast.info(
        `Using your selected domain ${domain} instead of ${typedDomain}.`,
      );
    }

    const fullAddress = `${local}@${domain}`;
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

    if (options.length === 0) {
      errors.fromEmail = "Add at least one email option";
      return;
    }

    onFinish();
  }
</script>

<form class="w-full max-w-sm" onsubmit={submit}>
  <Field.Group>
    <Field.Field>
      <Field.Label>From Email Options</Field.Label>

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
            <InputGroup.Text>@{domain}</InputGroup.Text>
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
        {domain}
      />

      <Button type="button" class="w-full mb-4" onclick={addEmailOption}>
        Add email
      </Button>

      <!-- SAVED OPTIONS -->
      {#if options.length > 0}
        <Item.Group class="mb-4">
          {#each options as opt, i (opt.address)}
            <Item.Root variant="outline" size="sm">
              <Item.Content>
                <Item.Title>{opt.label}</Item.Title>
                <Item.Description>{opt.address}</Item.Description>
              </Item.Content>
              <Item.Actions>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  title="Remove"
                  onclick={() => removeOption(i)}
                >
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
      <Button type="button" variant="outline" class="flex-1" onclick={onBack}>
        <ArrowRight class="-rotate-180" /> Back
      </Button>
      <Button type="submit" class="flex-1" disabled={isSaving}>
        {isSaving ? "Saving..." : "Finish"}
      </Button>
    </div>
  </Field.Group>
</form>
