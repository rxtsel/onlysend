<script lang="ts">
	import { Button } from "@/lib/components/ui/button/index.js";
	import * as Tooltip from "@/lib/components/ui/tooltip/index.js";
	import { cn } from "@/lib/utils.js";
	import PanelLeftIcon from "@lucide/svelte/icons/panel-left";
	import { mergeProps } from "bits-ui";
	import { onMount, type ComponentProps } from "svelte";
	import { useSidebar } from "./context.svelte.js";
	import { SIDEBAR_KEYBOARD_SHORTCUT } from "./constants.js";

	let {
		ref = $bindable(null),
		class: className,
		onclick,
		...restProps
	}: ComponentProps<typeof Button> & {
		onclick?: (e: MouseEvent) => void;
	} = $props();

	const sidebar = useSidebar();
	const shortcutKey = SIDEBAR_KEYBOARD_SHORTCUT.toUpperCase();
	let isMac = $state(false);
	onMount(() => { isMac = /Mac|iPhone|iPad|iPod/.test(navigator.platform); });
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<Button
				bind:ref
				data-sidebar="trigger"
				data-slot="sidebar-trigger"
				variant="ghost"
				size="icon"
				class={cn("size-7", className)}
				type="button"
				aria-keyshortcuts={`${isMac ? "Meta" : "Control"}+${shortcutKey}`}
				{...mergeProps(props, restProps, { onclick: (e: MouseEvent) => { onclick?.(e); sidebar.toggle(); } })}
			>
				<PanelLeftIcon />
				<span class="sr-only">Toggle Sidebar</span>
			</Button>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content side="bottom" sideOffset={6} hidden={sidebar.state !== "collapsed" || sidebar.isMobile}>
		<span class="inline-flex items-center gap-2">
			Toggle sidebar
			<span class="inline-flex items-center gap-1">
				<kbd class="rounded border border-current/30 px-1 font-sans text-[10px]">{isMac ? "⌘" : "Ctrl"}</kbd>
				<kbd class="rounded border border-current/30 px-1 font-sans text-[10px]">{shortcutKey}</kbd>
			</span>
		</span>
	</Tooltip.Content>
</Tooltip.Root>
