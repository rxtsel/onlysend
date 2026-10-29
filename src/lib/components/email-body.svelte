<script lang="ts">
  import { emailLinkFromClick } from "$lib/shared/email-html";

  let { html, label, onopenlink }: { html: string; label: string; onopenlink: (url: string) => void } = $props();
  let host = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (!host) return;
    const document = new DOMParser().parseFromString(html, "text/html");
    const root = host.shadowRoot ?? host.attachShadow({ mode: "open" });
    root.replaceChildren(document.documentElement);
  });

  $effect(() => {
    const element = host;
    if (!element) return;
    element.addEventListener("click", openLink);
    element.addEventListener("auxclick", openLink);
    return () => {
      element.removeEventListener("click", openLink);
      element.removeEventListener("auxclick", openLink);
    };
  });

  function openLink(event: MouseEvent) {
    const root = host?.shadowRoot;
    if (!root) return;
    const url = emailLinkFromClick(event, root);
    if (url) onopenlink(url);
  }
</script>

<div
  bind:this={host}
  role="document"
  aria-label={label}
  class="block w-full min-w-0 bg-white"
  style="display:block!important;position:relative!important;inset:auto!important;width:100%!important;height:auto!important;min-width:0!important;max-width:100%!important;max-height:none!important;margin:0!important;padding:0!important;float:none!important;transform:none!important;contain:layout paint!important;overflow:auto!important;isolation:isolate!important;background:#fff!important;color-scheme:light!important;z-index:auto!important"
></div>
