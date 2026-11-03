<script lang="ts">
  import type { Snippet } from "svelte";
  let { content = $bindable(""), files = $bindable<File[]>([]), disabled = false, toolbarActions = undefined as Snippet | undefined } = $props();
  let input: HTMLTextAreaElement;
  export function focus() { input.focus(); return document.activeElement === input; }
</script>
<div role="toolbar" aria-label="Message editor">{@render toolbarActions?.()}</div>
<textarea bind:this={input} bind:value={content} {disabled} aria-label="Message body"></textarea>
