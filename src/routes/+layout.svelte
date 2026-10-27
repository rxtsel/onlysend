<script lang="ts">
  import { Toaster } from "svelte-sonner";
  import { onMount } from "svelte";
  import { appUpdate } from "$lib/shared/app-update.svelte";
  import { appearance, isDarkTheme } from "$lib/shared/appearance.svelte";
  import "./layout.css";

  let { children } = $props();
  let systemDark = $state(false);
  onMount(() => {
    void appUpdate.checkOnce();
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    systemDark = media.matches;
    const update = () => { systemDark = media.matches; };
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  });

  $effect(() => {
    const dark = isDarkTheme(appearance.theme, systemDark);
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  });
</script>

{@render children()}

<Toaster richColors duration={6000} position="bottom-right" closeButton />
