<script lang="ts">
  let { content = $bindable(""), files = $bindable<File[]>([]), disabled = false } = $props();
  let input: HTMLTextAreaElement;
  export function focus() { input.focus(); return document.activeElement === input; }
</script>
<textarea bind:this={input} bind:value={content} {disabled} aria-label="Message body"></textarea>
