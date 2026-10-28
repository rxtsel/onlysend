import { invoke } from "@tauri-apps/api/core";
import { contentId } from "./attachments";

export function prepareEmailHtml(html: string, inlineImages: Record<string, string> = {}): Promise<string> {
  const parsed = new DOMParser().parseFromString(html, "text/html");
  if (Object.keys(inlineImages).length) {
    for (const image of parsed.querySelectorAll("img[src]")) {
      const source = image.getAttribute("src") || "";
      const id = contentId(source);
      if (/^cid:/i.test(source) && Object.hasOwn(inlineImages, id)) image.setAttribute("src", inlineImages[id]);
    }
    html = parsed.documentElement.outerHTML;
  }
  return invoke<string>("prepare_email_html", {
    html,
    htmlAttributes: Object.fromEntries(Array.from(parsed.documentElement.attributes, ({ name, value }) => [name, value])),
    bodyAttributes: Object.fromEntries(Array.from(parsed.body.attributes, ({ name, value }) => [name, value])),
  });
}

export function emailLinkFromClick(
  event: Pick<MouseEvent, "isTrusted" | "defaultPrevented" | "button" | "type" | "composedPath" | "preventDefault">,
  root: ShadowRoot,
): string | null {
  if (!event.isTrusted || event.defaultPrevented || (event.type === "click" ? event.button !== 0 : event.button !== 1)) return null;
  const anchor = event.composedPath().find((node) => node instanceof HTMLAnchorElement);
  if (!(anchor instanceof HTMLAnchorElement)) return null;
  const href = anchor.getAttribute("href")?.trim();
  if (!href) return null;
  event.preventDefault();
  if (href.startsWith("#")) {
    try { root.getElementById(decodeURIComponent(href.slice(1)))?.scrollIntoView(); }
    catch { /* Ignore invalid fragment escapes. */ }
    return null;
  }
  let url: URL;
  try { url = new URL(href.startsWith("//") ? `https:${href}` : href); }
  catch { return null; }
  return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password ? url.href : null;
}
