<script lang="ts">
  import { Toaster } from "svelte-sonner";
  import { onMount } from "svelte";
  import { appUpdate } from "$lib/shared/app-update.svelte";
  import "./layout.css";

  let { children } = $props();
  onMount(() => { void appUpdate.checkOnce(); });
</script>

{@render children()}

<Toaster richColors duration={6000} position="bottom-right" closeButton />
