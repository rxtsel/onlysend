<script lang="ts">
  let { open = $bindable(false), name, src, restoreFocus, onclose }: {
    open?: boolean;
    name: string;
    src: string | null;
    restoreFocus?: HTMLElement | null;
    onclose?: () => void;
  } = $props();
</script>

<output data-preview-name>{name}</output>
<output data-preview-src>{src || ""}</output>
<output data-preview-focus>{String(!!restoreFocus)}</output>
<output data-preview-open>{String(open)}</output>
<button type="button" aria-label="Close preview probe" onclick={() => { open = false; onclose?.(); }}>Close</button>
