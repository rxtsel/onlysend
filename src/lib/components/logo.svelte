<script lang="ts">
  import { cn } from "@/lib/utils";
  import type { HTMLImgAttributes } from "svelte/elements";

  let { class: className, ...rest }: HTMLImgAttributes = $props();
</script>

<img
  alt="logo"
  src="/icon-104.png"
  width="52"
  height="52"
  class={cn("size-13 rounded-xl", className)}
  {...rest}
/>
