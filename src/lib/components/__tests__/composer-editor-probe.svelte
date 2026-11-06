<script lang="ts">
  let { content = $bindable(""), files = $bindable<File[]>([]), disabled = false } = $props();
  let input: HTMLTextAreaElement;
  export function focus() { input.focus(); return document.activeElement === input; }
  export function getText() { return new DOMParser().parseFromString(content, "text/html").body.textContent ?? ""; }
</script>
<textarea bind:this={input} bind:value={content} {disabled} aria-label="Message body"></textarea>
<input type="file" multiple aria-label="Attach files" onchange={(event) => { files = Array.from(event.currentTarget.files ?? []); }} />
