<script lang="ts">
  import AccountAvatar from "./account-avatar.svelte";
  import { appearance } from "$lib/shared/appearance.svelte";
  import { emailAvatarSeed } from "$lib/shared/email-avatar";
  import { cn } from "$lib/utils";

  let { address, name, size = 32 }: { address: string; name: string; size?: number } = $props();
  const seed = $derived(emailAvatarSeed(address));
</script>

<span class={cn("inline-flex shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted text-xs font-medium text-muted-foreground", size >= 40 && "rounded-xl text-sm")} style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true">
  {#if appearance.emailAvatars === "blobavatars"}
    <AccountAvatar name={seed} {size} />
  {:else}
    {name.slice(0, 1).toUpperCase() || "?"}
  {/if}
</span>
