<script lang="ts">
  import { cn } from "$lib/utils";
  import { ContextMenu as ContextMenuPrimitive } from "bits-ui";

  let { ref = $bindable(null), portalProps, class: className, ...restProps }: ContextMenuPrimitive.ContentProps & {
    portalProps?: ContextMenuPrimitive.PortalProps;
  } = $props();
</script>

<ContextMenuPrimitive.Portal {...portalProps}>
  <ContextMenuPrimitive.Content
    bind:ref
    data-slot="context-menu-content"
    class={cn("bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 z-50 min-w-[8rem] overflow-y-auto overflow-x-hidden rounded-md border p-1 shadow-md outline-none", className)}
    {...restProps}
  />
</ContextMenuPrimitive.Portal>
