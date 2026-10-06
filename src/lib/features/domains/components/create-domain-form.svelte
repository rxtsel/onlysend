<script lang="ts">
  import { toast } from "svelte-sonner";

  import { Button } from "@/lib/components/ui/button";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import * as Select from "@/lib/components/ui/select";
  import { Switch } from "@/lib/components/ui/switch";
  import { ArrowLeft } from "@lucide/svelte";

  let {
    isCreating = false,
    onBack,
    onCreate,
  }: {
    isCreating?: boolean;
    onBack: () => void;
    /** Called with a validated payload when the domain can be created. */
    onCreate: (options: {
      name: string;
      region: string;
      enableReceiving: boolean;
    }) => Promise<void>;
  } = $props();

  const REGIONS = [
    { value: "us-east-1", label: "US East (Virginia)" },
    { value: "eu-west-1", label: "Europe (Ireland)" },
    { value: "sa-east-1", label: "South America (São Paulo)" },
    { value: "ap-northeast-1", label: "Asia Pacific (Tokyo)" },
  ];

  let name = $state("");
  let region = $state("us-east-1");
  // Receiving is an explicit remote action, never a side effect of inclusion.
  let inbox = $state(false);
  let error = $state("");

  function handleSubmit(e: SubmitEvent) {
    e.preventDefault();

    const clean = name.trim().toLowerCase();
    if (!clean || !clean.includes(".")) {
      error = "Enter a valid domain (e.g. yourdomain.com)";
      return;
    }

    if (inbox && !window.confirm("Changing MX records can redirect incoming email away from your current provider. Create this domain with receiving enabled?")) return;
    onCreate({ name: clean, region, enableReceiving: inbox }).catch(
      () => {}, // parent toasts API failures
    );
  }
</script>

<form onsubmit={handleSubmit}>
  <Field.Group>
    <Field.Field>
      <Field.Label for="domainName">Domain</Field.Label>
      <Input
        id="domainName"
        bind:value={name}
        placeholder="updates.example.com"
        aria-invalid={!!error}
        autofocus
      />
      {#if error}
        <Field.Error>{error}</Field.Error>
      {/if}
      <Field.Description>
        <span class="font-medium">Tip:</span> Resend recommends using a
        subdomain (e.g. <code
          class="font-mono text-[11px] px-1 py-0.5 rounded bg-muted"
        >
          updates.example.com</code
        >) to keep your root domain's email delivery unaffected.
      </Field.Description>
    </Field.Field>

    <Field.Field>
      <Field.Label for="region">Region</Field.Label>
      <Select.Root type="single" bind:value={region}>
        <Select.Trigger id="region" class="w-full">
          {REGIONS.find((r) => r.value === region)?.label}
        </Select.Trigger>
        <Select.Content>
          {#each REGIONS as r (r.value)}
            <Select.Item value={r.value}>{r.label}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      <Field.Description>
        Where emails will be sent from. Closest to your audience is best.
      </Field.Description>
    </Field.Field>

    <div class="flex items-center gap-3 my-2">
      <div class="min-w-0 flex-1">
        <p class="text-sm font-medium">Inbox (receiving)</p>
        <p class="text-xs text-muted-foreground">
          Adds a receiving MX record. Changing MX at your DNS provider can
          redirect incoming email away from your current provider.
        </p>
      </div>
      <Switch bind:checked={inbox} />
    </div>

    <div class="flex gap-2">
      <Button type="button" variant="outline" class="flex-1" onclick={onBack}>
        <ArrowLeft /> Back
      </Button>
      <Button type="submit" class="flex-1" disabled={isCreating}>
        {isCreating ? "Creating..." : "Create"}
      </Button>
    </div>
  </Field.Group>
</form>
