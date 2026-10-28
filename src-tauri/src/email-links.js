// Runs in an opaque-origin iframe; only this nonce-authorized bridge may execute.
(() => {
  function openLink(event) {
    if (!event.isTrusted || event.defaultPrevented) return;
    if (event.type === "click" ? event.button !== 0 : event.button !== 1) return;
    const anchor = event.composedPath().find((node) => node instanceof HTMLAnchorElement);
    if (!anchor) return;
    const href = anchor.getAttribute("href")?.trim();
    if (!href) return;
    // Fragments stay inside the message, regardless of the sender's target.
    if (href.startsWith("#")) {
      event.preventDefault();
      try { document.getElementById(decodeURIComponent(href.slice(1)))?.scrollIntoView(); }
      catch { /* An invalid fragment must not navigate out of the message. */ }
      return;
    }
    event.preventDefault();
    let url;
    try { url = new URL(href.startsWith("//") ? `https:${href}` : href); }
    catch { return; }
    if (!["https:", "http:"].includes(url.protocol)) return;
    if (url.username || url.password) return;
    window.parent.postMessage({
      type: "onlysend-email-link",
      token: "__ONLYSEND_TOKEN__",
      url: url.href,
    }, "*");
  }
  document.addEventListener("click", openLink);
  document.addEventListener("auxclick", openLink);
})();
