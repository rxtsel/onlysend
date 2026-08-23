<script lang="ts">
  import { Toaster } from "svelte-sonner";
  import "./layout.css";

  let { children } = $props();
</script>

{@render children()}

<Toaster richColors duration={6000} position="bottom-right" closeButton />
