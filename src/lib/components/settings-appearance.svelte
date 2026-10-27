<script lang="ts">
  import { Sun, Moon, Monitor } from "@lucide/svelte";
  import * as Field from "$lib/components/ui/field";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import AccountAvatar from "./account-avatar.svelte";
  import { appearance } from "$lib/shared/appearance.svelte";

  const themes = [
    { value: "light", label: "Light", icon: Sun },
    { value: "dark", label: "Dark", icon: Moon },
    { value: "system", label: "System", icon: Monitor },
  ];
  let error = $state<string | null>(null);
  function save(action: () => void) {
    try { action(); error = null; }
    catch { error = "Couldn't save appearance settings. Please try again."; }
  }
</script>

<section class="w-full" aria-label="Appearance">
  <Field.Group>
    <h3 class="mb-2 text-base font-medium">Appearance</h3>
    <Field.Field>
      <Field.Label id="appearance-theme-label">Theme</Field.Label>
      <ToggleGroup.Root type="single" variant="outline" class="w-full" aria-labelledby="appearance-theme-label" value={appearance.theme} onValueChange={(value) => save(() => appearance.setTheme(value))}>
        {#each themes as theme (theme.value)}
          <ToggleGroup.Item value={theme.value} aria-label={theme.label}><theme.icon />{theme.label}</ToggleGroup.Item>
        {/each}
      </ToggleGroup.Root>
      <Field.Description>System follows your device's appearance.</Field.Description>
    </Field.Field>
    <Field.Field>
      <Field.Label id="appearance-avatars-label">Email avatars</Field.Label>
      <ToggleGroup.Root type="single" variant="outline" class="w-full" aria-labelledby="appearance-avatars-label" value={appearance.emailAvatars} onValueChange={(value) => save(() => appearance.setEmailAvatars(value))}>
        <ToggleGroup.Item value="blobavatars" aria-label="Blobavatars" class="h-auto flex-col gap-2 py-3">
          <AccountAvatar name="preview@onlysend.app" />
          <span>Blobavatars</span>
        </ToggleGroup.Item>
        <ToggleGroup.Item value="minimal" aria-label="Minimal" class="h-auto flex-col gap-2 py-3">
          <span class="flex size-8 items-center justify-center rounded-lg bg-muted text-xs font-medium text-muted-foreground" aria-hidden="true">A</span>
          <span>Minimal</span>
        </ToggleGroup.Item>
      </ToggleGroup.Root>
      <Field.Description>Choose avatars or initials for Inbox and Sent. Each email address keeps its own avatar.</Field.Description>
    </Field.Field>
    {#if error}<Field.Error>{error}</Field.Error>{/if}
  </Field.Group>
</section>
