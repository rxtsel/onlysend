import { parse, type DefaultTreeAdapterTypes } from "parse5";

type Region = { start: number; end: number; kind: "text" | "href"; value: string; link?: number };
const excluded = new Set(["head", "style", "script", "title", "textarea", "select", "option", "template", "noscript", "svg", "math"]);

export function escapeHtml(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

// Source offsets let visual edits preserve everything else, including MSO comments and formatting.
export class HtmlDocument {
  regions: Region[] = [];
  constructor(public html: string, public prefix: string) {
    const visit = (node: DefaultTreeAdapterTypes.Node, link?: number) => {
      if ("tagName" in node) {
        if (excluded.has(node.tagName)) return;
        const attribute = node.sourceCodeLocation?.attrs?.href;
        if (node.tagName === "a" && attribute) {
          link = this.regions.length;
          this.regions.push({ start: attribute.startOffset, end: attribute.endOffset, kind: "href", value: node.attrs.find((attr) => attr.name === "href")?.value ?? "" });
        }
      }
      if (node.nodeName === "#text" && "value" in node && node.value.trim() && node.sourceCodeLocation) {
        const parent = node.parentNode;
        if (parent && "tagName" in parent && ["table", "thead", "tbody", "tfoot", "tr", "colgroup"].includes(parent.tagName)) return;
        const { startOffset: start, endOffset: end } = node.sourceCodeLocation;
        this.regions.push({ start, end, kind: "text", value: node.value, link });
      }
      if ("childNodes" in node) node.childNodes.forEach((child) => visit(child, link));
    };
    visit(parse(html, { sourceCodeLocationInfo: true }));
  }

  annotated(): string {
    let html = this.html;
    this.regions.map((region, index) => ({ ...region, index })).filter((region) => region.kind === "text")
      .sort((a, b) => b.start - a.start).forEach(({ start, end, index }) => {
        html = `${html.slice(0, start)}<span id="${this.prefix}-${index}">${html.slice(start, end)}</span>${html.slice(end)}`;
      });
    return html;
  }

  replace(index: number, value: string): string {
    const region = this.regions[index];
    if (!region || region.value === value) return this.html;
    if (region.kind === "href" && !isEmailLink(value)) throw new Error("Use an https://, http://, mailto:, tel: or # link");
    const replacement = region.kind === "text" ? escapeHtml(value) : `href="${escapeHtml(value)}"`;
    const { start, end } = region;
    this.html = this.html.slice(0, start) + replacement + this.html.slice(end);
    const delta = replacement.length - (end - start);
    for (const other of this.regions) {
      if (other === region) continue;
      if (other.start >= end) { other.start += delta; other.end += delta; }
    }
    region.end = start + replacement.length;
    region.value = value;
    return this.html;
  }
}

export function isEmailLink(value: string): boolean {
  if (/\p{Cc}/u.test(value)) return false;
  if (value.startsWith("#")) return true;
  try {
    const url = new URL(value);
    return ["https:", "http:", "mailto:", "tel:"].includes(url.protocol) && !url.username && !url.password;
  } catch { return false; }
}

export function previewDocument(sanitized: string): string {
  const document = new DOMParser().parseFromString(sanitized, "text/html");
  const policy = document.createElement("meta");
  policy.httpEquiv = "Content-Security-Policy";
  policy.content = "default-src 'none'; script-src 'none'; img-src https: http: data:; style-src 'unsafe-inline' https: http:; font-src https: http: data:; base-uri 'none'; form-action 'none'";
  const referrer = document.createElement("meta");
  referrer.name = "referrer";
  referrer.content = "no-referrer";
  document.head.prepend(policy, referrer);
  return `<!doctype html>${document.documentElement.outerHTML}`;
}
