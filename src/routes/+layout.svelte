<script lang="ts">
  import { Toaster } from "svelte-sonner";
  import { onMount } from "svelte";
  import { appUpdate } from "$lib/shared/app-update.svelte";
  import { protectAppExit } from "$lib/features/auth/app-exit";
  import { toast } from "svelte-sonner";
  import { appearance, isDarkTheme } from "$lib/shared/appearance.svelte";
  import "./layout.css";

  let { children } = $props();
  let systemDark = $state(false);
  let exiting = $state(false);
  onMount(() => {
    let disposed = false;
    let unprotect: (() => void) | undefined;
    if ("__TAURI_INTERNALS__" in window) {
      void protectAppExit((busy) => { exiting = busy; }).then((cleanup) => {
        if (disposed) cleanup(); else unprotect = cleanup;
      }).catch(() => toast.error("Close protection is unavailable. Wait for Saved before quitting."));
    }
    void appUpdate.checkOnce();
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    systemDark = media.matches;
    const update = () => { systemDark = media.matches; };
    media.addEventListener("change", update);
    return () => { disposed = true; unprotect?.(); media.removeEventListener("change", update); };
  });

  $effect(() => {
    const dark = isDarkTheme(appearance.theme, systemDark);
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  });
</script>

<div class="contents" inert={exiting} aria-busy={exiting}>
  {@render children()}
</div>
{#if exiting}<div class="fixed inset-x-0 top-0 z-[100] bg-background p-3 text-center text-sm" role="status">Saving before closing…</div>{/if}

<Toaster theme={appearance.theme} duration={6000} position="bottom-right" closeButton />
