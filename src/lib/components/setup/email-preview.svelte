<script lang="ts">
  import { fade } from "svelte/transition";
  import { ChevronDown } from "@lucide/svelte";

  let {
    label,
    localPart,
    domain,
  }: {
    label: string;
    localPart: string;
    domain: string;
  } = $props();

  const displayName = $derived(label.trim() || "Your Name");
  const initial = $derived(displayName.charAt(0).toUpperCase());
  const hasLocal = $derived(localPart.trim().length > 0);
</script>

<div class="bg-background m-px min-h-30 rounded-3xl flex flex-col">
  <!-- HEADER -->
  <div class="pb-4 border-b border-border">
    <div class="flex items-center gap-3">
      <!-- AVATAR -->
      <div
        class="shrink-0 flex w-8 h-8 items-center justify-center text-xs font-medium uppercase rounded-full bg-gradient-to-br from-accent to-border text-foreground"
      >
        {initial}
      </div>

      <!-- NAME + ADDRESS -->
      <div class="flex flex-col min-w-0">
        <div class="flex items-center gap-1.5 flex-wrap">
          <span class="text-sm font-semibold text-foreground truncate">
            {displayName}
          </span>
          <span
            class="text-sm text-muted-foreground flex items-center max-w-full min-w-0"
          >
            &lt;{#key hasLocal}
              <span
                class="inline-block truncate align-middle transition-opacity duration-150"
                in:fade={{ duration: 120 }}
              >
                {#if hasLocal}
                  {localPart}@{domain}
                {:else}
                  youremail@<span
                    class="inline-block h-3 w-24 bg-muted rounded animate-pulse align-middle"
                  ></span>
                {/if}
              </span>
            {/key}&gt;
          </span>
        </div>
        <div class="flex items-center gap-1 mt-0.5">
          <span class="text-sm text-muted-foreground">to me</span>
          <ChevronDown class="size-3 text-muted-foreground" />
        </div>
      </div>
    </div>
  </div>

  <!-- BODY PLACEHOLDER -->
  <div class="pt-4 flex flex-col gap-3">
    <div class="h-3 bg-muted rounded w-3/5"></div>
    <div class="h-3 bg-muted rounded w-4/5"></div>
  </div>
</div>
