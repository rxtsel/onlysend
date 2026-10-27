<script lang="ts" module>
  let avatar: Promise<typeof import("@blobatar/svelte")> | undefined;
  function loadAvatar() {
    return avatar ??= import("@blobatar/svelte");
  }
</script>

<script lang="ts">
  let { name, size = 32 }: { name: string; size?: number } = $props();
  const source = loadAvatar();
</script>

<span class="inline-flex shrink-0 overflow-hidden rounded-lg bg-muted" style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true">
  {#await source}
    <span class="size-full"></span>
  {:then { Blobatar }}
    <Blobatar {name} {size} />
  {:catch}
    <span class="size-full"></span>
  {/await}
</span>
