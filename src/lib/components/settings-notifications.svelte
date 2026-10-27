<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import * as Select from "$lib/components/ui/select";
  import * as Field from "$lib/components/ui/field";
  import { errorMessage } from "$lib/shared/utils/errors";

  const options = [
    { value: "0", label: "Off" },
    { value: "5", label: "Every 5 minutes (default)" },
    { value: "10", label: "Every 10 minutes" },
    { value: "30", label: "Every 30 minutes" },
    { value: "60", label: "Every hour" },
  ];
  let value = $state("5");
  let loaded = $state(false);
  let saving = $state(false);
  let error = $state<string | null>(null);

  onMount(() => {
    void invoke<number>("get_inbox_poll_minutes")
      .then((minutes) => { value = String(minutes); loaded = true; })
      .catch((err) => { error = errorMessage(err, "Could not load notification settings"); });
  });

  async function save(next: string) {
    if (!next || saving) return;
    saving = true;
    error = null;
    try {
      await invoke("set_inbox_poll_minutes", { minutes: Number(next) });
      value = next;
    } catch (err) {
      error = errorMessage(err, "Could not save notification settings");
    } finally {
      saving = false;
    }
  }
</script>

<section class="w-full" aria-label="Inbox notifications">
  <Field.Group>
    <div class="mb-2">
      <h3 class="text-base font-medium">Inbox notifications</h3>
      <Field.Description class="mt-1">Get notified when new emails arrive.</Field.Description>
    </div>
  <Field.Field data-disabled={!loaded || saving}>
    <Field.Label for="inbox-poll-interval">Check for new mail</Field.Label>
    <Select.Root type="single" {value} onValueChange={save} disabled={!loaded || saving}>
      <Select.Trigger id="inbox-poll-interval" aria-describedby="inbox-poll-description">
        {options.find((option) => option.value === value)?.label}
      </Select.Trigger>
      <Select.Content>
        <Select.Group>
          {#each options as option}
            <Select.Item value={option.value}>{option.label}</Select.Item>
          {/each}
        </Select.Group>
      </Select.Content>
    </Select.Root>
    <Field.Description id="inbox-poll-description">
      Keep OnlySend open to receive notifications, even when minimized.
    </Field.Description>
  </Field.Field>
  {#if saving}<Field.Description role="status">Saving…</Field.Description>{/if}
  {#if error}<Field.Error>{error}</Field.Error>{/if}
  </Field.Group>
</section>
