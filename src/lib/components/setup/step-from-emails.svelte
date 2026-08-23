<script lang="ts">
  import { emailOptionSchema } from "@/lib/schemas/email-option.schema";

  import { Button } from "@/lib/components/ui/button";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import { ArrowRight } from "@lucide/svelte";

  let {
    domain = "",
    options = $bindable([]),
    onBack,
    onFinish,
    isSaving = false,
  }: {
    domain?: string;
    options?: { label: string; address: string }[];
    onBack: () => void;
    onFinish: () => void;
    isSaving?: boolean;
  } = $props();

  let emailLabel = $state("");
  let emailAddress = $state("");
  let errors = $state<Record<string, string>>({});

  function addEmailOption() {
    errors = {};

    const result = emailOptionSchema.safeParse({
      label: emailLabel,
      address: emailAddress,
    });

    if (!result.success) {
      for (const issue of result.error.issues) {
        errors[issue.path[0]?.toString()] = issue.message;
      }
      return;
    }

    options = [...options, { label: emailLabel, address: emailAddress }];

    emailLabel = "";
    emailAddress = "";
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

      <div class="flex gap-2 mb-3">
        <Input
          placeholder="Label"
          bind:value={emailLabel}
          aria-invalid={!!errors.label}
          autofocus
        />
        <Input
          placeholder={domain ? `hello@${domain}` : "email@domain.com"}
          bind:value={emailAddress}
          aria-invalid={!!errors.address}
        />
      </div>

      {#if errors.label}
        <Field.Error>{errors.label}</Field.Error>
      {/if}
      {#if errors.address}
        <Field.Error>{errors.address}</Field.Error>
      {/if}

      <Button type="button" class="w-full mb-4" onclick={addEmailOption}>
        Add email
      </Button>

      <!-- LIST -->
      {#each options as opt}
        <p class="border px-3 py-1 rounded bg-accent/20">
          {opt.label} - {opt.address}
        </p>
      {/each}

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
